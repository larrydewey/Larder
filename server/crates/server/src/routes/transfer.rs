//! Import (files, URLs, text, photos, live Mealie) and export (JSON backup,
//! printable cookbook PDF) endpoints.

use axum::body::Body;
use axum::extract::{Multipart, Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use larder_core::RecipeData;
use larder_import::{ImageSource, ImportedRecipe, Source};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::AuthUser;
use crate::db::{new_id, now, RowExt};
use crate::error::{AppError, AppResult};
use crate::images::ImageSize;
use crate::recipes::{self, InsertOpts};
use crate::routes::recipes::read_upload;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/import/url", post(import_url))
        .route("/import/text", post(import_text))
        .route("/import/image", post(import_image))
        .route("/import/file", post(import_file))
        .route("/import/mealie", post(import_mealie))
        .route("/import/jobs", get(list_jobs))
        .route("/import/jobs/{id}", get(get_job))
        .route("/export/json", get(export_json))
        .route("/export/cookbook/themes", get(cookbook_themes))
        .route("/export/cookbook", post(export_cookbook))
}

fn bad_import(e: impl std::fmt::Display) -> AppError {
    AppError::unprocessable(e.to_string())
}

/// Saves (or previews) a single imported recipe.
/// Gives an uncategorised recipe the category Larder suggests for it.
fn auto_categorize(d: &mut RecipeData) {
    if d.categories.is_empty() {
        if let Some(c) = larder_core::categorize::suggest_category(d) {
            d.categories.push(c.to_string());
        }
    }
}

async fn save_or_preview(state: &AppState, user: &AuthUser, mut imported: ImportedRecipe, save: bool) -> AppResult<Response> {
    auto_categorize(&mut imported.data);
    if !save {
        let mut v = serde_json::to_value(&imported.data)?;
        if let Some(ImageSource::Url(u)) = &imported.image {
            v["image_url"] = json!(u);
        }
        return Ok((StatusCode::OK, Json(v)).into_response());
    }
    let recipe = store_imported(state, user, imported).await?;
    Ok((StatusCode::CREATED, Json(recipe)).into_response())
}

async fn store_imported(state: &AppState, user: &AuthUser, imported: ImportedRecipe) -> AppResult<recipes::Recipe> {
    let image_id = match imported.image {
        Some(src) => resolve_image(state, src).await,
        None => None,
    };
    let (recipe, _) = recipes::insert(
        state,
        &user.household_id,
        &user.id,
        imported.data,
        InsertOpts { id: None, created_by: Some(&user.id), created_at: imported.created_at, image_id },
    )
    .await?;
    if imported.favorite {
        let _ = sqlx::query("INSERT INTO favorites (user_id, recipe_id, created_at) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
            .bind(&user.id)
            .bind(&recipe.id)
            .bind(now())
            .execute(&state.db.pool)
            .await;
    }
    Ok(recipe)
}

async fn resolve_image(state: &AppState, src: ImageSource) -> Option<String> {
    let bytes = match src {
        ImageSource::Bytes(b) => b,
        ImageSource::Url(u) if u.parse::<url::Url>().map(|p| crate::netguard::url_has_blocked_ip(&p)).unwrap_or(true) => {
            tracing::debug!("refusing image URL {u}");
            return None;
        }
        ImageSource::Url(u) => match larder_import::fetch_image(&state.fetch, &u).await {
            Ok(b) => b,
            Err(e) => {
                tracing::debug!("image download failed for {u}: {e}");
                return None;
            }
        },
    };
    match state.images.save(bytes).await {
        Ok(id) => Some(id),
        Err(e) => {
            tracing::debug!("image rejected: {e:#}");
            None
        }
    }
}

/// Validates a user-supplied URL before fetching it. Name resolution and
/// redirects are additionally guarded by [`crate::netguard`].
async fn check_public_url(raw: &str) -> AppResult<url::Url> {
    let url = url::Url::parse(raw.trim()).map_err(|_| AppError::bad_request("invalid URL"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::bad_request("only http(s) URLs can be imported"));
    }
    let host = url.host_str().ok_or_else(|| AppError::bad_request("URL has no host"))?.to_string();
    if crate::netguard::url_has_blocked_ip(&url) {
        return Err(AppError::bad_request("this address cannot be imported from"));
    }
    let port = url.port_or_known_default().unwrap_or(80);
    let addrs: Vec<_> = tokio::net::lookup_host((host.as_str(), port))
        .await
        .map_err(|_| AppError::bad_request(format!("cannot resolve host '{host}'")))?
        .collect();
    if addrs.is_empty() || addrs.iter().all(|a| crate::netguard::is_blocked(a.ip())) {
        return Err(AppError::bad_request("this address cannot be imported from"));
    }
    Ok(url)
}

#[derive(Deserialize)]
struct UrlReq {
    url: String,
    #[serde(default = "yes")]
    save: bool,
}

fn yes() -> bool {
    true
}

async fn import_url(State(state): State<AppState>, user: AuthUser, Json(req): Json<UrlReq>) -> AppResult<Response> {
    let url = check_public_url(&req.url).await?;
    let imported = larder_import::scrape_url(&state.fetch, url.as_str()).await.map_err(bad_import)?;
    save_or_preview(&state, &user, imported, req.save).await
}

#[derive(Deserialize)]
struct TextReq {
    text: String,
    #[serde(default)]
    save: bool,
    #[serde(default)]
    use_llm: bool,
}

async fn import_text(State(state): State<AppState>, user: AuthUser, Json(req): Json<TextReq>) -> AppResult<Response> {
    if req.text.trim().is_empty() {
        return Err(AppError::bad_request("text is empty"));
    }
    if req.text.len() > 200_000 {
        return Err(AppError::bad_request("text is too long"));
    }
    let data: RecipeData = if req.use_llm {
        let llm = state.llm.as_ref().ok_or_else(AppError::llm_disabled)?;
        llm.recipe_from_text(&req.text).await.map_err(|e| AppError::unprocessable(format!("{e:#}")))?
    } else {
        let mut d = larder_import::text::parse_plain_text(&req.text);
        d.normalize();
        d
    };
    if data.name.trim().is_empty() {
        return Err(AppError::unprocessable("could not find a recipe title in the text"));
    }
    let imported = ImportedRecipe { data, image: None, created_at: None, favorite: false };
    save_or_preview(&state, &user, imported, req.save).await
}

async fn import_image(State(state): State<AppState>, user: AuthUser, mp: Multipart) -> AppResult<Response> {
    let llm = state.llm.as_ref().ok_or_else(AppError::llm_disabled)?;
    let (bytes, _, fields) = read_upload(mp, "file").await?;
    let save = fields.iter().any(|(k, v)| k == "save" && v == "true");
    // Normalise to a reasonably sized JPEG for the model.
    let jpeg = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
        let img = crate::images::decode(&bytes)?;
        let img = if img.width().max(img.height()) > 2000 { img.resize(2000, 2000, image::imageops::FilterType::Lanczos3) } else { img };
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 88).encode_image(&image::DynamicImage::ImageRgb8(img.to_rgb8()))?;
        Ok(out)
    })
    .await
    .map_err(|e| AppError::internal(e.to_string()))?
    .map_err(|e| AppError::bad_request(format!("{e:#}")))?;
    let data = llm
        .recipe_from_image(larder_assist::ImageInput { bytes: jpeg.clone(), media_type: "image/jpeg".into() })
        .await
        .map_err(|e| AppError::unprocessable(format!("{e:#}")))?;
    let mut imported = ImportedRecipe { data, image: None, created_at: None, favorite: false };
    if !save {
        return save_or_preview(&state, &user, imported, false).await;
    }
    auto_categorize(&mut imported.data);
    let mut recipe = store_imported(&state, &user, imported).await?;
    // Keep the photo as the recipe image.
    if let Ok(img) = state.images.save(jpeg).await {
        recipes::set_image(&state, &user.household_id, &recipe.id, Some(img)).await?;
        recipe = recipes::load(&state, &user.household_id, &user.id, &recipe.id).await?;
    }
    Ok((StatusCode::CREATED, Json(recipe)).into_response())
}

// ---------------- background import jobs ----------------

async fn create_job(state: &AppState, user: &AuthUser, source: &str) -> AppResult<String> {
    let id = new_id();
    sqlx::query(
        "INSERT INTO import_jobs (id, household_id, user_id, source, status, total, imported, skipped, failed, errors, recipe_ids, created_at) \
         VALUES ($1, $2, $3, $4, 'pending', 0, 0, 0, 0, '[]', '[]', $5)",
    )
    .bind(&id)
    .bind(&user.household_id)
    .bind(&user.id)
    .bind(source)
    .bind(now())
    .execute(&state.db.pool)
    .await?;
    Ok(id)
}

fn job_json(r: &sqlx::any::AnyRow) -> Value {
    json!({
        "id": r.s("id"),
        "source": r.s("source"),
        "status": r.s("status"),
        "total": r.i("total"),
        "imported": r.i("imported"),
        "skipped": r.i("skipped"),
        "failed": r.i("failed"),
        "errors": r.json::<Vec<String>>("errors"),
        "recipe_ids": r.json::<Vec<String>>("recipe_ids"),
        "created_at": r.s("created_at"),
        "finished_at": r.os("finished_at"),
    })
}

async fn load_job(state: &AppState, household: &str, id: &str) -> AppResult<Value> {
    let row = sqlx::query("SELECT * FROM import_jobs WHERE id = $1 AND household_id = $2")
        .bind(id)
        .bind(household)
        .fetch_optional(&state.db.pool)
        .await?
        .ok_or_else(|| AppError::not_found("import job not found"))?;
    Ok(job_json(&row))
}

async fn list_jobs(State(state): State<AppState>, user: AuthUser) -> AppResult<Json<Vec<Value>>> {
    let rows = sqlx::query("SELECT * FROM import_jobs WHERE household_id = $1 ORDER BY created_at DESC LIMIT 50")
        .bind(&user.household_id)
        .fetch_all(&state.db.pool)
        .await?;
    Ok(Json(rows.iter().map(job_json).collect()))
}

async fn get_job(State(state): State<AppState>, user: AuthUser, Path(id): Path<String>) -> AppResult<Json<Value>> {
    Ok(Json(load_job(&state, &user.household_id, &id).await?))
}

/// Mutable progress of a running job, flushed to the database periodically.
struct Progress {
    job_id: String,
    total: usize,
    imported: usize,
    skipped: usize,
    failed: usize,
    errors: Vec<String>,
    recipe_ids: Vec<String>,
}

impl Progress {
    fn new(job_id: String) -> Self {
        Self { job_id, total: 0, imported: 0, skipped: 0, failed: 0, errors: vec![], recipe_ids: vec![] }
    }

    fn error(&mut self, e: String) {
        self.failed += 1;
        if self.errors.len() < 200 {
            self.errors.push(e);
        }
    }

    async fn flush(&self, state: &AppState, status: &str) {
        let finished = matches!(status, "done" | "failed").then(now);
        let res = sqlx::query(
            "UPDATE import_jobs SET status = $1, total = $2, imported = $3, skipped = $4, failed = $5, errors = $6, recipe_ids = $7, finished_at = $8 WHERE id = $9",
        )
        .bind(status)
        .bind(self.total as i64)
        .bind(self.imported as i64)
        .bind(self.skipped as i64)
        .bind(self.failed as i64)
        .bind(serde_json::to_string(&self.errors).unwrap_or_else(|_| "[]".into()))
        .bind(serde_json::to_string(&self.recipe_ids).unwrap_or_else(|_| "[]".into()))
        .bind(finished)
        .bind(&self.job_id)
        .execute(&state.db.pool)
        .await;
        if let Err(e) = res {
            tracing::error!("failed to update import job: {e}");
        }
    }
}

async fn import_one(state: &AppState, user: &AuthUser, p: &mut Progress, mut imported: ImportedRecipe, opts: JobOpts) {
    if opts.categorize {
        auto_categorize(&mut imported.data);
    }
    let skip_duplicates = opts.skip_duplicates;
    let name = imported.data.name.clone();
    if skip_duplicates {
        match recipes::find_duplicate(state, &user.household_id, &imported.data).await {
            Ok(Some(_)) => {
                p.skipped += 1;
                return;
            }
            Ok(None) => {}
            Err(e) => {
                p.error(format!("Recipe '{name}': {}", e.message));
                return;
            }
        }
    }
    match store_imported(state, user, imported).await {
        Ok(r) => {
            p.imported += 1;
            p.recipe_ids.push(r.id);
        }
        Err(e) => p.error(format!("Recipe '{name}': {}", e.message)),
    }
}

async fn import_file(State(state): State<AppState>, user: AuthUser, mp: Multipart) -> AppResult<Response> {
    let (bytes, filename, fields) = read_upload(mp, "file").await?;
    let field = |k: &str| fields.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    let source_str = field("source").unwrap_or_else(|| "auto".into());
    let source = Source::parse(&source_str).ok_or_else(|| AppError::bad_request(format!("unknown import source '{source_str}'")))?;
    let opts = JobOpts {
        skip_duplicates: field("skip_duplicates").map(|v| v != "false").unwrap_or(true),
        categorize: field("categorize").map(|v| v != "false").unwrap_or(true),
    };
    let filename = filename.unwrap_or_else(|| "upload".into());
    let job_id = create_job(&state, &user, source.as_str()).await?;
    let job = load_job(&state, &user.household_id, &job_id).await?;
    let st = state.clone();
    tokio::spawn(async move {
        let mut p = Progress::new(job_id);
        p.flush(&st, "running").await;
        let parsed = tokio::task::spawn_blocking(move || larder_import::parse_file(source, &filename, &bytes)).await;
        let batch = match parsed {
            Ok(Ok(b)) => b,
            Ok(Err(e)) => {
                p.errors.push(e.to_string());
                p.flush(&st, "failed").await;
                return;
            }
            Err(e) => {
                p.errors.push(format!("import crashed: {e}"));
                p.flush(&st, "failed").await;
                return;
            }
        };
        p.total = batch.recipes.len() + batch.errors.len();
        for e in batch.errors {
            p.error(e);
        }
        p.flush(&st, "running").await;
        for (i, r) in batch.recipes.into_iter().enumerate() {
            import_one(&st, &user, &mut p, r, opts).await;
            if i % 10 == 9 {
                p.flush(&st, "running").await;
            }
        }
        p.flush(&st, "done").await;
    });
    Ok((StatusCode::ACCEPTED, Json(job)).into_response())
}

#[derive(Deserialize)]
struct MealieReq {
    base_url: String,
    token: String,
    #[serde(default = "yes")]
    skip_duplicates: bool,
    #[serde(default = "yes")]
    categorize: bool,
}

/// Options shared by background import jobs.
#[derive(Clone, Copy)]
struct JobOpts {
    skip_duplicates: bool,
    /// Give recipes without a category the suggested one.
    categorize: bool,
}

async fn import_mealie(State(state): State<AppState>, user: AuthUser, Json(req): Json<MealieReq>) -> AppResult<Response> {
    let base = check_public_url(&req.base_url).await?;
    let client = larder_import::mealie_api::MealieClient::new(state.fetch.clone(), base.as_str(), &req.token).map_err(bad_import)?;
    // Validate credentials up-front so the user gets an immediate error.
    let slugs = client.list_slugs().await.map_err(bad_import)?;
    let job_id = create_job(&state, &user, "mealie").await?;
    let job = load_job(&state, &user.household_id, &job_id).await?;
    let st = state.clone();
    tokio::spawn(async move {
        let mut p = Progress::new(job_id);
        p.total = slugs.len();
        p.flush(&st, "running").await;
        for (i, slug) in slugs.iter().enumerate() {
            match client.fetch(slug).await {
                Ok(r) => {
                    import_one(&st, &user, &mut p, r, JobOpts { skip_duplicates: req.skip_duplicates, categorize: req.categorize }).await
                }
                Err(e) => p.error(format!("Recipe '{slug}': {e}")),
            }
            if i % 5 == 4 {
                p.flush(&st, "running").await;
            }
        }
        p.flush(&st, "done").await;
    });
    Ok((StatusCode::ACCEPTED, Json(job)).into_response())
}

/// Marks jobs left running by a previous process as failed.
pub async fn fail_stale_jobs(state: &AppState) {
    let _ = sqlx::query(
        "UPDATE import_jobs SET status = 'failed', errors = '[\"The server restarted before the import finished\"]', finished_at = $1 \
         WHERE status IN ('pending', 'running')",
    )
    .bind(now())
    .execute(&state.db.pool)
    .await;
}

// ---------------- export ----------------

fn attachment(bytes: Vec<u8>, content_type: &str, filename: &str) -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}\""))
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(bytes))
        .unwrap()
}

async fn export_json(State(state): State<AppState>, user: AuthUser) -> AppResult<Response> {
    let all = recipes::load_all(&state, &user.household_id, &user.id, None).await?;
    let mut items = vec![];
    for r in all {
        let image = match &r.image_id {
            Some(img) => state.images.read(img, ImageSize::Full).await,
            None => None,
        };
        let exported = larder_import::larder::ExportedRecipe {
            id: r.id.clone(),
            data: r.data,
            image: image.as_ref().map(|_| format!("images/{}.jpg", r.id)),
            favorite: r.is_favorite,
            created_at: Some(r.created_at),
            updated_at: Some(r.updated_at),
        };
        items.push((exported, image));
    }
    let zip = tokio::task::spawn_blocking(move || larder_import::larder::write_export(items))
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .map_err(|e| AppError::internal(e.to_string()))?;
    let date = chrono::Utc::now().format("%Y-%m-%d");
    Ok(attachment(zip, "application/zip", &format!("larder-export-{date}.zip")))
}

async fn cookbook_themes(_user: AuthUser) -> Json<Value> {
    Json(serde_json::to_value(larder_cookbook::themes()).unwrap_or_default())
}

#[derive(Deserialize)]
struct CookbookReq {
    #[serde(default)]
    recipe_ids: Option<Vec<String>>,
    #[serde(flatten)]
    options: larder_cookbook::CookbookOptions,
}

async fn export_cookbook(State(state): State<AppState>, user: AuthUser, Json(req): Json<CookbookReq>) -> AppResult<Response> {
    let mut all = recipes::load_all(&state, &user.household_id, &user.id, None).await?;
    if let Some(ids) = &req.recipe_ids {
        let wanted: std::collections::HashSet<&String> = ids.iter().collect();
        all.retain(|r| wanted.contains(&r.id) || wanted.contains(&r.slug));
    }
    if all.len() > 5000 {
        return Err(AppError::bad_request("too many recipes for one cookbook (max 5000)"));
    }
    let mut book = Vec::with_capacity(all.len());
    for r in all {
        let image = match (&r.image_id, req.options.include_images) {
            (Some(img), true) => state.images.read(img, ImageSize::Full).await,
            _ => None,
        };
        book.push(larder_cookbook::BookRecipe { id: r.id, data: r.data, image });
    }
    let opts = req.options;
    let title = opts.title.clone();
    let pdf = tokio::task::spawn_blocking(move || larder_cookbook::render_pdf(&opts, book))
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
        .map_err(|e| AppError::internal(format!("rendering cookbook failed: {e:#}")))?;
    let mut name = larder_core::text::slugify(&title);
    if name == "recipe" {
        name = "cookbook".into();
    }
    Ok(attachment(pdf, "application/pdf", &format!("{name}.pdf")))
}
