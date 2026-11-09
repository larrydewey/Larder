//! Nextcloud Cookbook (`<Recipe>/recipe.json` + `full.jpg`), also used for any
//! zip of schema.org JSON files.

use serde_json::Value;

use crate::archive::{basename, dirname, join, strip_bom, SafeZip};
use crate::schema_org::{find_recipes, recipe_from_json};
use crate::{Collector, ImageSource, ImportError};

const IMAGE_EXTS: [&str; 5] = ["jpg", "jpeg", "png", "webp", "gif"];

fn sibling_image(zip: &SafeZip, dir: &str) -> Option<String> {
    for stem in ["full", "image", "photo", "cover"] {
        if let Some(n) = IMAGE_EXTS.iter().find_map(|e| zip.find_ci(&join(dir, &format!("{stem}.{e}")))) {
            return Some(n);
        }
    }
    if dir.is_empty() {
        return None;
    }
    // Any other image directly in the recipe folder (but not the thumbnails).
    zip.names()
        .filter(|n| dirname(n) == dir)
        .filter(|n| {
            let b = basename(n).to_ascii_lowercase();
            !b.starts_with("thumb") && IMAGE_EXTS.iter().any(|e| b.ends_with(&format!(".{e}")))
        })
        .map(str::to_string)
        .next()
}

pub(crate) fn parse_zip(zip: &mut SafeZip, c: &mut Collector) -> Result<(), ImportError> {
    let jsons: Vec<String> =
        zip.names().filter(|n| n.to_ascii_lowercase().ends_with(".json") && basename(n) != "manifest.json").map(str::to_string).collect();
    if jsons.is_empty() {
        return Err(ImportError::UnsupportedFormat("no recipe JSON files found in the archive".into()));
    }
    // Several json files in one folder (unusual) — only attach the folder image once per recipe file.
    for name in jsons {
        let dir = dirname(&name).to_string();
        let ctx = if dir.is_empty() { name.clone() } else { basename(&dir).to_string() };
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
        let mut found: Vec<Value> = find_recipes(&v).into_iter().cloned().collect();
        if found.is_empty() && basename(&name) == "recipe.json" && v.get("name").is_some() {
            let mut typed = v.clone();
            typed["@type"] = Value::String("Recipe".into());
            found.push(typed);
        }
        let single = found.len() == 1;
        for r in found {
            let Some(mut rec) = recipe_from_json(&r, None) else { continue };
            if single {
                if let Some(img) = sibling_image(zip, &dir) {
                    match zip.read(&img) {
                        Ok(b) => rec.image = Some(ImageSource::Bytes(b)),
                        Err(e) => tracing::warn!("nextcloud image {img}: {e}"),
                    }
                }
            }
            c.push(rec, &ctx);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::archive::testutil::make_zip;
    use crate::{parse_file, ImageSource, Source};
    use serde_json::json;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n0000";

    #[test]
    fn nextcloud_zip() {
        let shrimp = json!({
            "@context": "http://schema.org", "@type": "Recipe", "name": "Air Fryer Shrimp",
            "description": "Juicy.", "image": "https://www.crunchy.example/shrimp.jpg", "recipeYield": 4,
            "prepTime": "PT0H15M", "cookTime": "PT0H8M", "totalTime": "PT0H23M",
            "recipeIngredient": ["1 lb shrimp", "2 teaspoons olive oil", "½ teaspoon garlic powder"],
            "recipeInstructions": ["Clean the shrimp.", "Mix oil with seasoning."],
            "recipeCategory": "Main Course", "recipeCuisine": ["American"], "keywords": "air fryer shrimp",
            "dateCreated": "2021-01-19T13:29:00+0000",
            "url": "https://www.crunchy.example/air-fryer-shrimp/"
        });
        let pie = json!({"@context": "http://schema.org", "@type": "Recipe", "name": "Shepherd's Pie",
            "recipeIngredient": "1 lb lamb\n2 potatoes", "recipeInstructions": [{"@type": "HowToStep", "text": "Cook."}]});
        let a = serde_json::to_vec(&shrimp).unwrap();
        let p = serde_json::to_vec(&pie).unwrap();
        let z = make_zip(&[
            ("Recipes/Air Fryer Shrimp/recipe.json", &a),
            ("Recipes/Air Fryer Shrimp/full.jpg", PNG),
            ("Recipes/Air Fryer Shrimp/thumb.jpg", b"thumb"),
            ("Recipes/Skillet Shepherd's Pie/recipe.json", &p),
            ("Recipes/Broken/recipe.json", b"{"),
        ]);
        let b = parse_file(Source::Auto, "cookbook.zip", &z).unwrap();
        assert_eq!(b.source, Source::Nextcloud);
        assert_eq!(b.recipes.len(), 2);
        assert_eq!(b.errors.len(), 1);
        let s = b.recipes.iter().find(|r| r.data.name == "Air Fryer Shrimp").unwrap();
        assert!(matches!(s.image, Some(ImageSource::Bytes(ref x)) if x == PNG));
        assert_eq!(s.data.total_time_minutes, Some(23));
        assert_eq!(s.data.yield_quantity, Some(4.0));
        assert_eq!(s.data.tags, vec!["air fryer shrimp", "American"]);
        assert_eq!(s.data.categories, vec!["Main Course"]);
        assert_eq!(s.data.source_url.as_deref(), Some("https://www.crunchy.example/air-fryer-shrimp/"));
        assert_eq!(s.data.source_name.as_deref(), Some("crunchy.example"));
        assert_eq!(s.created_at.as_deref(), Some("2021-01-19T13:29:00Z"));
        let pie = b.recipes.iter().find(|r| r.data.name == "Shepherd's Pie").unwrap();
        assert!(pie.image.is_none());
        assert_eq!(pie.data.ingredients.len(), 2);
    }
}
