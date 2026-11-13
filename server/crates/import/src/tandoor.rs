//! Tandoor: export zip containing one zip per recipe, each holding
//! `recipe.json` and `image.(jpg|jpeg|png|webp)`.

use larder_core::{Ingredient, Instruction, RecipeData};
use serde_json::Value;

use crate::archive::{basename, dirname, join, strip_bom, SafeZip};
use crate::util::{as_bool, as_f64, as_text, clean_block, clean_line, get, name_list, parse_nutrition_text, text_of, to_rfc3339};
use crate::{Collector, ImageSource, ImportError, ImportedRecipe};

pub(crate) fn parse_zip(zip: &mut SafeZip, c: &mut Collector) -> Result<(), ImportError> {
    let inner: Vec<String> = zip.names().filter(|n| n.to_ascii_lowercase().ends_with(".zip")).map(str::to_string).collect();
    if inner.is_empty() {
        // A single recipe zip (recipe.json at the root or in one folder).
        let jsons: Vec<String> = zip.names().filter(|n| basename(n) == "recipe.json").map(str::to_string).collect();
        if jsons.is_empty() {
            return Err(ImportError::UnsupportedFormat("no Tandoor recipes found in the archive".into()));
        }
        for j in jsons {
            parse_recipe_dir(zip, dirname(&j), &j, c);
        }
        return Ok(());
    }
    for name in inner {
        let ctx = basename(&name).to_string();
        let bytes = match zip.read(&name) {
            Ok(b) => b,
            Err(e @ crate::ImportError::TooLarge(_)) => return Err(e),
            Err(e) => {
                c.error(&ctx, e);
                continue;
            }
        };
        let mut sub = match SafeZip::open(bytes, zip.budget()) {
            Ok(z) => z,
            Err(e) => {
                c.error(&ctx, e);
                continue;
            }
        };
        let json = sub
            .names()
            .find(|n| basename(n) == "recipe.json")
            .map(str::to_string)
            .or_else(|| sub.names().find(|n| n.ends_with(".json")).map(str::to_string));
        let Some(json) = json else {
            c.error(&ctx, "recipe.json not found");
            continue;
        };
        parse_recipe_dir(&mut sub, dirname(&json).to_string().as_str(), &json, c);
    }
    Ok(())
}

fn parse_recipe_dir(zip: &mut SafeZip, dir: &str, json: &str, c: &mut Collector) {
    let ctx = if dir.is_empty() { json.to_string() } else { dir.to_string() };
    let v: Value = match zip
        .read(json)
        .map_err(|e| e.to_string())
        .and_then(|b| serde_json::from_slice(strip_bom(&b)).map_err(|e| format!("invalid JSON: {e}")))
    {
        Ok(v) => v,
        Err(e) => {
            c.error(&ctx, e);
            return;
        }
    };
    let mut r = recipe_from_tandoor_json(&v);
    let img = ["jpg", "jpeg", "png", "webp", "gif"].iter().find_map(|ext| zip.find_ci(&join(dir, &format!("image.{ext}"))));
    if let Some(img) = img {
        match zip.read(&img) {
            Ok(b) => r.image = Some(ImageSource::Bytes(b)),
            Err(e) => tracing::warn!("tandoor image: {e}"),
        }
    }
    c.push(r, &ctx);
}

pub(crate) fn parse_json_value(v: &Value, c: &mut Collector) {
    let items: Vec<&Value> = match v {
        Value::Array(a) => a.iter().collect(),
        Value::Object(o) if o.get("results").is_some_and(Value::is_array) => o["results"].as_array().unwrap().iter().collect(),
        _ => vec![v],
    };
    for (i, item) in items.into_iter().enumerate() {
        c.push(recipe_from_tandoor_json(item), &format!("#{}", i + 1));
    }
}

fn tandoor_ingredient(i: &Value) -> Ingredient {
    let food = get(i, &["food"]).and_then(|f| text_of(f, &["name"]).or_else(|| as_text(f))).map(|s| clean_line(&s));
    let unit = get(i, &["unit"]).and_then(|u| text_of(u, &["name"]).or_else(|| as_text(u))).map(|s| clean_line(&s));
    let note = text_of(i, &["note"]).map(|s| clean_line(&s)).filter(|s| !s.is_empty());
    let no_amount = get(i, &["no_amount"]).is_some_and(as_bool);
    let amount = if no_amount { None } else { get(i, &["amount"]).and_then(as_f64).filter(|a| *a > 0.0) };
    if food.is_none() && unit.is_none() {
        // Unparsed line: keep the original text.
        let line = text_of(i, &["original_text"]).or(note.clone()).unwrap_or_default();
        return Ingredient { original: clean_line(&line), ..Default::default() };
    }
    Ingredient {
        quantity: amount,
        unit: if amount.is_none() && no_amount { None } else { unit },
        food,
        note,
        // Let normalize() render it from the structured fields.
        original: String::new(),
        ..Default::default()
    }
}

/// Maps a Tandoor `recipe.json` (export or API) to an [`ImportedRecipe`].
pub(crate) fn recipe_from_tandoor_json(v: &Value) -> ImportedRecipe {
    let mut d = RecipeData {
        name: text_of(v, &["name"]).map(|s| clean_line(&s)).unwrap_or_default(),
        description: text_of(v, &["description"]).map(|s| clean_block(&s)).unwrap_or_default(),
        ..Default::default()
    };
    d.yield_quantity = get(v, &["servings"]).and_then(as_f64).filter(|s| *s > 0.0);
    if let Some(t) = text_of(v, &["servings_text"]).map(|s| clean_line(&s)).filter(|s| !s.is_empty()) {
        d.yield_unit = t;
    }
    let minutes = |k: &str| get(v, &[k]).and_then(as_f64).filter(|m| *m > 0.0).map(|m| m.round() as i64);
    d.prep_time_minutes = minutes("working_time");
    d.cook_time_minutes = minutes("waiting_time");
    d.source_url = text_of(v, &["source_url"]).filter(|u| u.starts_with("http"));
    d.rating = get(v, &["rating"]).and_then(as_f64).map(|r| r.round() as i64);
    if let Some(k) = get(v, &["keywords"]) {
        d.tags.extend(name_list(k));
    }

    let steps: Vec<&Value> = get(v, &["steps"]).and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    let steps_with_ingredients =
        steps.iter().filter(|s| get(s, &["ingredients"]).and_then(Value::as_array).is_some_and(|a| !a.is_empty())).count();
    let mut sorted = steps.clone();
    sorted.sort_by(|a, b| {
        let oa = get(a, &["order"]).and_then(as_f64).unwrap_or(0.0);
        let ob = get(b, &["order"]).and_then(as_f64).unwrap_or(0.0);
        oa.total_cmp(&ob)
    });
    for step in sorted {
        let step_name = text_of(step, &["name"]).map(|s| clean_line(&s)).filter(|s| !s.is_empty());
        let mut ing_section = if steps_with_ingredients > 1 { step_name.clone() } else { None };
        if let Some(Value::Array(ings)) = get(step, &["ingredients"]) {
            for i in ings {
                if get(i, &["is_header"]).is_some_and(as_bool) {
                    ing_section = text_of(i, &["note"])
                        .or_else(|| get(i, &["food"]).and_then(|f| text_of(f, &["name"])))
                        .map(|s| clean_line(&s))
                        .filter(|s| !s.is_empty());
                    continue;
                }
                let mut ing = tandoor_ingredient(i);
                ing.section = ing_section.clone();
                d.ingredients.push(ing);
            }
        }
        if let Some(text) = text_of(step, &["instruction", "instructions_markdown"]) {
            let mut section = step_name.clone();
            for para in clean_block(&text).split("\n\n") {
                let mut lines: Vec<&str> = para.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
                if lines.is_empty() {
                    continue;
                }
                if let Some(h) = lines[0].strip_prefix('#') {
                    section = Some(h.trim_start_matches('#').trim().to_string()).filter(|s| !s.is_empty());
                    lines.remove(0);
                }
                if lines.is_empty() {
                    continue;
                }
                d.instructions.push(Instruction { section: section.clone(), text: lines.join("\n") });
            }
        }
    }

    if let Some(n) = get(v, &["nutrition"]) {
        match n {
            Value::Object(o) => {
                for (k, val) in o {
                    if let (Some(key), Some(val)) = (crate::util::nutrition_key(k), as_f64(val).filter(|f| *f > 0.0)) {
                        let unit = match key.as_str() {
                            "calories" => " kcal",
                            "sodiumContent" | "cholesterolContent" => " mg",
                            _ => " g",
                        };
                        d.nutrition.insert(key, format!("{}{unit}", larder_core::format::format_quantity(val)));
                    }
                }
            }
            Value::String(s) => d.nutrition.extend(parse_nutrition_text(s)),
            _ => {}
        }
    }
    d.normalize();
    let created_at = text_of(v, &["created_at"]).and_then(|s| to_rfc3339(&s));
    ImportedRecipe { data: d, image: None, created_at, favorite: false, trial: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::testutil::make_zip;
    use crate::{parse_file, Source};
    use serde_json::json;

    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10, b'J', b'F', b'I', b'F'];

    fn recipe() -> Value {
        json!({
            "name": "“Texas Red” Chili",
            "description": "",
            "keywords": [{"name": "American", "icon": null}, {"name": "Course: Main"}],
            "working_time": 30, "waiting_time": 120, "internal": true,
            "nutrition": {"calories": 520.0, "carbohydrates": 12.5, "fats": 30, "proteins": 45, "source": "x"},
            "servings": 6, "servings_text": "bowls",
            "source_url": "https://craft.example/chili",
            "steps": [
                {"name": "Chile purée", "instruction": "Roast the chiles.\n\nBlend with beer.", "order": 0, "time": 0,
                 "ingredients": [
                    {"food": {"name": "New Mexico chile, dry"}, "unit": {"name": "oz"}, "amount": 2.0, "note": "57 g",
                     "is_header": false, "no_amount": false},
                    {"food": {"name": "doppelbock"}, "unit": {"name": "cup"}, "amount": 1.5, "note": "", "is_header": false}
                 ]},
                {"name": "Stew", "instruction": "Brown the meat.\n\n### Serving\nTop with sour cream.", "order": 1,
                 "ingredients": [
                    {"food": null, "unit": null, "amount": 0, "note": "Garnish", "is_header": true},
                    {"food": {"name": "salt"}, "unit": null, "amount": 0, "note": "to taste", "is_header": false, "no_amount": true}
                 ]}
            ]
        })
    }

    #[test]
    fn maps_recipe() {
        let r = recipe_from_tandoor_json(&recipe());
        let d = &r.data;
        assert_eq!(d.name, "“Texas Red” Chili");
        assert_eq!((d.yield_quantity, d.yield_unit.as_str()), (Some(6.0), "bowls"));
        assert_eq!((d.prep_time_minutes, d.cook_time_minutes, d.total_time_minutes), (Some(30), Some(120), Some(150)));
        assert_eq!(d.tags, vec!["American", "Course: Main"]);
        assert_eq!(d.ingredients.len(), 3);
        assert_eq!(d.ingredients[0].section.as_deref(), Some("Chile purée"));
        assert_eq!(d.ingredients[0].food.as_deref(), Some("New Mexico chile, dry"));
        assert_eq!(d.ingredients[0].original, "2 oz New Mexico chile, dry, 57 g");
        assert_eq!(d.ingredients[2].section.as_deref(), Some("Garnish"));
        assert_eq!(d.ingredients[2].quantity, None);
        assert_eq!(d.instructions.len(), 4);
        assert_eq!(d.instructions[3].section.as_deref(), Some("Serving"));
        assert_eq!(d.instructions[3].text, "Top with sour cream.");
        assert_eq!(d.nutrition.get("calories").map(String::as_str), Some("520 kcal"));
        assert_eq!(d.nutrition.get("fatContent").map(String::as_str), Some("30 g"));
    }

    #[test]
    fn nested_zip_export() {
        let j = serde_json::to_vec(&recipe()).unwrap();
        let inner1 = make_zip(&[("recipe.json", &j), ("image.jpeg", JPEG)]);
        let inner2 = make_zip(&[("readme.txt", b"nothing")]);
        let outer = make_zip(&[("1.zip", &inner1), ("2.zip", &inner2)]);
        let b = parse_file(Source::Auto, "export.zip", &outer).unwrap();
        assert_eq!(b.source, Source::Tandoor);
        assert_eq!(b.recipes.len(), 1);
        assert_eq!(b.errors, vec!["Recipe '2.zip': recipe.json not found"]);
        assert!(matches!(b.recipes[0].image, Some(ImageSource::Bytes(_))));
        assert_eq!(b.recipes[0].data.source_name.as_deref(), Some("craft.example"));

        // Explicit source with a single recipe zip.
        let b = parse_file(Source::Tandoor, "1.zip", &inner1).unwrap();
        assert_eq!(b.recipes.len(), 1);
        // Auto-detected single recipe zip.
        assert_eq!(parse_file(Source::Auto, "1.zip", &inner1).unwrap().source, Source::Tandoor);
    }
}
