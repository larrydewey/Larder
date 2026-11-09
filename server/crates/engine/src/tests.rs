use super::*;

fn ok(method: &str, req: Value) -> Value {
    let out: Value = serde_json::from_slice(&call(method, req.to_string().as_bytes())).unwrap();
    assert!(out.get("error").is_none(), "{method} failed: {out}");
    out["ok"].clone()
}

fn err(method: &str, req: &[u8]) -> Value {
    let out: Value = serde_json::from_slice(&call(method, req)).unwrap();
    assert!(out.get("ok").is_none(), "{method} unexpectedly succeeded: {out}");
    out
}

fn png() -> Vec<u8> {
    let img = image::RgbImage::from_fn(800, 600, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 70]));
    let mut out = Vec::new();
    DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    out
}

use image::DynamicImage;

#[test]
fn errors_and_version() {
    assert_eq!(ok("version", json!({}))["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(err("nope", b"{}")["code"], "bad_request");
    assert_eq!(err("parse_ingredients", b"not json")["code"], "bad_request");
    assert_eq!(err("normalize_recipe", br#"{"name":" "}"#)["code"], "bad_request");
}

#[test]
fn ffi_roundtrip() {
    let method = std::ffi::CString::new("parse_ingredients").unwrap();
    let input = br#"{"lines":["2 cups flour"]}"#;
    let mut len = 0usize;
    unsafe {
        let ptr = larder_call(method.as_ptr(), input.as_ptr(), input.len(), &mut len);
        let v: Value = serde_json::from_slice(std::slice::from_raw_parts(ptr, len)).unwrap();
        assert_eq!(v["ok"][0]["unit"], "cup");
        larder_free(ptr, len);
    }
}

#[test]
fn normalize_and_format() {
    let r = ok(
        "normalize_recipe",
        json!({"id": "abc", "name": " Crème Brûlée ", "prep_time_minutes": 5, "cook_time_minutes": 10,
        "ingredients": [{"original": "2 cups cream"}], "local_image_path": "/x.jpg"}),
    );
    assert_eq!(r["id"], "abc");
    assert_eq!(r["local_image_path"], "/x.jpg");
    assert_eq!(r["name"], "Crème Brûlée");
    assert_eq!(r["slug"], "creme-brulee");
    assert_eq!(r["total_time_minutes"], 15);
    assert_eq!(r["ingredients"][0]["food"], "cream");
    assert!(r["search_text"].as_str().unwrap().contains("creme brulee"));
    let t = ok("format_ingredient", json!({"ingredient": r["ingredients"][0], "scale": 1.5, "units": "metric"}));
    assert_eq!(t["text"], "710 ml cream");
    assert_eq!(ok("classify_aisle", json!({"food": "basil"}))["aisle"], "Produce");
    let item = ok("parse_shopping_item", json!({"text": "2 lbs apples"}));
    assert_eq!(item["unit"], "lb");
    assert_eq!(item["display"], "2 lb apples");
}

#[test]
fn merge() {
    let recipe = json!({"id": "r1", "name": "Pancakes", "yield_quantity": 4.0,
        "ingredients": [{"original": "1 cup milk", "quantity": 1.0, "unit": "cup", "food": "milk"}]});
    let items = json!([{"id": "i1", "list_id": "L", "food": "milk", "quantity": 1.0, "unit": "cup", "aisle": "Dairy & Eggs"}]);
    let out = ok("merge_shopping", json!({"items": items, "add": [{"recipe": recipe, "servings": 8}]}));
    assert_eq!(out["items"].as_array().unwrap().len(), 1);
    assert_eq!(out["items"][0]["quantity"], 3.0);
    assert_eq!(out["items"][0]["display"], "3 cups milk");
    assert_eq!(out["updated"][0], "i1");
}

#[test]
fn images_and_backup_and_import() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in.png");
    std::fs::write(&src, png()).unwrap();
    let paths = ok("process_image", json!({"input_path": src, "output_dir": dir.path().join("img"), "id": "r1"}));
    let thumb = image::open(paths["thumb"].as_str().unwrap()).unwrap();
    assert_eq!(thumb.width(), 320);
    assert_eq!(
        err("process_image", json!({"input_path": dir.path().join("missing"), "output_dir": dir.path()}).to_string().as_bytes())["code"],
        "bad_request"
    );

    let backup = dir.path().join("out/backup.zip");
    ok(
        "export_backup",
        json!({"recipes": [{"id": "r1", "data": {"name": "Soup", "ingredients": [{"original": "1 onion"}]}, "image_path": paths["full"], "favorite": true}],
            "out_path": backup}),
    );
    let imported = ok("import_file", json!({"source": "auto", "path": backup, "image_dir": dir.path().join("imported")}));
    assert_eq!(imported["source"], "larder");
    let recipes = imported["recipes"].as_array().unwrap();
    assert_eq!(recipes.len(), 1);
    assert_eq!(recipes[0]["data"]["name"], "Soup");
    assert_eq!(recipes[0]["favorite"], true);
    let img = recipes[0]["image_path"].as_str().unwrap();
    assert!(std::path::Path::new(img).exists());

    let text = ok("import_text", json!({"text": "Toast\n\nIngredients\n1 slice bread\n\nMethod\n1. Toast it."}));
    assert_eq!(text["name"], "Toast");
}

#[test]
fn cookbook() {
    let dir = tempfile::tempdir().unwrap();
    let themes = ok("cookbook_themes", json!({}));
    assert!(themes.as_array().unwrap().len() >= 3);
    let out = dir.path().join("book.pdf");
    let res = ok(
        "render_cookbook",
        json!({"options": {"title": "Test", "paper": "a5", "theme": "modern"},
            "recipes": [{"id": "r1", "data": {"name": "Soup", "ingredients": [{"original": "1 onion"}], "instructions": [{"text": "Cook."}]}}],
            "out_path": out}),
    );
    assert!(res["bytes"].as_u64().unwrap() > 1000);
    assert_eq!(&std::fs::read(out).unwrap()[..4], b"%PDF");
}

#[test]
fn llm_requires_config() {
    let e = err("llm_recipe_from_text", br#"{"llm":{"provider":"none"},"text":"x"}"#);
    assert_eq!(e["code"], "llm_disabled");
    let e = err("llm_recipe_from_text", br#"{"llm":{"provider":"bogus"},"text":"x"}"#);
    assert_eq!(e["code"], "bad_request");
}

#[test]
fn timestamps() {
    assert_eq!(civil_from_days(0), (1970, 1, 1));
    assert_eq!(civil_from_days(20_401), (2025, 11, 9));
    let ts = now_ts();
    assert_eq!(ts.len(), 27);
    assert!(ts.ends_with('Z'));
}
