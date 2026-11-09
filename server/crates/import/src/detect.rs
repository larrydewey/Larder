//! Format detection and dispatch for [`crate::parse_file`].

use serde_json::Value;

use crate::archive::{basename, is_gzip, is_zip, strip_bom, Budget, SafeZip, MAX_TOTAL_BYTES};
use crate::{larder, mealie, nextcloud, paprika, recipekeeper, schema_org, tandoor, Collector, ImportBatch, ImportError, Source};

/// Largest accepted non-archive upload (JSON / HTML / gzip).
const MAX_PLAIN_BYTES: usize = 100 * 1024 * 1024;

pub(crate) fn parse_file(source: Source, filename: &str, bytes: &[u8]) -> Result<ImportBatch, ImportError> {
    if bytes.is_empty() {
        return Err(ImportError::InvalidFormat("the file is empty".into()));
    }
    let budget = Budget::new(MAX_TOTAL_BYTES);
    let batch = if is_zip(bytes) {
        let mut zip = SafeZip::open(bytes.to_vec(), budget)?;
        let src = if source == Source::Auto { detect_zip(&mut zip)? } else { source };
        let mut c = Collector::new(src);
        match src {
            Source::Mealie => {
                let db = zip.names().find(|n| basename(n) == "database.json").map(str::to_string);
                if let Some(db) = db {
                    mealie::parse_backup_zip(&mut zip, &db, &mut c)?;
                } else {
                    mealie::parse_export_zip(&mut zip, &mut c)?;
                }
            }
            Source::Tandoor => tandoor::parse_zip(&mut zip, &mut c)?,
            Source::Paprika => paprika::parse_zip(&mut zip, &mut c)?,
            Source::Nextcloud | Source::JsonLd => nextcloud::parse_zip(&mut zip, &mut c)?,
            Source::RecipeKeeper => recipekeeper::parse_zip(&mut zip, &mut c)?,
            Source::Larder => larder::parse_zip(&mut zip, &mut c)?,
            Source::Auto => unreachable!("detect_zip never returns Auto"),
        }
        c.finish()
    } else {
        if bytes.len() > MAX_PLAIN_BYTES {
            return Err(ImportError::TooLarge("the file is larger than 100 MB".into()));
        }
        if is_gzip(bytes) {
            if !matches!(source, Source::Auto | Source::Paprika) {
                return Err(ImportError::UnsupportedFormat(format!("a gzip file is not a {} export", source.label())));
            }
            let mut c = Collector::new(Source::Paprika);
            paprika::parse_single(bytes, filename, &budget, &mut c)?;
            c.finish()
        } else {
            parse_text_file(source, filename, bytes)?
        }
    };
    if batch.recipes.is_empty() && batch.errors.is_empty() {
        return Err(ImportError::NoRecipeFound);
    }
    Ok(batch)
}

fn parse_text_file(source: Source, filename: &str, bytes: &[u8]) -> Result<ImportBatch, ImportError> {
    let text = String::from_utf8_lossy(strip_bom(bytes));
    let trimmed = text.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        let v: Value = serde_json::from_str(trimmed)?;
        let src = if source == Source::Auto { detect_json(&v) } else { source };
        let mut c = Collector::new(src);
        match src {
            Source::Larder => larder::parse_json_value(&v, None, &mut c),
            Source::Mealie => mealie::parse_json_value(&v, &mut c),
            Source::Tandoor => tandoor::parse_json_value(&v, &mut c),
            Source::Nextcloud | Source::JsonLd | Source::Auto => parse_jsonld_value(&v, filename, &mut c),
            Source::Paprika => paprika::parse_json_value(&v, &mut c),
            Source::RecipeKeeper => return Err(ImportError::UnsupportedFormat("Recipe Keeper exports are zip or HTML files".into())),
        }
        return Ok(c.finish());
    }
    if trimmed.starts_with('<') {
        let is_rk = text.contains("recipe-details");
        if source == Source::RecipeKeeper || (source == Source::Auto && is_rk) {
            let mut c = Collector::new(Source::RecipeKeeper);
            if recipekeeper::parse_document(&text, &mut |_| None, &mut c) == 0 {
                return Err(ImportError::UnsupportedFormat("no Recipe Keeper recipes found in the HTML file".into()));
            }
            return Ok(c.finish());
        }
        if matches!(source, Source::Auto | Source::JsonLd) {
            let mut c = Collector::new(Source::JsonLd);
            match crate::parse_html(&text, "") {
                Ok(r) => c.push(r, filename),
                Err(e) => c.error(filename, e),
            }
            return Ok(c.finish());
        }
    }
    let ext = filename.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    Err(ImportError::UnsupportedFormat(format!(
        "{} is not a recognised {} file",
        if ext.is_empty() { "this".to_string() } else { format!(".{ext}") },
        if source == Source::Auto { "export".to_string() } else { source.label().to_string() }
    )))
}

/// Guesses the source of a standalone JSON document.
pub(crate) fn detect_json(v: &Value) -> Source {
    let first = match v {
        Value::Array(a) => a.first(),
        Value::Object(o) if o.contains_key("recipes") && !o.contains_key("@type") => {
            v.get("recipes").and_then(|r| r.as_array()).and_then(|a| a.first())
        }
        _ => Some(v),
    };
    let Some(first) = first else { return Source::JsonLd };
    if v.get("format").and_then(|f| f.as_str()).is_some_and(|f| f == larder::FORMAT || f == larder::LEGACY_FORMAT) {
        return Source::Larder;
    }
    if !schema_org::find_recipes(v).is_empty() {
        return Source::JsonLd;
    }
    if first.get("slug").is_some() && (first.get("recipe_ingredient").is_some() || first.get("recipeIngredient").is_some()) {
        return Source::Mealie;
    }
    if first.get("steps").is_some() && (first.get("working_time").is_some() || first.get("keywords").is_some()) {
        return Source::Tandoor;
    }
    if first.get("directions").is_some() && first.get("ingredients").is_some_and(|i| i.is_string()) {
        return Source::Paprika;
    }
    if first.get("id").is_some() && first.get("yield_unit").is_some() {
        return Source::Larder;
    }
    Source::JsonLd
}

/// Imports every schema.org recipe in a JSON document.
pub(crate) fn parse_jsonld_value(v: &Value, context: &str, c: &mut Collector) {
    let recipes = schema_org::find_recipes(v);
    if recipes.is_empty() {
        // Lenient: a single object without @type that looks like a recipe.
        let candidates: Vec<&Value> = match v {
            Value::Array(a) => a.iter().collect(),
            _ => vec![v],
        };
        for r in candidates {
            if r.get("name").is_some() && (r.get("recipeIngredient").is_some() || r.get("recipeInstructions").is_some()) {
                let mut typed = r.clone();
                typed["@type"] = Value::String("Recipe".into());
                if let Some(rec) = schema_org::recipe_from_json(&typed, None) {
                    c.push(rec, context);
                }
            }
        }
        return;
    }
    for r in recipes {
        match schema_org::recipe_from_json(r, None) {
            Some(rec) => c.push(rec, context),
            None => c.error(context, "not a schema.org Recipe"),
        }
    }
}

fn detect_zip(zip: &mut SafeZip) -> Result<Source, ImportError> {
    let names: Vec<String> = zip.names().map(str::to_string).collect();
    let has_base = |b: &str| names.iter().any(|n| basename(n).eq_ignore_ascii_case(b));
    let has_ext = |e: &str| names.iter().any(|n| n.to_ascii_lowercase().ends_with(e));
    if has_base("database.json") {
        return Ok(Source::Mealie);
    }
    if let Some(m) = names.iter().find(|n| basename(n) == "manifest.json").cloned() {
        if zip.read_string(&m).is_ok_and(|s| s.contains(larder::FORMAT) || s.contains(larder::LEGACY_FORMAT)) {
            return Ok(Source::Larder);
        }
    }
    if names.iter().any(|n| n == "recipes.json") {
        return Ok(Source::Larder);
    }
    if let Some(h) = names.iter().find(|n| basename(n).eq_ignore_ascii_case("recipes.html")).cloned() {
        if zip.read_string(&h).is_ok_and(|s| s.contains("recipe-details")) {
            return Ok(Source::RecipeKeeper);
        }
        return Err(ImportError::UnsupportedFormat(
            "recipes.html is not a Recipe Keeper export (other apps' HTML exports are not supported)".into(),
        ));
    }
    if has_ext(".paprikarecipe") {
        return Ok(Source::Paprika);
    }
    if has_ext(".zip") {
        return Ok(Source::Tandoor);
    }
    if names.iter().any(|n| mealie::is_export_json(n)) {
        return Ok(Source::Mealie);
    }
    if let Some(r) = names.iter().find(|n| basename(n) == "recipe.json").cloned() {
        let v: Option<Value> = zip.read(&r).ok().and_then(|b| serde_json::from_slice(strip_bom(&b)).ok());
        if v.as_ref().is_some_and(|v| v.get("steps").is_some() && v.get("@type").is_none()) {
            return Ok(Source::Tandoor);
        }
        return Ok(Source::Nextcloud);
    }
    if has_ext(".json") {
        return Ok(Source::JsonLd);
    }
    Err(ImportError::UnsupportedFormat("the archive does not contain a known recipe export".into()))
}
