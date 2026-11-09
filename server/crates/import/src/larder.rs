//! Larder's own export format: a zip with `manifest.json`
//! (`{"format": "larder-export", "version": 1}`), `recipes.json` (array of
//! [`ExportedRecipe`]) and `images/<id>.<ext>`.

use std::io::{Cursor, Write};

use larder_core::RecipeData;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use zip::write::SimpleFileOptions;

use crate::archive::{dirname, resolve_relative, strip_bom, SafeZip};
use crate::{image_extension, Collector, ImageSource, ImportError, ImportedRecipe};

/// Value of `format` in `manifest.json`.
pub const FORMAT: &str = "larder-export";
/// Manifest format written by versions released as "Recipe-rs" (still accepted).
pub const LEGACY_FORMAT: &str = "recipers-export";
/// Current export format version.
pub const VERSION: u32 = 1;

/// One recipe in `recipes.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportedRecipe {
    pub id: String,
    #[serde(flatten)]
    pub data: RecipeData,
    /// Path of the image inside the zip (e.g. `images/<id>.jpg`).
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Serialize)]
struct Manifest<'a> {
    format: &'a str,
    version: u32,
    exported_at: String,
    recipe_count: usize,
    generator: &'a str,
}

fn safe_file_stem(id: &str) -> String {
    let s: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).take(100).collect();
    if s.is_empty() {
        "recipe".into()
    } else {
        s
    }
}

/// Writes a Larder export zip. Each item's `image` field is overwritten
/// with the path of the stored image (or `None` when no bytes are given).
pub fn write_export(items: Vec<(ExportedRecipe, Option<Vec<u8>>)>) -> Result<Vec<u8>, ImportError> {
    let zerr = |e: zip::result::ZipError| ImportError::InvalidArchive(e.to_string());
    let ioerr = |e: std::io::Error| ImportError::InvalidArchive(e.to_string());
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let deflate = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    let manifest = Manifest {
        format: FORMAT,
        version: VERSION,
        exported_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        recipe_count: items.len(),
        generator: concat!("Larder ", env!("CARGO_PKG_VERSION")),
    };
    w.start_file("manifest.json", deflate).map_err(zerr)?;
    w.write_all(&serde_json::to_vec_pretty(&manifest).map_err(ImportError::from)?).map_err(ioerr)?;

    let mut recipes = Vec::with_capacity(items.len());
    let mut used = std::collections::HashSet::new();
    for (mut rec, bytes) in items {
        rec.image = None;
        if let Some(bytes) = bytes.filter(|b| !b.is_empty()) {
            let ext = image_extension(&bytes).unwrap_or("jpg");
            let mut stem = safe_file_stem(&rec.id);
            let mut n = 1;
            while !used.insert(stem.clone()) {
                n += 1;
                stem = format!("{}-{n}", safe_file_stem(&rec.id));
            }
            let path = format!("images/{stem}.{ext}");
            // Images are already compressed.
            w.start_file(path.as_str(), stored).map_err(zerr)?;
            w.write_all(&bytes).map_err(ioerr)?;
            rec.image = Some(path);
        }
        recipes.push(rec);
    }
    w.start_file("recipes.json", deflate).map_err(zerr)?;
    w.write_all(&serde_json::to_vec_pretty(&recipes).map_err(ImportError::from)?).map_err(ioerr)?;
    Ok(w.finish().map_err(zerr)?.into_inner())
}

pub(crate) fn parse_zip(zip: &mut SafeZip, c: &mut Collector) -> Result<(), ImportError> {
    if let Some(m) = zip.find("manifest.json").map(str::to_string) {
        let v: Value = serde_json::from_slice(strip_bom(&zip.read(&m)?))?;
        let fmt = v.get("format").and_then(Value::as_str).unwrap_or(FORMAT);
        if fmt != FORMAT && fmt != LEGACY_FORMAT {
            return Err(ImportError::UnsupportedFormat(format!("unknown export format '{fmt}'")));
        }
        if let Some(ver) = v.get("version").and_then(Value::as_u64) {
            if ver > VERSION as u64 {
                return Err(ImportError::UnsupportedFormat(format!("Larder export version {ver} is newer than supported")));
            }
        }
    }
    let name = zip
        .names()
        .find(|n| *n == "recipes.json" || n.ends_with("/recipes.json"))
        .map(str::to_string)
        .ok_or_else(|| ImportError::UnsupportedFormat("recipes.json not found in the archive".into()))?;
    let v: Value = serde_json::from_slice(strip_bom(&zip.read(&name)?))?;
    let base = dirname(&name).to_string();
    let mut load = |path: &str| -> Option<Vec<u8>> {
        let p = resolve_relative(&base, path)?;
        let n = zip.find_ci(&p)?;
        zip.read(&n).ok()
    };
    parse_json_value(&v, Some(&mut load), c);
    Ok(())
}

/// Loads a file referenced from `recipes.json`.
pub(crate) type ImageLoader<'a> = dyn FnMut(&str) -> Option<Vec<u8>> + 'a;

/// Parses `recipes.json` (array, or `{recipes: [...]}`), loading images via `load`.
pub(crate) fn parse_json_value(v: &Value, mut load: Option<&mut ImageLoader<'_>>, c: &mut Collector) {
    let items: &[Value] = match v {
        Value::Array(a) => a,
        Value::Object(o) => match o.get("recipes") {
            Some(Value::Array(a)) => a,
            _ => std::slice::from_ref(v),
        },
        _ => &[],
    };
    for (i, item) in items.iter().enumerate() {
        let ctx = item.get("name").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("#{}", i + 1));
        let rec: ExportedRecipe = match serde_json::from_value(item.clone()) {
            Ok(r) => r,
            Err(e) => {
                c.error(&ctx, format!("invalid recipe: {e}"));
                continue;
            }
        };
        let image = match (&rec.image, load.as_mut()) {
            (Some(path), Some(load)) => load(path).map(ImageSource::Bytes),
            (Some(path), None) if path.starts_with("http") => Some(ImageSource::Url(path.clone())),
            _ => None,
        };
        c.push(ImportedRecipe { data: rec.data, image, created_at: rec.created_at, favorite: rec.favorite }, &ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_file, Source};
    use larder_core::{Ingredient, Instruction};

    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 9, 9, 9];
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nxxxx";

    fn sample(id: &str, name: &str) -> ExportedRecipe {
        let mut data = RecipeData {
            name: name.into(),
            description: "Desc".into(),
            yield_quantity: Some(4.0),
            prep_time_minutes: Some(10),
            source_url: Some("https://ex.example/r".into()),
            source_name: None,
            ingredients: vec![Ingredient { original: "2 cups flour".into(), section: Some("Dough".into()), ..Default::default() }],
            instructions: vec![Instruction { section: None, text: "Mix.".into() }],
            tags: vec!["a".into()],
            categories: vec!["b".into()],
            ..Default::default()
        };
        data.nutrition.insert("calories".into(), "100".into());
        data.normalize();
        ExportedRecipe {
            id: id.into(),
            data,
            image: None,
            favorite: true,
            created_at: Some("2024-01-01T00:00:00Z".into()),
            updated_at: Some("2024-02-01T00:00:00Z".into()),
        }
    }

    #[test]
    fn round_trip() {
        let a = sample("0b6f-1", "Bread");
        let b = sample("../../etc/x", "Weird Id");
        let c = sample("plain", "No Image");
        let zip = write_export(vec![(a.clone(), Some(JPEG.to_vec())), (b, Some(PNG.to_vec())), (c, None)]).unwrap();
        let batch = parse_file(Source::Auto, "larder-export.zip", &zip).unwrap();
        assert_eq!(batch.source, Source::Larder);
        assert!(batch.errors.is_empty(), "{:?}", batch.errors);
        assert_eq!(batch.recipes.len(), 3);
        let r = &batch.recipes[0];
        assert_eq!(r.data, a.data);
        assert!(r.favorite);
        assert_eq!(r.created_at.as_deref(), Some("2024-01-01T00:00:00Z"));
        assert!(matches!(r.image, Some(ImageSource::Bytes(ref x)) if x == JPEG));
        assert!(matches!(batch.recipes[1].image, Some(ImageSource::Bytes(ref x)) if x == PNG));
        assert!(batch.recipes[2].image.is_none());
        // source_name is not invented for our own format
        assert_eq!(r.data.source_name, None);

        // The layout matches the documented format.
        let mut z = zip::ZipArchive::new(Cursor::new(zip)).unwrap();
        let names: Vec<String> = (0..z.len()).map(|i| z.by_index(i).unwrap().name().to_string()).collect();
        assert!(names.contains(&"manifest.json".to_string()));
        assert!(names.contains(&"images/0b6f-1.jpg".to_string()));
        assert!(names.contains(&"images/______etc_x.png".to_string()));
        let mut m = String::new();
        std::io::Read::read_to_string(&mut z.by_name("manifest.json").unwrap(), &mut m).unwrap();
        let m: Value = serde_json::from_str(&m).unwrap();
        assert_eq!(m["format"], FORMAT);
        assert_eq!(m["version"], 1);
    }

    #[test]
    fn legacy_manifest_is_detected() {
        let json = serde_json::to_vec(&vec![sample("x", "Old")]).unwrap();
        let z = crate::archive::testutil::make_zip(&[
            ("manifest.json", br#"{"format":"recipers-export","version":1}"#),
            ("data/recipes.json", &json),
        ]);
        let b = parse_file(Source::Auto, "old.zip", &z).unwrap();
        assert_eq!(b.source, Source::Larder);
        assert_eq!(b.recipes[0].data.name, "Old");
    }

    #[test]
    fn plain_recipes_json() {
        let json = serde_json::to_vec(&vec![sample("x", "From JSON")]).unwrap();
        let b = parse_file(Source::Auto, "recipes.json", &json).unwrap();
        assert_eq!(b.source, Source::Larder);
        assert_eq!(b.recipes[0].data.name, "From JSON");
    }
}
