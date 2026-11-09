//! Mealie: recipe export zips (`recipes/<slug>/<slug>.json` + `images/original.webp`),
//! backup zips (`database.json` + `data/recipes/<id>/images/original.webp`) and
//! recipe JSON as returned by the API (camelCase) or written to exports (snake_case).

use std::collections::{HashMap, HashSet};

use larder_core::{Ingredient, Instruction, RecipeData};
use serde_json::{json, Map, Value};

use crate::archive::{basename, dirname, strip_bom, SafeZip};
use crate::schema_org::{nutrition_from, parse_iso8601_duration};
use crate::util::{as_bool, as_f64, as_text, clean_block, clean_line, get, name_list, parse_yield_text, text_of, to_rfc3339};
use crate::{Collector, ImageSource, ImportError, ImportedRecipe};

/// True for `.../<slug>/<slug>.json`, the layout of Mealie's recipe exporter.
pub(crate) fn is_export_json(name: &str) -> bool {
    let Some(stem) = basename(name).strip_suffix(".json") else { return false };
    !stem.is_empty() && basename(dirname(name)) == stem
}

fn find_image(zip: &SafeZip, dir: &str) -> Option<String> {
    let prefs = ["original.webp", "original.jpg", "original.jpeg", "original.png", "min-original.webp"];
    for p in prefs {
        let cand = format!("{dir}/images/{p}");
        if let Some(n) = zip.find(&cand) {
            return Some(n.to_string());
        }
    }
    let prefix = format!("{dir}/images/");
    zip.names()
        .filter(|n| n.starts_with(&prefix) && !basename(n).starts_with("tiny-"))
        .find(|n| crate::archive::basename(n).contains("original"))
        .map(str::to_string)
}

pub(crate) fn parse_export_zip(zip: &mut SafeZip, c: &mut Collector) -> Result<(), ImportError> {
    let jsons: Vec<String> = zip.names().filter(|n| is_export_json(n)).map(str::to_string).collect();
    if jsons.is_empty() {
        return Err(ImportError::UnsupportedFormat("no Mealie recipes found in the archive".into()));
    }
    for name in jsons {
        let ctx = basename(&name).trim_end_matches(".json").to_string();
        let v: Value = match zip
            .read(&name)
            .map_err(|e| e.to_string())
            .and_then(|b| serde_json::from_slice(strip_bom(&b)).map_err(|e| format!("invalid JSON: {e}")))
        {
            Ok(v) => v,
            Err(e) => {
                c.error(&ctx, e);
                continue;
            }
        };
        let mut r = recipe_from_mealie_json(&v);
        if let Some(img) = find_image(zip, dirname(&name)) {
            match zip.read(&img) {
                Ok(b) => r.image = Some(ImageSource::Bytes(b)),
                Err(e) => tracing::warn!("mealie image {img}: {e}"),
            }
        }
        c.push(r, &ctx);
    }
    Ok(())
}

/// A standalone Mealie recipe JSON (single object, array or paginated `{items}`).
pub(crate) fn parse_json_value(v: &Value, c: &mut Collector) {
    let items: Vec<&Value> = match v {
        Value::Array(a) => a.iter().collect(),
        Value::Object(o) if o.get("items").is_some_and(Value::is_array) => o["items"].as_array().unwrap().iter().collect(),
        _ => vec![v],
    };
    for (i, item) in items.into_iter().enumerate() {
        c.push(recipe_from_mealie_json(item), &format!("#{}", i + 1));
    }
}

fn unit_name(u: &Value) -> Option<String> {
    match u {
        Value::Object(_) => {
            let name = text_of(u, &["name"]);
            let abbr = text_of(u, &["abbreviation"]);
            let known = |s: &Option<String>| s.as_deref().and_then(larder_core::units::canonical_unit).is_some();
            if !known(&name) && known(&abbr) {
                abbr
            } else {
                name.or(abbr)
            }
        }
        _ => as_text(u),
    }
}

fn starts_numeric(s: &str) -> bool {
    s.trim_start().chars().next().is_some_and(|c| c.is_ascii_digit() || "½¼¾⅓⅔⅛⅜⅝⅞⅕⅖⅗⅘⅙⅚⁰¹²³⁴⁵⁶⁷⁸⁹".contains(c))
}

/// One Mealie ingredient (works for both API/export objects and rebuilt backup rows).
fn mealie_ingredient(v: &Value, disable_amount: bool) -> Ingredient {
    if let Value::String(s) = v {
        return Ingredient { original: clean_line(s), ..Default::default() };
    }
    let note = text_of(v, &["note"]).map(|s| clean_line(&s)).filter(|s| !s.is_empty());
    let unit = get(v, &["unit"]).and_then(unit_name);
    let food = get(v, &["food"]).and_then(as_text).map(|s| clean_line(&s));
    let qty = get(v, &["quantity"]).and_then(as_f64).filter(|q| *q > 0.0);
    let display = text_of(v, &["display"]).map(|s| clean_line(&s)).filter(|s| !s.is_empty());
    let original_text = text_of(v, &["original_text", "originalText"]).map(|s| clean_line(&s));
    let disable = disable_amount || get(v, &["disable_amount", "disableAmount"]).is_some_and(as_bool);

    if !disable && (food.is_some() || unit.is_some()) {
        let (food, note) = if food.is_none() { (note, None) } else { (food, note) };
        return Ingredient { quantity: qty, unit, food, note, original: display.unwrap_or_default(), ..Default::default() };
    }
    // Unparsed Mealie ingredients keep the whole line in `note` and carry a
    // default quantity of 0 or 1 that Mealie also bakes into `display`
    // ("1 1⁄2 cup butter" for a note of "1⁄2 cup butter"). Prefer the note
    // and only prepend a quantity that cannot be that default.
    let line = if disable {
        note.or(original_text).or(display)
    } else {
        note.map(|n| match qty {
            Some(q) if (q - 1.0).abs() > f64::EPSILON && !starts_numeric(&n) => {
                format!("{} {n}", larder_core::format::format_quantity(q))
            }
            _ => n,
        })
        .or(original_text)
        .or(display)
    };
    Ingredient { original: line.unwrap_or_default(), ..Default::default() }
}

/// Maps a Mealie recipe (snake_case export/backup or camelCase API) to an
/// [`ImportedRecipe`] (data normalised, no image).
pub fn recipe_from_mealie_json(v: &Value) -> ImportedRecipe {
    let mut d = RecipeData {
        name: text_of(v, &["name"]).map(|s| clean_line(&s)).unwrap_or_default(),
        description: text_of(v, &["description"]).map(|s| clean_block(&s)).unwrap_or_default(),
        ..Default::default()
    };

    let servings = get(v, &["recipe_servings", "recipeServings"]).and_then(as_f64).filter(|f| *f > 0.0);
    let yq = get(v, &["recipe_yield_quantity", "recipeYieldQuantity"]).and_then(as_f64).filter(|f| *f > 0.0);
    let ytext = text_of(v, &["recipe_yield", "recipeYield"]).map(|s| clean_line(&s)).unwrap_or_default();
    if let Some(s) = servings {
        d.yield_quantity = Some(s);
        d.yield_unit = "servings".into();
    } else if let Some(q) = yq {
        d.yield_quantity = Some(q);
        let (pq, pu) = parse_yield_text(&ytext);
        d.yield_unit = if pq.is_some() {
            pu
        } else if ytext.is_empty() {
            "servings".into()
        } else {
            ytext.clone()
        };
    } else if !ytext.is_empty() {
        let (q, u) = parse_yield_text(&ytext);
        d.yield_quantity = q;
        d.yield_unit = u;
    }

    let dur = |keys: &[&str]| text_of(v, keys).and_then(|s| parse_iso8601_duration(&s));
    d.prep_time_minutes = dur(&["prep_time", "prepTime"]);
    d.cook_time_minutes = dur(&["perform_time", "performTime", "cook_time", "cookTime"]);
    d.total_time_minutes = dur(&["total_time", "totalTime"]);
    d.source_url = text_of(v, &["org_url", "orgURL", "orgUrl"]).filter(|u| u.starts_with("http"));
    d.rating = get(v, &["rating"]).and_then(as_f64).map(|r| r.round() as i64);

    let disable_amount = get(v, &["settings"]).and_then(|s| get(s, &["disable_amount", "disableAmount"])).is_some_and(as_bool);
    let mut section: Option<String> = None;
    if let Some(Value::Array(ings)) = get(v, &["recipe_ingredient", "recipeIngredient"]) {
        for ing in ings {
            if let Some(t) = text_of(ing, &["title"]).map(|s| clean_line(&s)).filter(|s| !s.is_empty()) {
                section = Some(t);
            }
            let mut i = mealie_ingredient(ing, disable_amount);
            i.section = section.clone();
            d.ingredients.push(i);
        }
    }
    let mut section: Option<String> = None;
    if let Some(Value::Array(steps)) = get(v, &["recipe_instructions", "recipeInstructions"]) {
        for s in steps {
            if let Some(t) = text_of(s, &["title"]).map(|s| clean_line(&s)).filter(|s| !s.is_empty()) {
                section = Some(t);
            }
            let text = match s {
                Value::String(t) => clean_block(t),
                _ => text_of(s, &["text"]).map(|t| clean_block(&t)).unwrap_or_default(),
            };
            d.instructions.push(Instruction { section: section.clone(), text });
        }
    }
    if let Some(t) = get(v, &["tags"]) {
        d.tags.extend(name_list(t));
    }
    if let Some(t) = get(v, &["recipeCuisine", "recipe_cuisine"]) {
        d.tags.extend(name_list(t));
    }
    if let Some(t) = get(v, &["recipe_category", "recipeCategory"]) {
        d.categories.extend(name_list(t));
    }
    if let Some(n) = get(v, &["nutrition"]) {
        nutrition_from(n, &mut d.nutrition);
    }
    let mut notes: Vec<String> = vec![];
    if let Some(Value::Array(ns)) = get(v, &["notes"]) {
        for n in ns {
            let title = text_of(n, &["title"]).map(|s| clean_line(&s)).unwrap_or_default();
            let text = text_of(n, &["text"]).map(|s| clean_block(&s)).unwrap_or_default();
            let block = match (title.is_empty(), text.is_empty()) {
                (false, false) => format!("{title}\n{text}"),
                (false, true) => title,
                (true, false) => text,
                (true, true) => continue,
            };
            notes.push(block);
        }
    } else if let Some(n) = text_of(v, &["notes"]) {
        notes.push(clean_block(&n));
    }
    if let Some(Value::Object(extras)) = get(v, &["extras"]) {
        let lines: Vec<String> = extras.iter().filter_map(|(k, val)| as_text(val).map(|t| format!("{k}: {}", clean_line(&t)))).collect();
        if !lines.is_empty() {
            notes.push(lines.join("\n"));
        }
    }
    d.notes = notes.join("\n\n");
    d.normalize();

    let created_at = text_of(v, &["created_at", "createdAt", "date_added", "dateAdded"]).and_then(|s| to_rfc3339(&s));
    ImportedRecipe { data: d, image: None, created_at, favorite: false }
}

// ---------------------------------------------------------------------------
// Backups
// ---------------------------------------------------------------------------

fn table<'a>(db: &'a Value, name: &str) -> &'a [Value] {
    db.get(name).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

fn id_of(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(as_text).map(|s| s.to_ascii_lowercase().replace('-', ""))
}

fn group_by<'a>(rows: &'a [Value], key: &str) -> HashMap<String, Vec<&'a Value>> {
    let mut m: HashMap<String, Vec<&Value>> = HashMap::new();
    for r in rows {
        if let Some(k) = id_of(r, key) {
            m.entry(k).or_default().push(r);
        }
    }
    for v in m.values_mut() {
        v.sort_by(|a, b| {
            let pa = a.get("position").and_then(as_f64).unwrap_or(0.0);
            let pb = b.get("position").and_then(as_f64).unwrap_or(0.0);
            pa.total_cmp(&pb)
        });
    }
    m
}

fn index_by<'a>(rows: &'a [Value], key: &str) -> HashMap<String, &'a Value> {
    rows.iter().filter_map(|r| Some((id_of(r, key)?, r))).collect()
}

pub(crate) fn parse_backup_zip(zip: &mut SafeZip, db_name: &str, c: &mut Collector) -> Result<(), ImportError> {
    let bytes = zip.read(db_name)?;
    let db: Value = serde_json::from_slice(strip_bom(&bytes))?;
    drop(bytes);
    let recipes = table(&db, "recipes");
    if recipes.is_empty() {
        return Err(ImportError::UnsupportedFormat("the Mealie backup contains no recipes".into()));
    }
    let units = index_by(table(&db, "ingredient_units"), "id");
    let foods = index_by(table(&db, "ingredient_foods"), "id");
    let tags = index_by(table(&db, "tags"), "id");
    let cats = index_by(table(&db, "categories"), "id");
    let ingredients = group_by(table(&db, "recipes_ingredients"), "recipe_id");
    let steps = group_by(table(&db, "recipe_instructions"), "recipe_id");
    let r2t = group_by(table(&db, "recipes_to_tags"), "recipe_id");
    let r2c = group_by(table(&db, "recipes_to_categories"), "recipe_id");
    let nutrition = index_by(table(&db, "recipe_nutrition"), "recipe_id");
    let settings = index_by(table(&db, "recipe_settings"), "recipe_id");
    let notes = group_by(table(&db, "notes"), "recipe_id");
    let user_recipes = group_by(table(&db, "users_to_recipes"), "recipe_id");
    let favorites: HashSet<String> = table(&db, "users_to_favorites").iter().filter_map(|r| id_of(r, "recipe_id")).collect();
    let names: Vec<String> = zip.names().map(str::to_string).collect();

    for row in recipes {
        let Some(id) = id_of(row, "id") else { continue };
        let ctx = text_of(row, &["name", "slug"]).unwrap_or_else(|| id.clone());
        let mut obj: Map<String, Value> = row.as_object().cloned().unwrap_or_default();

        let ings: Vec<Value> = ingredients
            .get(&id)
            .into_iter()
            .flatten()
            .map(|i| {
                let unit = id_of(i, "unit_id").and_then(|u| units.get(&u)).map(|u| (*u).clone()).unwrap_or(Value::Null);
                let food = id_of(i, "food_id").and_then(|f| foods.get(&f)).map(|f| (*f).clone()).unwrap_or(Value::Null);
                json!({
                    "title": i.get("title"), "note": i.get("note"), "quantity": i.get("quantity"),
                    "original_text": i.get("original_text"), "unit": unit, "food": food,
                })
            })
            .collect();
        obj.insert("recipe_ingredient".into(), Value::Array(ings));
        let ins: Vec<Value> =
            steps.get(&id).into_iter().flatten().map(|s| json!({"title": s.get("title"), "text": s.get("text")})).collect();
        obj.insert("recipe_instructions".into(), Value::Array(ins));
        let tag_names: Vec<Value> = r2t
            .get(&id)
            .into_iter()
            .flatten()
            .filter_map(|l| tags.get(&id_of(l, "tag_id")?).map(|t| json!({"name": t.get("name")})))
            .collect();
        obj.insert("tags".into(), Value::Array(tag_names));
        let cat_names: Vec<Value> = r2c
            .get(&id)
            .into_iter()
            .flatten()
            .filter_map(|l| cats.get(&id_of(l, "category_id")?).map(|t| json!({"name": t.get("name")})))
            .collect();
        obj.insert("recipe_category".into(), Value::Array(cat_names));
        if let Some(n) = nutrition.get(&id) {
            let mut n = (*n).clone();
            if let Some(o) = n.as_object_mut() {
                for k in ["id", "recipe_id", "created_at", "update_at", "updated_at"] {
                    o.remove(k);
                }
            }
            obj.insert("nutrition".into(), n);
        }
        if let Some(s) = settings.get(&id) {
            obj.insert("settings".into(), (*s).clone());
        }
        let ns: Vec<Value> = notes.get(&id).into_iter().flatten().map(|n| (*n).clone()).collect();
        obj.insert("notes".into(), Value::Array(ns));

        let user_rows = user_recipes.get(&id).map(Vec::as_slice).unwrap_or(&[]);
        if obj.get("rating").and_then(as_f64).is_none_or(|r| r <= 0.0) {
            let best = user_rows.iter().filter_map(|u| u.get("rating").and_then(as_f64)).fold(0.0, f64::max);
            if best > 0.0 {
                obj.insert("rating".into(), json!(best));
            }
        }
        let favorite = favorites.contains(&id) || user_rows.iter().any(|u| u.get("is_favorite").is_some_and(as_bool));

        let mut r = recipe_from_mealie_json(&Value::Object(obj));
        r.favorite = favorite;
        let raw_id = text_of(row, &["id"]).unwrap_or_default();
        if let Some(img) = backup_image(&names, &raw_id, &id) {
            match zip.read(&img) {
                Ok(b) => r.image = Some(ImageSource::Bytes(b)),
                Err(e) => tracing::warn!("mealie backup image {img}: {e}"),
            }
        }
        c.push(r, &ctx);
    }
    Ok(())
}

fn backup_image(names: &[String], raw_id: &str, compact_id: &str) -> Option<String> {
    let dashed = if compact_id.len() == 32 {
        format!("{}-{}-{}-{}-{}", &compact_id[..8], &compact_id[8..12], &compact_id[12..16], &compact_id[16..20], &compact_id[20..])
    } else {
        raw_id.to_ascii_lowercase()
    };
    for id in [raw_id.to_string(), dashed, compact_id.to_string()] {
        if id.is_empty() {
            continue;
        }
        for file in ["original.webp", "original.jpg", "original.png", "min-original.webp"] {
            let suffix = format!("recipes/{id}/images/{file}");
            if let Some(n) = names.iter().find(|n| n.ends_with(&suffix)) {
                return Some(n.clone());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::testutil::make_zip;
    use crate::{parse_file, Source};

    const WEBP: &[u8] = b"RIFF\x10\x00\x00\x00WEBPVP8 rest";

    fn export_recipe() -> Value {
        json!({
            "id": "6f1b6b1e-0000-4000-8000-000000000001",
            "slug": "lemon-bars",
            "group_id": "g",
            "name": "Lemon Bars",
            "description": "Tangy &amp; sweet",
            "recipe_servings": 0,
            "recipe_yield_quantity": 16,
            "recipe_yield": "bars",
            "prep_time": "20 minutes",
            "perform_time": "PT45M",
            "total_time": "1 Hour 5 Minutes",
            "org_url": "https://www.example.com/lemon-bars",
            "rating": 4,
            "date_added": "2024-01-02",
            "created_at": "2024-01-02T10:11:12.123456",
            "recipe_category": [{"name": "Dessert", "slug": "dessert"}],
            "tags": [{"name": "Citrus"}, {"name": "Baking"}],
            "tools": [{"name": "9x13 pan"}],
            "recipe_ingredient": [
                {"title": "Crust", "note": "softened", "quantity": 1.0,
                 "unit": {"name": "cup", "abbreviation": "c"}, "food": {"name": "butter"},
                 "display": "1 cup butter softened", "reference_id": "r1"},
                {"title": "", "note": "", "quantity": 2.0, "unit": {"name": "cup"}, "food": {"name": "flour"},
                 "display": "", "reference_id": "r2"},
                {"title": "Filling", "note": "4 large eggs", "quantity": 0, "unit": null, "food": null,
                 "display": "4 large eggs", "reference_id": "r3"},
                {"note": "zest of 2 lemons", "quantity": 1, "disable_amount": true, "unit": null, "food": null}
            ],
            "recipe_instructions": [
                {"title": "Crust", "text": "Press into pan."},
                {"title": "", "text": "Bake 20 minutes."},
                {"title": "Filling", "text": "<p>Whisk &amp; pour.</p>"}
            ],
            "nutrition": {"calories": "180", "fat_content": "9", "protein_content": null},
            "notes": [{"title": "Tip", "text": "Dust with sugar."}],
            "extras": {"source_book": "Grandma"}
        })
    }

    #[test]
    fn maps_recipe_json() {
        let r = recipe_from_mealie_json(&export_recipe());
        let d = &r.data;
        assert_eq!(d.name, "Lemon Bars");
        assert_eq!(d.description, "Tangy & sweet");
        assert_eq!((d.yield_quantity, d.yield_unit.as_str()), (Some(16.0), "bars"));
        assert_eq!((d.prep_time_minutes, d.cook_time_minutes, d.total_time_minutes), (Some(20), Some(45), Some(65)));
        assert_eq!(d.source_url.as_deref(), Some("https://www.example.com/lemon-bars"));
        assert_eq!(d.rating, Some(4));
        assert_eq!(d.tags, vec!["Citrus", "Baking"]);
        assert_eq!(d.categories, vec!["Dessert"]);
        assert_eq!(d.ingredients.len(), 4);
        let i0 = &d.ingredients[0];
        assert_eq!(
            (i0.quantity, i0.unit.as_deref(), i0.food.as_deref(), i0.note.as_deref()),
            (Some(1.0), Some("cup"), Some("butter"), Some("softened"))
        );
        assert_eq!(i0.section.as_deref(), Some("Crust"));
        assert_eq!(d.ingredients[1].original, "2 cups flour");
        assert_eq!(d.ingredients[1].section.as_deref(), Some("Crust"));
        assert_eq!(d.ingredients[2].quantity, Some(4.0));
        assert_eq!(d.ingredients[2].section.as_deref(), Some("Filling"));
        assert_eq!(d.ingredients[3].original, "zest of 2 lemons");
        assert_eq!(d.instructions[1].section.as_deref(), Some("Crust"));
        assert_eq!(d.instructions[2].text, "Whisk & pour.");
        assert_eq!(d.nutrition.get("fatContent").map(String::as_str), Some("9"));
        assert!(d.notes.contains("Tip\nDust with sugar."));
        assert!(d.notes.contains("source_book: Grandma"));
        assert_eq!(r.created_at.as_deref(), Some("2024-01-02T10:11:12Z"));
    }

    #[test]
    fn camel_case_api_shape() {
        let v = json!({
            "name": "Soup", "slug": "soup", "recipeServings": 4, "recipeYield": "4 servings",
            "orgURL": "https://soup.example/x", "prepTime": "PT10M", "performTime": "30 minutes",
            "recipeIngredient": [{"note": "1 onion", "quantity": 1, "unit": null, "food": null, "display": "1 onion"}],
            "recipeInstructions": [{"text": "Cook."}], "recipeCategory": [{"name": "Soups"}],
            "nutrition": {"calories": "100", "fatContent": "2"}, "createdAt": "2024-05-01T00:00:00Z"
        });
        let r = recipe_from_mealie_json(&v);
        assert_eq!(r.data.yield_quantity, Some(4.0));
        assert_eq!(r.data.cook_time_minutes, Some(30));
        assert_eq!(r.data.ingredients[0].food.as_deref(), Some("onion"));
        assert_eq!(r.data.categories, vec!["Soups"]);
        assert_eq!(r.data.nutrition.get("fatContent").map(String::as_str), Some("2"));
    }

    #[test]
    fn old_mealie_string_tags() {
        let v = json!({"name": "Cookies", "slug": "cookies", "recipe_category": ["Dessert", "Cookies"], "tags": [],
            "recipe_yield": "", "recipe_ingredient": [{"title": null, "note": "1.5 cups (230g) cake flour  ", "unit": null,
            "food": null, "disable_amount": true, "quantity": 1}], "recipe_instructions": [{"text": "Mix"}]});
        let r = recipe_from_mealie_json(&v);
        assert_eq!(r.data.categories, vec!["Dessert", "Cookies"]);
        assert_eq!(r.data.ingredients[0].quantity, Some(1.5));
        assert_eq!(r.data.ingredients[0].food.as_deref(), Some("cake flour"));
    }

    #[test]
    fn export_zip() {
        let j = serde_json::to_vec(&export_recipe()).unwrap();
        let z = make_zip(&[
            ("recipes/lemon-bars/lemon-bars.json", &j),
            ("recipes/lemon-bars/images/tiny-original.webp", WEBP),
            ("recipes/lemon-bars/images/original.webp", WEBP),
            ("recipes/broken/broken.json", b"{not json"),
        ]);
        let b = parse_file(Source::Auto, "mealie_export.zip", &z).unwrap();
        assert_eq!(b.source, Source::Mealie);
        assert_eq!(b.recipes.len(), 1);
        assert_eq!(b.errors.len(), 1);
        assert!(b.errors[0].starts_with("Recipe 'broken'"));
        assert!(matches!(b.recipes[0].image, Some(ImageSource::Bytes(ref x)) if x == WEBP));
        assert_eq!(b.recipes[0].data.source_name.as_deref(), Some("example.com"));
    }

    #[test]
    fn backup_zip() {
        let rid = "54d61f19-632d-4b90-908e-f0bc9944c9d0";
        let db = json!({
            "alembic_version": [{"version_num": "bcfdad6b7355"}],
            "recipes": [
                {"id": rid, "slug": "pasta", "name": "Pasta", "description": "Simple", "recipe_yield": "2 servings",
                 "prep_time": "5 minutes", "perform_time": "10 minutes", "total_time": null, "org_url": null,
                 "rating": null, "created_at": "2022-12-05T22:33:16.445964", "date_added": "2022-12-05"},
                {"id": "11111111-2222-3333-4444-555555555555", "slug": "noname", "name": ""}
            ],
            "ingredient_units": [{"id": "u1", "name": "gram", "abbreviation": "g"}],
            "ingredient_foods": [{"id": "f1", "name": "spaghetti"}],
            "recipes_ingredients": [
                {"id": 2, "position": 1, "recipe_id": rid, "title": null, "note": "grated parmesan", "unit_id": null,
                 "food_id": null, "quantity": 0, "original_text": null},
                {"id": 1, "position": 0, "recipe_id": rid, "title": "Pasta", "note": "", "unit_id": "u1",
                 "food_id": "f1", "quantity": 200.0, "original_text": "200g spaghetti"}
            ],
            "recipe_instructions": [
                {"id": "s2", "recipe_id": rid, "position": 1, "title": "", "text": "Drain."},
                {"id": "s1", "recipe_id": rid, "position": 0, "title": "", "text": "Boil water."}
            ],
            "tags": [{"id": "t1", "name": "Quick"}],
            "categories": [{"id": "c1", "name": "Dinner"}],
            "recipes_to_tags": [{"recipe_id": rid, "tag_id": "t1"}],
            "recipes_to_categories": [{"recipe_id": rid, "category_id": "c1"}],
            "recipe_nutrition": [{"id": 1, "recipe_id": rid, "calories": "400", "fat_content": null}],
            "recipe_settings": [{"recipe_id": rid, "disable_amount": false}],
            "notes": [{"id": 1, "recipe_id": rid, "title": "", "text": "Salt the water."}],
            "users_to_recipes": [{"user_id": "x", "recipe_id": rid, "rating": 5, "is_favorite": true}]
        });
        let dbj = serde_json::to_vec(&db).unwrap();
        let img_path = format!("data/recipes/{rid}/images/original.webp");
        let z = make_zip(&[("database.json", &dbj), (&img_path, WEBP), ("data/.PLACEHOLDER", b"")]);
        let b = parse_file(Source::Auto, "mealie_backup.zip", &z).unwrap();
        assert_eq!(b.source, Source::Mealie);
        assert_eq!(b.recipes.len(), 1, "{:?}", b.errors);
        assert_eq!(b.errors.len(), 1);
        let r = &b.recipes[0];
        let d = &r.data;
        assert_eq!(d.name, "Pasta");
        assert_eq!(d.yield_quantity, Some(2.0));
        assert_eq!(d.total_time_minutes, Some(15));
        assert_eq!(d.ingredients[0].food.as_deref(), Some("spaghetti"));
        assert_eq!(d.ingredients[0].unit.as_deref(), Some("g"));
        assert_eq!(d.ingredients[0].section.as_deref(), Some("Pasta"));
        assert_eq!(d.ingredients[1].original, "Parmesan, grated");
        assert_eq!(d.instructions[0].text, "Boil water.");
        assert_eq!(d.tags, vec!["Quick"]);
        assert_eq!(d.categories, vec!["Dinner"]);
        assert_eq!(d.nutrition.get("calories").map(String::as_str), Some("400"));
        assert_eq!(d.notes, "Salt the water.");
        assert_eq!(d.rating, Some(5));
        assert_eq!(d.source_name.as_deref(), Some("Mealie"));
        assert!(r.favorite);
        assert!(r.image.is_some());
        assert_eq!(r.created_at.as_deref(), Some("2022-12-05T22:33:16Z"));
    }

    #[test]
    fn unparsed_lines_ignore_default_quantity() {
        // Shape seen in real Mealie exports: unparsed lines keep the text in
        // `note`, carry a default quantity of 1 or 0, and `display` has the
        // default baked in ("1 1⁄2 cup butter").
        let v = json!({"name": "Defaults", "recipe_ingredient": [
            {"quantity": 1.0, "unit": null, "food": null, "note": "1⁄2\tcup\tbutter\t", "display": "1 1⁄2\tcup\tbutter"},
            {"quantity": 1.0, "unit": null, "food": null, "note": "Salt and pepper", "display": "1 Salt and pepper"},
            {"quantity": 0.0, "unit": null, "food": null, "note": "2 cups flour", "display": "2 cups flour"},
            {"quantity": 2.0, "unit": null, "food": null, "note": "onions, diced", "display": "2 onions, diced"},
            {"quantity": 1.0, "unit": null, "food": null, "note": "1 3⁄4 cups sugar", "display": "1 1 3⁄4 cups sugar"}
        ], "recipe_instructions": [{"text": "Mix."}]});
        let r = recipe_from_mealie_json(&v);
        let ings = &r.data.ingredients;
        assert_eq!(ings[0].original, "1/2 cup butter");
        assert_eq!((ings[0].quantity, ings[0].unit.as_deref()), (Some(0.5), Some("cup")));
        assert_eq!(ings[1].original, "salt and pepper");
        assert_eq!(ings[1].quantity, None);
        assert_eq!(ings[2].quantity, Some(2.0));
        assert_eq!(ings[3].original, "2 onions, diced");
        assert_eq!(ings[4].quantity, Some(1.75));
    }
}
