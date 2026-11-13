//! Paprika: `.paprikarecipes` (zip of gzip-compressed JSON `.paprikarecipe`
//! files) or a single `.paprikarecipe`.

use base64::Engine;
use larder_core::{Instruction, RecipeData};
use serde_json::Value;

use crate::archive::{basename, gunzip_limited, is_gzip, strip_bom, Budget, SafeZip, MAX_ENTRY_BYTES};
use crate::schema_org::{ingredients_from_lines, parse_iso8601_duration};
use crate::util::{
    as_bool, as_f64, clean_block, clean_line, get, has_step_number, name_list, parse_nutrition_text, parse_yield_text, resolve_url,
    strip_step_number, text_of, to_rfc3339,
};
use crate::{Collector, ImageSource, ImportError, ImportedRecipe};

pub(crate) fn parse_zip(zip: &mut SafeZip, c: &mut Collector) -> Result<(), ImportError> {
    let files: Vec<String> = zip.names().filter(|n| n.to_ascii_lowercase().ends_with(".paprikarecipe")).map(str::to_string).collect();
    if files.is_empty() {
        return Err(ImportError::UnsupportedFormat("no .paprikarecipe files found in the archive".into()));
    }
    let budget = zip.budget();
    for f in files {
        let ctx = basename(&f).trim_end_matches(".paprikarecipe").to_string();
        match zip.read(&f) {
            Ok(b) => {
                if let Err(e) = parse_single(&b, &ctx, &budget, c) {
                    if matches!(e, ImportError::TooLarge(_)) {
                        return Err(e);
                    }
                    c.error(&ctx, e);
                }
            }
            Err(e @ ImportError::TooLarge(_)) => return Err(e),
            Err(e) => c.error(&ctx, e),
        }
    }
    Ok(())
}

/// One `.paprikarecipe` (gzip JSON; plain JSON is accepted too).
pub(crate) fn parse_single(bytes: &[u8], ctx: &str, budget: &Budget, c: &mut Collector) -> Result<(), ImportError> {
    let json = if is_gzip(bytes) {
        let out = gunzip_limited(bytes, MAX_ENTRY_BYTES)?;
        budget.take(out.len() as u64)?;
        out
    } else {
        bytes.to_vec()
    };
    let v: Value = serde_json::from_slice(strip_bom(&json))?;
    let ctx = ctx.trim_end_matches(".paprikarecipe");
    parse_json_value_ctx(&v, ctx, c);
    Ok(())
}

pub(crate) fn parse_json_value(v: &Value, c: &mut Collector) {
    parse_json_value_ctx(v, "#1", c)
}

fn parse_json_value_ctx(v: &Value, ctx: &str, c: &mut Collector) {
    match v {
        Value::Array(a) => {
            for (i, item) in a.iter().enumerate() {
                c.push(recipe_from_paprika_json(item), &format!("{ctx} #{}", i + 1));
            }
        }
        _ => c.push(recipe_from_paprika_json(v), ctx),
    }
}

/// Splits Paprika's free-text directions into steps.
pub(crate) fn split_directions(text: &str) -> Vec<Instruction> {
    let text = clean_block(text);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let numbered = lines.iter().filter(|l| has_step_number(l)).count() >= 2;
    let mut out: Vec<Instruction> = vec![];
    let mut section: Option<String> = None;
    let mut current: Option<String> = None;
    let flush = |cur: &mut Option<String>, out: &mut Vec<Instruction>, section: &Option<String>| {
        if let Some(t) = cur.take() {
            out.push(Instruction { section: section.clone(), text: t });
        }
    };
    for line in lines {
        if line.is_empty() {
            if !numbered {
                flush(&mut current, &mut out, &section);
            }
            continue;
        }
        let is_heading = (line.ends_with(':') && line.chars().count() < 50 && !line.contains(". ")) || line.starts_with('#');
        if is_heading && !has_step_number(line) {
            flush(&mut current, &mut out, &section);
            section = Some(line.trim_start_matches('#').trim_end_matches(':').trim().to_string()).filter(|s| !s.is_empty());
            continue;
        }
        if numbered {
            if has_step_number(line) {
                flush(&mut current, &mut out, &section);
                current = Some(strip_step_number(line));
            } else if let Some(cur) = current.as_mut() {
                cur.push('\n');
                cur.push_str(line);
            } else {
                current = Some(line.to_string());
            }
        } else {
            // Paprika users typically put one step per line (or paragraph).
            flush(&mut current, &mut out, &section);
            current = Some(line.to_string());
        }
    }
    flush(&mut current, &mut out, &section);
    out
}

/// Maps one Paprika recipe JSON to an [`ImportedRecipe`].
pub(crate) fn recipe_from_paprika_json(v: &Value) -> ImportedRecipe {
    let mut d = RecipeData {
        name: text_of(v, &["name"]).map(|s| clean_line(&s)).unwrap_or_default(),
        description: text_of(v, &["description"]).map(|s| clean_block(&s)).unwrap_or_default(),
        ..Default::default()
    };
    if let Some(s) = text_of(v, &["servings", "yield"]) {
        let (q, u) = parse_yield_text(&s);
        d.yield_quantity = q;
        d.yield_unit = u;
    }
    let dur = |k: &str| text_of(v, &[k]).and_then(|s| parse_iso8601_duration(&s));
    d.prep_time_minutes = dur("prep_time");
    d.cook_time_minutes = dur("cook_time");
    d.total_time_minutes = dur("total_time");
    d.source_url = text_of(v, &["source_url"]).and_then(|u| resolve_url(&u, None));
    d.source_name = text_of(v, &["source"]).map(|s| clean_line(&s)).filter(|s| !s.is_empty() && !s.starts_with("http"));
    d.rating = get(v, &["rating"]).and_then(as_f64).map(|r| r.round() as i64);
    if let Some(cats) = get(v, &["categories"]) {
        d.categories = name_list(cats);
    }
    if let Some(ing) = text_of(v, &["ingredients"]) {
        let lines: Vec<String> = clean_block(&ing).lines().map(str::to_string).collect();
        d.ingredients = ingredients_from_lines(&lines);
    }
    if let Some(dir) = text_of(v, &["directions"]) {
        d.instructions = split_directions(&dir);
    }
    let mut notes = text_of(v, &["notes"]).map(|s| clean_block(&s)).unwrap_or_default();
    if let Some(n) = text_of(v, &["nutritional_info"]) {
        let pairs = parse_nutrition_text(&n);
        if pairs.is_empty() {
            if !notes.is_empty() {
                notes.push_str("\n\n");
            }
            notes.push_str("Nutrition:\n");
            notes.push_str(&clean_block(&n));
        } else {
            d.nutrition.extend(pairs);
        }
    }
    d.notes = notes;
    d.normalize();

    let image = text_of(v, &["photo_data"])
        .and_then(|b64| {
            let cleaned: String = b64.chars().filter(|c| !c.is_whitespace()).collect();
            base64::engine::general_purpose::STANDARD.decode(cleaned).ok()
        })
        .filter(|b| !b.is_empty())
        .map(ImageSource::Bytes)
        .or_else(|| text_of(v, &["image_url"]).and_then(|u| resolve_url(&u, None)).map(ImageSource::Url));
    let created_at = text_of(v, &["created"]).and_then(|s| to_rfc3339(&s));
    let favorite = get(v, &["on_favorites"]).is_some_and(as_bool);
    ImportedRecipe { data: d, image, created_at, favorite, trial: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::testutil::{gzip, make_zip};
    use crate::{parse_file, Source};
    use serde_json::json;

    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10, b'J', b'F', b'I', b'F', 0, 1];

    fn recipe(name: &str) -> Vec<u8> {
        let v = json!({
            "uid": "D818BC90", "name": name, "created": "2012-09-05 13:03:36",
            "servings": "4 servings", "prep_time": "15 min", "cook_time": "1 hr 10 min", "total_time": "",
            "source": "Chefkoch.de", "source_url": "http://www.chefkoch.de/rezepte/317951113441309/Zucchini.html",
            "rating": 4, "on_favorites": 1, "categories": ["Vegetarisch", "Snacks"],
            "ingredients": "400 g Zucchini\n400 g Kartoffel(n)\n\nFor frying:\n2 EL Mehl\nSalz",
            "directions": "1. Zucchini und Kartoffeln grob raffeln.\n2. Eier und Mehl dazugeben,\ngut mischen.\n3. Braten.",
            "notes": "Mit Creme fraiche servieren.",
            "nutritional_info": "Calories: 210\nFat: 9 g",
            "photo_data": base64::engine::general_purpose::STANDARD.encode(JPEG),
            "image_url": "http://cdn.example/x.jpg", "photos": [], "difficulty": "", "hash": "abc"
        });
        gzip(&serde_json::to_vec(&v).unwrap())
    }

    #[test]
    fn paprikarecipes_zip() {
        let a = recipe("Zucchinipuffer");
        let bad = gzip(b"{broken");
        let z = make_zip(&[
            ("My Recipes/Zucchinipuffer.paprikarecipe", &a),
            ("My Recipes/Broken.paprikarecipe", &bad),
            ("__MACOSX/My Recipes/._Zucchinipuffer.paprikarecipe", b"junk"),
        ]);
        let b = parse_file(Source::Auto, "Export.paprikarecipes", &z).unwrap();
        assert_eq!(b.source, Source::Paprika);
        assert_eq!(b.recipes.len(), 1);
        assert_eq!(b.errors.len(), 1);
        assert!(b.errors[0].starts_with("Recipe 'Broken'"), "{:?}", b.errors);
        let r = &b.recipes[0];
        let d = &r.data;
        assert_eq!(d.name, "Zucchinipuffer");
        assert_eq!(d.yield_quantity, Some(4.0));
        assert_eq!((d.prep_time_minutes, d.cook_time_minutes, d.total_time_minutes), (Some(15), Some(70), Some(85)));
        assert_eq!(d.source_name.as_deref(), Some("Chefkoch.de"));
        assert_eq!(d.categories, vec!["Vegetarisch", "Snacks"]);
        assert_eq!(d.ingredients.len(), 4);
        assert_eq!(d.ingredients[0].unit.as_deref(), Some("g"));
        assert_eq!(d.ingredients[2].section.as_deref(), Some("For frying"));
        assert_eq!(d.instructions.len(), 3);
        assert_eq!(d.instructions[1].text, "Eier und Mehl dazugeben,\ngut mischen.");
        assert_eq!(d.nutrition.get("calories").map(String::as_str), Some("210"));
        assert_eq!(d.rating, Some(4));
        assert!(r.favorite);
        assert_eq!(r.created_at.as_deref(), Some("2012-09-05T13:03:36Z"));
        assert!(matches!(r.image, Some(ImageSource::Bytes(ref b)) if b == JPEG));
    }

    #[test]
    fn single_paprikarecipe() {
        let b = parse_file(Source::Auto, "Zucchinipuffer.paprikarecipe", &recipe("Single")).unwrap();
        assert_eq!(b.source, Source::Paprika);
        assert_eq!(b.recipes[0].data.name, "Single");
    }

    #[test]
    fn unnumbered_directions() {
        let s = split_directions("Mix everything.\nBake for 20 minutes.\n\nServe:\nSlice and enjoy.");
        assert_eq!(s.len(), 3);
        assert_eq!(s[2].section.as_deref(), Some("Serve"));
    }
}
