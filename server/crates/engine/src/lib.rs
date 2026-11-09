//! On-device engine for the Larder Android app.
//!
//! In standalone mode (no server) the app calls into this library through a
//! tiny C ABI: one function taking a method name and a JSON request, returning
//! a JSON response. The methods mirror what the server does, using the very
//! same crates, so behaviour is identical with or without a server. See
//! `docs/ENGINE.md` for the contract.

use std::ffi::{c_char, CStr};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use larder_core::shopping::{self, RecipeAddition, ShoppingItem};
use larder_core::units::UnitSystem;
use larder_core::{Ingredient, RecipeData};
use larder_import::{ImageSource, ImportedRecipe, Source};
use serde::Deserialize;
use serde_json::{json, Value};

mod images;

/// Error returned to Dart as `{"error": message, "code": code}`.
#[derive(Debug)]
pub struct EngineError {
    pub code: &'static str,
    pub message: String,
}

impl EngineError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
    fn bad_request(m: impl Into<String>) -> Self {
        Self::new("bad_request", m)
    }
    fn unprocessable(m: impl std::fmt::Display) -> Self {
        Self::new("unprocessable", m.to_string())
    }
    fn internal(m: impl std::fmt::Display) -> Self {
        Self::new("internal", m.to_string())
    }
}

type Result<T> = std::result::Result<T, EngineError>;

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("larder-engine")
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(larder_import::USER_AGENT)
            .timeout(std::time::Duration::from_secs(60))
            .connect_timeout(std::time::Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::limited(8))
            .build()
            .expect("http client")
    })
}

fn parse<T: for<'de> Deserialize<'de>>(input: &[u8]) -> Result<T> {
    let bytes = if input.is_empty() { b"{}".as_slice() } else { input };
    serde_json::from_slice(bytes).map_err(|e| EngineError::bad_request(format!("invalid request: {e}")))
}

fn to_value<T: serde::Serialize>(v: &T) -> Result<Value> {
    serde_json::to_value(v).map_err(EngineError::internal)
}

/// Safe entry point used by the FFI wrapper and by tests. Always returns a JSON document.
pub fn call(method: &str, input: &[u8]) -> Vec<u8> {
    let result = catch_unwind(AssertUnwindSafe(|| dispatch(method, input)));
    let body = match result {
        Ok(Ok(v)) => json!({ "ok": v }),
        Ok(Err(e)) => json!({ "error": e.message, "code": e.code }),
        Err(p) => {
            let msg = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
            json!({ "error": format!("engine panicked: {msg}"), "code": "internal" })
        }
    };
    serde_json::to_vec(&body).unwrap_or_else(|_| br#"{"error":"serialisation failed","code":"internal"}"#.to_vec())
}

/// C ABI: see docs/ENGINE.md.
///
/// # Safety
/// `method` must be a valid NUL-terminated string, `input` must point to
/// `input_len` readable bytes (or be null when `input_len` is 0), and
/// `out_len` must be a valid pointer. The returned buffer must be released
/// with [`larder_free`].
#[no_mangle]
pub unsafe extern "C" fn larder_call(method: *const c_char, input: *const u8, input_len: usize, out_len: *mut usize) -> *mut u8 {
    let method = if method.is_null() { "" } else { CStr::from_ptr(method).to_str().unwrap_or("") };
    let input = if input.is_null() || input_len == 0 { &[][..] } else { std::slice::from_raw_parts(input, input_len) };
    let out = call(method, input).into_boxed_slice();
    if !out_len.is_null() {
        *out_len = out.len();
    }
    Box::into_raw(out) as *mut u8
}

/// Frees a buffer returned by [`larder_call`].
///
/// # Safety
/// `ptr`/`len` must come from a single previous `larder_call` result.
#[no_mangle]
pub unsafe extern "C" fn larder_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)));
    }
}

fn dispatch(method: &str, input: &[u8]) -> Result<Value> {
    match method {
        "version" => Ok(json!({ "version": env!("CARGO_PKG_VERSION") })),
        "parse_ingredients" => parse_ingredients(parse(input)?),
        "normalize_recipe" => normalize_recipe(parse(input)?),
        "format_ingredient" => format_ingredient(parse(input)?),
        "parse_shopping_item" => parse_shopping_item(parse(input)?),
        "classify_aisle" => {
            #[derive(Deserialize)]
            struct Req {
                food: String,
            }
            let r: Req = parse(input)?;
            Ok(json!({ "aisle": larder_core::aisle::classify(&r.food) }))
        }
        "merge_shopping" => merge_shopping(parse(input)?),
        "suggest_categories" => suggest_categories(parse(input)?),
        "process_image" => process_image(parse(input)?),
        "import_file" => import_file(parse(input)?),
        "import_url" => import_url(parse(input)?),
        "import_text" => {
            #[derive(Deserialize)]
            struct Req {
                text: String,
            }
            let r: Req = parse(input)?;
            let mut d = larder_import::text::parse_plain_text(&r.text);
            d.normalize();
            to_value(&d)
        }
        "mealie_list" => mealie_list(parse(input)?),
        "mealie_fetch" => mealie_fetch(parse(input)?),
        "llm_recipe_from_text" => llm_text(parse(input)?),
        "llm_recipe_from_image" => llm_image(parse(input)?),
        "llm_parse_ingredients" => llm_ingredients(parse(input)?),
        "cookbook_themes" => to_value(&larder_cookbook::themes()),
        "render_cookbook" => render_cookbook(parse(input)?),
        "export_backup" => export_backup(parse(input)?),
        "" => Err(EngineError::bad_request("missing method")),
        other => Err(EngineError::bad_request(format!("unknown method '{other}'"))),
    }
}

// ---------------- parsing & formatting ----------------

#[derive(Deserialize)]
struct LinesReq {
    lines: Vec<String>,
}

fn parse_ingredients(r: LinesReq) -> Result<Value> {
    to_value(&r.lines.iter().map(|l| Ingredient::parse(l)).collect::<Vec<_>>())
}

fn normalize_recipe(input: Value) -> Result<Value> {
    if !input.is_object() {
        return Err(EngineError::bad_request("expected a recipe object"));
    }
    let mut data: RecipeData =
        serde_json::from_value(input.clone()).map_err(|e| EngineError::bad_request(format!("invalid recipe: {e}")))?;
    data.normalize();
    data.validate().map_err(EngineError::bad_request)?;
    let mut out = input;
    let normalized = to_value(&data)?;
    for (k, v) in normalized.as_object().unwrap() {
        out[k] = v.clone();
    }
    out["slug"] = json!(larder_core::text::slugify(&data.name));
    out["search_text"] = json!(data.search_text());
    Ok(out)
}

#[derive(Deserialize)]
struct FormatReq {
    ingredient: Ingredient,
    #[serde(default = "one")]
    scale: f64,
    #[serde(default)]
    units: Option<String>,
}

fn one() -> f64 {
    1.0
}

fn format_ingredient(r: FormatReq) -> Result<Value> {
    let system = UnitSystem::parse(r.units.as_deref().unwrap_or("original"));
    Ok(json!({ "text": r.ingredient.display(r.scale, system) }))
}

#[derive(Deserialize)]
struct TextReq {
    text: String,
}

fn parse_shopping_item(r: TextReq) -> Result<Value> {
    if r.text.trim().is_empty() {
        return Err(EngineError::bad_request("text is empty"));
    }
    let it = ShoppingItem::from_text(&r.text);
    Ok(json!({ "food": it.food, "quantity": it.quantity, "unit": it.unit, "note": it.note, "aisle": it.aisle, "display": it.display }))
}

#[derive(Deserialize)]
struct MergeAdd {
    recipe: Value,
    #[serde(default)]
    servings: Option<f64>,
}

#[derive(Deserialize)]
struct MergeReq {
    #[serde(default)]
    list_id: Option<String>,
    items: Vec<ShoppingItem>,
    add: Vec<MergeAdd>,
    #[serde(default)]
    now: Option<String>,
}

fn merge_shopping(r: MergeReq) -> Result<Value> {
    let list_id = r.list_id.clone().or_else(|| r.items.first().map(|i| i.list_id.clone())).unwrap_or_default();
    let mut recipes = vec![];
    for a in &r.add {
        let id =
            a.recipe.get("id").and_then(|v| v.as_str()).ok_or_else(|| EngineError::bad_request("each recipe needs an id"))?.to_string();
        let data: RecipeData =
            serde_json::from_value(a.recipe.clone()).map_err(|e| EngineError::bad_request(format!("invalid recipe: {e}")))?;
        recipes.push((id, data, a.servings));
    }
    let additions: Vec<RecipeAddition> = recipes.iter().map(|(id, d, s)| RecipeAddition { recipe_id: id, data: d, servings: *s }).collect();
    let mut items = r.items;
    for it in &mut items {
        if it.aisle.is_empty() {
            it.aisle = larder_core::aisle::classify(it.food.as_deref().unwrap_or("")).to_string();
        }
        it.refresh_display();
    }
    let now = r.now.unwrap_or_else(now_ts);
    let res = shopping::merge_recipes(&mut items, &list_id, &additions, || uuid::Uuid::new_v4().to_string(), &now);
    shopping::sort_items(&mut items);
    Ok(json!({ "items": items, "updated": res.updated, "created": res.created }))
}

fn now_ts() -> String {
    let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    // Fixed-width RFC 3339 UTC with microseconds, matching the server.
    let secs = d.as_secs() as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, m, dd) = civil_from_days(days);
    format!("{y:04}-{m:02}-{dd:02}T{:02}:{:02}:{:02}.{:06}Z", rem / 3600, (rem % 3600) / 60, rem % 60, d.subsec_micros())
}

/// Howard Hinnant's days-to-civil algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[derive(Deserialize)]
struct CategorizeReq {
    recipes: Vec<Value>,
}

/// Suggests a category for each recipe (`null` when nothing fits).
fn suggest_categories(r: CategorizeReq) -> Result<Value> {
    let mut out = vec![];
    for v in r.recipes {
        let mut data: RecipeData =
            serde_json::from_value(v.clone()).map_err(|e| EngineError::bad_request(format!("invalid recipe: {e}")))?;
        // Parse lines sent as `original` only, so ingredient hints can apply.
        data.normalize();
        out.push(json!({ "id": v.get("id"), "category": larder_core::categorize::suggest_category(&data) }));
    }
    Ok(json!({ "categories": larder_core::categorize::CATEGORIES, "suggestions": out }))
}

// ---------------- images ----------------

#[derive(Deserialize)]
struct ProcessImageReq {
    input_path: PathBuf,
    output_dir: PathBuf,
    #[serde(default)]
    id: Option<String>,
}

fn process_image(r: ProcessImageReq) -> Result<Value> {
    let bytes = std::fs::read(&r.input_path).map_err(|e| EngineError::bad_request(format!("cannot read image: {e}")))?;
    let id = r.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let paths = images::process(&bytes, &r.output_dir, &id).map_err(EngineError::unprocessable)?;
    Ok(json!({ "thumb": paths.thumb, "medium": paths.medium, "full": paths.full }))
}

/// Writes an imported image as a JPEG into `image_dir`, returning its path.
fn store_image(src: Option<ImageSource>, image_dir: &Path) -> Option<String> {
    let bytes = match src? {
        ImageSource::Bytes(b) => b,
        ImageSource::Url(u) => runtime().block_on(larder_import::fetch_image(http(), &u)).ok()?,
    };
    std::fs::create_dir_all(image_dir).ok()?;
    let path = image_dir.join(format!("{}.jpg", uuid::Uuid::new_v4()));
    images::write_jpeg(&bytes, &path, 2400).ok()?;
    Some(path.to_string_lossy().into_owned())
}

fn imported_json(r: ImportedRecipe, image_dir: &Path) -> Value {
    let image_path = store_image(r.image, image_dir);
    json!({ "data": r.data, "image_path": image_path, "favorite": r.favorite, "created_at": r.created_at })
}

// ---------------- importers ----------------

#[derive(Deserialize)]
struct ImportFileReq {
    #[serde(default)]
    source: Option<String>,
    path: PathBuf,
    #[serde(default)]
    filename: Option<String>,
    image_dir: PathBuf,
}

fn import_file(r: ImportFileReq) -> Result<Value> {
    let source_name = r.source.unwrap_or_else(|| "auto".into());
    let source = Source::parse(&source_name).ok_or_else(|| EngineError::bad_request(format!("unknown import source '{source_name}'")))?;
    let bytes = std::fs::read(&r.path).map_err(|e| EngineError::bad_request(format!("cannot read file: {e}")))?;
    let filename = r.filename.unwrap_or_else(|| r.path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default());
    let batch = larder_import::parse_file(source, &filename, &bytes).map_err(EngineError::unprocessable)?;
    let recipes: Vec<Value> = batch.recipes.into_iter().map(|rec| imported_json(rec, &r.image_dir)).collect();
    Ok(json!({ "source": batch.source.as_str(), "recipes": recipes, "errors": batch.errors }))
}

#[derive(Deserialize)]
struct ImportUrlReq {
    url: String,
    image_dir: PathBuf,
}

fn import_url(r: ImportUrlReq) -> Result<Value> {
    let rec = runtime().block_on(larder_import::scrape_url(http(), r.url.trim())).map_err(EngineError::unprocessable)?;
    Ok(imported_json(rec, &r.image_dir))
}

#[derive(Deserialize)]
struct MealieReq {
    base_url: String,
    token: String,
    #[serde(default)]
    slug: Option<String>,
    #[serde(default)]
    image_dir: Option<PathBuf>,
}

fn mealie_client(r: &MealieReq) -> Result<larder_import::mealie_api::MealieClient> {
    larder_import::mealie_api::MealieClient::new(http().clone(), &r.base_url, &r.token).map_err(EngineError::unprocessable)
}

fn mealie_list(r: MealieReq) -> Result<Value> {
    let client = mealie_client(&r)?;
    let slugs = runtime().block_on(client.list_slugs()).map_err(EngineError::unprocessable)?;
    Ok(json!({ "slugs": slugs }))
}

fn mealie_fetch(r: MealieReq) -> Result<Value> {
    let client = mealie_client(&r)?;
    let slug = r.slug.clone().ok_or_else(|| EngineError::bad_request("slug is required"))?;
    let dir = r.image_dir.clone().ok_or_else(|| EngineError::bad_request("image_dir is required"))?;
    let rec = runtime().block_on(client.fetch(&slug)).map_err(EngineError::unprocessable)?;
    Ok(imported_json(rec, &dir))
}

// ---------------- LLM ----------------

#[derive(Deserialize)]
struct LlmSettings {
    provider: String,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
}

fn llm(settings: LlmSettings) -> Result<larder_assist::Llm> {
    let blank = |s: Option<String>| s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let cfg =
        larder_assist::LlmConfig::from_parts(&settings.provider, blank(settings.api_key), blank(settings.model), blank(settings.base_url))
            .map_err(EngineError::bad_request)?
            .ok_or_else(|| EngineError::new("llm_disabled", "AI assist is not configured"))?;
    Ok(larder_assist::Llm::new(cfg, http().clone()))
}

#[derive(Deserialize)]
struct LlmTextReq {
    llm: LlmSettings,
    text: String,
}

fn llm_text(r: LlmTextReq) -> Result<Value> {
    let client = llm(r.llm)?;
    let d = runtime().block_on(client.recipe_from_text(&r.text)).map_err(|e| EngineError::unprocessable(format!("{e:#}")))?;
    to_value(&d)
}

#[derive(Deserialize)]
struct LlmImageReq {
    llm: LlmSettings,
    image_path: PathBuf,
}

fn llm_image(r: LlmImageReq) -> Result<Value> {
    let client = llm(r.llm)?;
    let bytes = std::fs::read(&r.image_path).map_err(|e| EngineError::bad_request(format!("cannot read image: {e}")))?;
    let jpeg = images::to_jpeg(&bytes, 2000).map_err(EngineError::unprocessable)?;
    let d = runtime()
        .block_on(client.recipe_from_image(larder_assist::ImageInput { bytes: jpeg, media_type: "image/jpeg".into() }))
        .map_err(|e| EngineError::unprocessable(format!("{e:#}")))?;
    to_value(&d)
}

#[derive(Deserialize)]
struct LlmLinesReq {
    llm: LlmSettings,
    lines: Vec<String>,
}

fn llm_ingredients(r: LlmLinesReq) -> Result<Value> {
    let client = llm(r.llm)?;
    let v = runtime().block_on(client.parse_ingredients(&r.lines)).map_err(|e| EngineError::unprocessable(format!("{e:#}")))?;
    to_value(&v)
}

// ---------------- cookbook & backup ----------------

#[derive(Deserialize)]
struct BookRecipeReq {
    id: String,
    data: RecipeData,
    #[serde(default)]
    image_path: Option<PathBuf>,
}

#[derive(Deserialize)]
struct CookbookReq {
    #[serde(default)]
    options: larder_cookbook::CookbookOptions,
    recipes: Vec<BookRecipeReq>,
    out_path: PathBuf,
}

fn render_cookbook(r: CookbookReq) -> Result<Value> {
    let book = r
        .recipes
        .into_iter()
        .map(|b| larder_cookbook::BookRecipe {
            id: b.id,
            image: if r.options.include_images { b.image_path.and_then(|p| std::fs::read(p).ok()) } else { None },
            data: b.data,
        })
        .collect();
    let pdf =
        larder_cookbook::render_pdf(&r.options, book).map_err(|e| EngineError::internal(format!("rendering cookbook failed: {e:#}")))?;
    write_out(&r.out_path, &pdf)?;
    Ok(json!({ "path": r.out_path, "bytes": pdf.len() }))
}

#[derive(Deserialize)]
struct BackupRecipe {
    id: String,
    data: RecipeData,
    #[serde(default)]
    image_path: Option<PathBuf>,
    #[serde(default)]
    favorite: bool,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
}

#[derive(Deserialize)]
struct BackupReq {
    recipes: Vec<BackupRecipe>,
    out_path: PathBuf,
}

fn export_backup(r: BackupReq) -> Result<Value> {
    let items = r
        .recipes
        .into_iter()
        .map(|b| {
            let image = b.image_path.and_then(|p| std::fs::read(p).ok());
            let exported = larder_import::larder::ExportedRecipe {
                image: image.as_ref().map(|_| format!("images/{}.jpg", b.id)),
                id: b.id,
                data: b.data,
                favorite: b.favorite,
                created_at: b.created_at,
                updated_at: b.updated_at,
            };
            (exported, image)
        })
        .collect();
    let zip = larder_import::larder::write_export(items).map_err(EngineError::internal)?;
    write_out(&r.out_path, &zip)?;
    Ok(json!({ "path": r.out_path }))
}

fn write_out(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| EngineError::internal(format!("cannot create {}: {e}", parent.display())))?;
    }
    std::fs::write(path, bytes).map_err(|e| EngineError::internal(format!("cannot write {}: {e}", path.display())))
}

#[cfg(test)]
mod tests;
