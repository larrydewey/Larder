//! Recipe endpoints: list/search, CRUD, favorites, trial verdicts, images, tags, parsing.

use axum::body::Body;
use axum::extract::{Multipart, Path, Query, RawQuery, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use larder_core::{Ingredient, RecipeData, RecipeStatus, TrialState};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::AuthUser;
use crate::db::{now, RowExt};
use crate::error::{AppError, AppResult};
use crate::images::ImageSize;
use crate::recipes::{self, InsertOpts, Recipe, Verdict, FAV_EXPR, RECIPE_COLS};
use crate::state::AppState;

#[derive(Debug, Default)]
struct ListParams {
    q: Option<String>,
    tags: Vec<String>,
    categories: Vec<String>,
    favorites: bool,
    /// `None` = all statuses; see [`parse_list_params`] for the default.
    status: Option<RecipeStatus>,
    sort: String,
    page: i64,
    per_page: i64,
}

/// Parses the list query. Without `status`, a plain listing shows only the
/// cookbook (trial recipes have their own list) while a search (`q`) looks
/// at every recipe; `status=trial|cookbook|all` overrides that.
fn parse_list_params(raw: Option<&str>) -> AppResult<ListParams> {
    let mut p = ListParams { sort: "name".into(), page: 1, per_page: 50, ..Default::default() };
    let mut status: Option<String> = None;
    for (k, v) in url::form_urlencoded::parse(raw.unwrap_or("").as_bytes()) {
        let v = v.trim().to_string();
        match k.as_ref() {
            "q" if !v.is_empty() => p.q = Some(v),
            "tag" | "tags" if !v.is_empty() => p.tags.push(v),
            "category" | "categories" if !v.is_empty() => p.categories.push(v),
            "favorites" => p.favorites = matches!(v.as_str(), "true" | "1" | "yes"),
            "status" if !v.is_empty() => status = Some(v),
            "sort" if !v.is_empty() => p.sort = v,
            "page" => p.page = v.parse().unwrap_or(1).max(1),
            "per_page" => p.per_page = v.parse().unwrap_or(50).clamp(1, 500),
            _ => {}
        }
    }
    p.status = match status.as_deref() {
        Some("all") => None,
        Some(s) => Some(RecipeStatus::parse(s).ok_or_else(|| AppError::bad_request("status must be cookbook, trial or all"))?),
        None if p.q.is_some() => None,
        None => Some(RecipeStatus::Cookbook),
    };
    Ok(p)
}

pub async fn list(State(state): State<AppState>, user: AuthUser, RawQuery(raw): RawQuery) -> AppResult<Json<Value>> {
    let p = parse_list_params(raw.as_deref())?;
    // $1 = user id (favorites), $2 = household; further params are appended.
    let mut binds: Vec<String> = vec![user.id.clone(), user.household_id.clone()];
    let mut wh = vec!["r.household_id = $2".to_string()];
    if let Some(q) = &p.q {
        for term in larder_core::text::fold(q).split_whitespace().take(10) {
            binds.push(format!("%{}%", term.replace(['\\', '%', '_'], "")));
            wh.push(format!("r.search_text LIKE ${}", binds.len()));
        }
    }
    for (kind, list) in [("tag", &p.tags), ("category", &p.categories)] {
        for t in list {
            binds.push(t.to_lowercase());
            wh.push(format!(
                "EXISTS (SELECT 1 FROM recipe_tags t WHERE t.recipe_id = r.id AND t.kind = '{kind}' AND t.name_lower = ${})",
                binds.len()
            ));
        }
    }
    if p.favorites {
        wh.push("EXISTS (SELECT 1 FROM favorites f2 WHERE f2.recipe_id = r.id AND f2.user_id = $1)".into());
    }
    if let Some(status) = p.status {
        binds.push(status.as_str().to_string());
        wh.push(format!("r.status = ${}", binds.len()));
    }
    let order = match p.sort.as_str() {
        "-name" => "r.name_lower DESC",
        "updated" => "r.updated_at ASC",
        "-updated" => "r.updated_at DESC",
        "created" => "r.created_at ASC",
        "-created" => "r.created_at DESC",
        "rating" => "COALESCE(r.rating, 0) ASC, r.name_lower ASC",
        "-rating" => "COALESCE(r.rating, 0) DESC, r.name_lower ASC",
        _ => "r.name_lower ASC",
    };
    let where_sql = wh.join(" AND ");
    let count_sql = format!("SELECT COUNT(*) AS n FROM recipes r WHERE {where_sql} AND $1 = $1");
    let mut cq = sqlx::query(&count_sql);
    for b in &binds {
        cq = cq.bind(b);
    }
    let total = cq.fetch_one(&state.db.pool).await?.i("n");
    let offset = (p.page - 1) * p.per_page;
    let sql = format!(
        "SELECT {RECIPE_COLS}, {FAV_EXPR} FROM recipes r WHERE {where_sql} ORDER BY {order} LIMIT {} OFFSET {}",
        p.per_page, offset
    );
    let mut q = sqlx::query(&sql);
    for b in &binds {
        q = q.bind(b);
    }
    let items: Vec<Value> = q
        .fetch_all(&state.db.pool)
        .await?
        .iter()
        .map(|r| serde_json::to_value(recipes::recipe_from_row(r).summary()).unwrap_or_default())
        .collect();
    Ok(Json(json!({ "items": items, "total": total, "page": p.page, "per_page": p.per_page })))
}

/// Request body for create/update: RecipeData plus optional id/version and status.
#[derive(Deserialize)]
pub struct RecipeInput {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    version: Option<i64>,
    /// Create: defaults to `cookbook`. Update: omitted leaves it unchanged.
    #[serde(default)]
    status: Option<RecipeStatus>,
    /// Create only (e.g. moving standalone data to a server); updates ignore
    /// these, they change through `POST /recipes/{id}/tried`.
    #[serde(default)]
    tried_count: Option<i64>,
    #[serde(default)]
    last_tried_at: Option<String>,
    #[serde(flatten)]
    data: RecipeData,
}

pub async fn create(State(state): State<AppState>, user: AuthUser, Json(input): Json<RecipeInput>) -> AppResult<Response> {
    let (recipe, created) = recipes::insert(
        &state,
        &user.household_id,
        &user.id,
        input.data,
        InsertOpts {
            id: input.id,
            created_by: Some(&user.id),
            created_at: None,
            image_id: None,
            trial: TrialState {
                status: input.status.unwrap_or_default(),
                tried_count: input.tried_count.unwrap_or(0),
                last_tried_at: input.last_tried_at,
            },
        },
    )
    .await?;
    let status = if created { StatusCode::CREATED } else { StatusCode::OK };
    Ok((status, Json(recipe)).into_response())
}

pub async fn get(State(state): State<AppState>, user: AuthUser, Path(id): Path<String>) -> AppResult<Json<Recipe>> {
    Ok(Json(recipes::load(&state, &user.household_id, &user.id, &id).await?))
}

pub async fn update(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(input): Json<RecipeInput>,
) -> AppResult<Json<Recipe>> {
    Ok(Json(recipes::update(&state, &user.household_id, &user.id, &id, input.data, input.status, input.version).await?))
}

pub async fn delete(State(state): State<AppState>, user: AuthUser, Path(id): Path<String>) -> AppResult<StatusCode> {
    recipes::delete(&state, &user.household_id, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn duplicate(State(state): State<AppState>, user: AuthUser, Path(id): Path<String>) -> AppResult<impl IntoResponse> {
    let src = recipes::load(&state, &user.household_id, &user.id, &id).await?;
    let mut data = src.data.clone();
    data.name = format!("{} (copy)", data.name);
    // Copy the image so both recipes own their files independently.
    let image_id = match &src.image_id {
        Some(img) => match state.images.read(img, ImageSize::Full).await {
            Some(bytes) => state.images.save(bytes).await.ok(),
            None => None,
        },
        None => None,
    };
    let (recipe, _) = recipes::insert(
        &state,
        &user.household_id,
        &user.id,
        data,
        // The copy starts its own trial history.
        InsertOpts { id: None, created_by: Some(&user.id), created_at: None, image_id, trial: TrialState::new(src.trial.status) },
    )
    .await?;
    Ok((StatusCode::CREATED, Json(recipe)))
}

#[derive(Deserialize)]
pub struct TriedReq {
    /// `keep` (move to the cookbook) or `again` (stay on trial).
    outcome: String,
    /// When it was made (RFC 3339); defaults to now. Also makes the request
    /// idempotent: a replay with the same value changes nothing.
    #[serde(default)]
    tried_at: Option<String>,
    /// The cook's local date for the note line (`YYYY-MM-DD`); defaults to the UTC date of `tried_at`.
    #[serde(default)]
    date: Option<String>,
    /// 1-5 stars; omitted or null keeps the current rating.
    #[serde(default)]
    rating: Option<i64>,
    /// Appended to the notes as a dated line.
    #[serde(default)]
    note: Option<String>,
}

/// `POST /recipes/{id}/tried`: "I made this", with the household's verdict.
pub async fn tried(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<TriedReq>,
) -> AppResult<Json<Recipe>> {
    let keep = match req.outcome.trim() {
        "keep" => true,
        "again" => false,
        _ => return Err(AppError::bad_request("outcome must be keep or again")),
    };
    let tried_at = match req.tried_at.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(t) => {
            larder_core::trial::normalize_timestamp(t).ok_or_else(|| AppError::bad_request("tried_at must be an RFC 3339 timestamp"))?
        }
        None => now(),
    };
    let date = match req.date.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(d) => chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|_| AppError::bad_request("date must be YYYY-MM-DD"))?,
        None => chrono::DateTime::parse_from_rfc3339(&tried_at).map(|t| t.date_naive()).unwrap_or_default(),
    };
    if req.rating.is_some_and(|r| !(1..=5).contains(&r)) {
        return Err(AppError::bad_request("rating must be 1-5"));
    }
    let note = req.note.unwrap_or_default();
    if note.chars().count() > 2000 {
        return Err(AppError::bad_request("note is too long"));
    }
    let verdict = Verdict { keep, tried_at, rating: req.rating, note_line: larder_core::trial::note_line(date, &note) };
    Ok(Json(recipes::record_tried(&state, &user.household_id, &user.id, &id, verdict).await?))
}

pub async fn favorite(State(state): State<AppState>, user: AuthUser, Path(id): Path<String>) -> AppResult<StatusCode> {
    let r = recipes::load(&state, &user.household_id, &user.id, &id).await?;
    sqlx::query("INSERT INTO favorites (user_id, recipe_id, created_at) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
        .bind(&user.id)
        .bind(&r.id)
        .bind(now())
        .execute(&state.db.pool)
        .await?;
    recipes::touch(&state, &r.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn unfavorite(State(state): State<AppState>, user: AuthUser, Path(id): Path<String>) -> AppResult<StatusCode> {
    let r = recipes::load(&state, &user.household_id, &user.id, &id).await?;
    sqlx::query("DELETE FROM favorites WHERE user_id = $1 AND recipe_id = $2").bind(&user.id).bind(&r.id).execute(&state.db.pool).await?;
    recipes::touch(&state, &r.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Reads the first file field from a multipart body.
pub async fn read_upload(mut mp: Multipart, field_name: &str) -> AppResult<(Vec<u8>, Option<String>, Vec<(String, String)>)> {
    let mut file = None;
    let mut fields = vec![];
    while let Some(field) = mp.next_field().await? {
        let name = field.name().unwrap_or("").to_string();
        if name == field_name || (file.is_none() && field.file_name().is_some()) {
            let filename = field.file_name().map(str::to_string);
            let bytes = field.bytes().await?;
            file = Some((bytes.to_vec(), filename));
        } else {
            let text = field.text().await.unwrap_or_default();
            fields.push((name, text));
        }
    }
    let (bytes, filename) = file.ok_or_else(|| AppError::bad_request(format!("missing '{field_name}' file field")))?;
    if bytes.is_empty() {
        return Err(AppError::bad_request("uploaded file is empty"));
    }
    Ok((bytes, filename, fields))
}

pub async fn upload_image(State(state): State<AppState>, user: AuthUser, Path(id): Path<String>, mp: Multipart) -> AppResult<Json<Recipe>> {
    let r = recipes::load(&state, &user.household_id, &user.id, &id).await?;
    let (bytes, _, _) = read_upload(mp, "file").await?;
    let image_id = state.images.save(bytes).await.map_err(|e| AppError::bad_request(format!("{e:#}")))?;
    recipes::set_image(&state, &user.household_id, &r.id, Some(image_id)).await?;
    Ok(Json(recipes::load(&state, &user.household_id, &user.id, &r.id).await?))
}

pub async fn delete_image(State(state): State<AppState>, user: AuthUser, Path(id): Path<String>) -> AppResult<Json<Recipe>> {
    let r = recipes::load(&state, &user.household_id, &user.id, &id).await?;
    recipes::set_image(&state, &user.household_id, &r.id, None).await?;
    Ok(Json(recipes::load(&state, &user.household_id, &user.id, &r.id).await?))
}

#[derive(Deserialize)]
pub struct ImageQuery {
    size: Option<String>,
}

pub async fn serve_image(State(state): State<AppState>, Path(id): Path<String>, Query(q): Query<ImageQuery>) -> Response {
    let id = id.trim_end_matches(".jpg");
    match state.images.read(id, ImageSize::parse(q.size.as_deref())).await {
        Some(bytes) => Response::builder()
            .header(header::CONTENT_TYPE, "image/jpeg")
            .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
            .body(Body::from(bytes))
            .unwrap(),
        None => AppError::not_found("image not found").into_response(),
    }
}

async fn tag_counts(state: &AppState, household: &str, kind: &str) -> AppResult<Json<Vec<Value>>> {
    // Group case-insensitively, reporting the most common spelling.
    let rows = sqlx::query(
        "SELECT name_lower, MIN(name) AS name, COUNT(*) AS n FROM recipe_tags WHERE household_id = $1 AND kind = $2 \
         GROUP BY name_lower ORDER BY name_lower",
    )
    .bind(household)
    .bind(kind)
    .fetch_all(&state.db.pool)
    .await?;
    Ok(Json(rows.iter().map(|r| json!({ "name": r.s("name"), "count": r.i("n") })).collect()))
}

pub async fn tags(State(state): State<AppState>, user: AuthUser) -> AppResult<Json<Vec<Value>>> {
    tag_counts(&state, &user.household_id, "tag").await
}

pub async fn categories(State(state): State<AppState>, user: AuthUser) -> AppResult<Json<Vec<Value>>> {
    tag_counts(&state, &user.household_id, "category").await
}

#[derive(Deserialize)]
pub struct RenameTagReq {
    kind: String,
    from: String,
    #[serde(default)]
    to: Option<String>,
}

async fn rewrite_tag(state: &AppState, user: &AuthUser, kind: &str, from: &str, to: Option<&str>) -> AppResult<()> {
    if !matches!(kind, "tag" | "category") {
        return Err(AppError::bad_request("kind must be tag or category"));
    }
    let ids: Vec<String> = sqlx::query("SELECT recipe_id FROM recipe_tags WHERE household_id = $1 AND kind = $2 AND name_lower = $3")
        .bind(&user.household_id)
        .bind(kind)
        .bind(from.to_lowercase())
        .fetch_all(&state.db.pool)
        .await?
        .iter()
        .map(|r| r.s("recipe_id"))
        .collect();
    for id in ids {
        let r = recipes::load(state, &user.household_id, &user.id, &id).await?;
        let mut data = r.data.clone();
        let list = if kind == "tag" { &mut data.tags } else { &mut data.categories };
        let mut out = vec![];
        for t in list.iter() {
            if t.eq_ignore_ascii_case(from) {
                if let Some(to) = to {
                    out.push(to.to_string());
                }
            } else {
                out.push(t.clone());
            }
        }
        *list = out;
        recipes::update(state, &user.household_id, &user.id, &id, data, None, None).await?;
    }
    Ok(())
}

pub async fn rename_tag(State(state): State<AppState>, user: AuthUser, Json(req): Json<RenameTagReq>) -> AppResult<StatusCode> {
    let to = req.to.as_deref().map(str::trim).filter(|t| !t.is_empty()).ok_or_else(|| AppError::bad_request("'to' is required"))?;
    rewrite_tag(&state, &user, &req.kind, &req.from, Some(to)).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct DeleteTagReq {
    kind: String,
    name: String,
}

pub async fn delete_tag(State(state): State<AppState>, user: AuthUser, Json(req): Json<DeleteTagReq>) -> AppResult<StatusCode> {
    rewrite_tag(&state, &user, &req.kind, &req.name, None).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct ParseReq {
    lines: Vec<String>,
    #[serde(default)]
    use_llm: bool,
}

pub async fn parse_ingredients(
    State(state): State<AppState>,
    _user: AuthUser,
    Json(req): Json<ParseReq>,
) -> AppResult<Json<Vec<Ingredient>>> {
    if req.lines.len() > 500 {
        return Err(AppError::bad_request("too many lines (max 500)"));
    }
    if req.use_llm {
        let llm = state.ai.client().ok_or_else(AppError::llm_disabled)?;
        return Ok(Json(llm.parse_ingredients(&req.lines).await.map_err(|e| AppError::unprocessable(format!("{e:#}")))?));
    }
    Ok(Json(req.lines.iter().map(|l| Ingredient::parse(l)).collect()))
}

#[derive(Deserialize)]
pub struct OrganizeReq {
    /// "uncategorized" (default), "all" or "none".
    #[serde(default)]
    categorize: Option<String>,
    /// Re-run ingredient normalisation on stored recipes (default true).
    #[serde(default)]
    normalize: Option<bool>,
    /// Report what would change without saving (default false).
    #[serde(default)]
    dry_run: bool,
}

/// Tidies the household's recipes: normalises ingredients (units, food
/// names) and fills in categories. With `dry_run`, only reports the changes.
pub async fn organize(State(state): State<AppState>, user: AuthUser, Json(req): Json<OrganizeReq>) -> AppResult<Json<Value>> {
    let mode = req.categorize.as_deref().unwrap_or("uncategorized");
    if !matches!(mode, "uncategorized" | "all" | "none") {
        return Err(AppError::bad_request("categorize must be uncategorized, all or none"));
    }
    let normalize = req.normalize.unwrap_or(true);
    let all = recipes::load_all(&state, &user.household_id, &user.id, None).await?;
    let mut changes = vec![];
    let (mut categorized, mut normalized) = (0, 0);
    for r in all {
        let mut data = r.data.clone();
        if normalize {
            data.normalize();
        }
        let ingredients_changed = data.ingredients != r.data.ingredients;
        let before = r.data.categories.first().cloned();
        let mut category_changed = false;
        if mode != "none" && (mode == "all" || data.categories.is_empty()) {
            if let Some(c) = larder_core::categorize::suggest_category(&data) {
                if before.as_deref() != Some(c) {
                    data.categories.retain(|x| !x.eq_ignore_ascii_case(c));
                    data.categories.insert(0, c.to_string());
                    category_changed = true;
                }
            }
        }
        if !ingredients_changed && !category_changed {
            continue;
        }
        if ingredients_changed {
            normalized += 1;
        }
        if category_changed {
            categorized += 1;
        }
        changes.push(json!({
            "id": r.id,
            "name": r.data.name,
            "category_before": before,
            "category_after": data.categories.first(),
            "ingredients_changed": ingredients_changed,
        }));
        if !req.dry_run {
            recipes::update(&state, &user.household_id, &user.id, &r.id, data, None, None).await?;
        }
    }
    Ok(Json(json!({
        "dry_run": req.dry_run,
        "recipes_changed": changes.len(),
        "categorized": categorized,
        "normalized": normalized,
        "changes": changes,
    })))
}
