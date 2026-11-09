//! Meal planner, shopping lists and offline sync endpoints.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use larder_core::aisle;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::any::AnyRow;

use crate::auth::AuthUser;
use crate::db::{id_or_new, now, RowExt};
use crate::error::{AppError, AppResult};
use crate::images::image_url;
use crate::recipes;
use crate::state::AppState;

const MEALS: &[&str] = &["breakfast", "lunch", "dinner", "snack"];

fn valid_date(d: &str) -> AppResult<String> {
    chrono::NaiveDate::parse_from_str(d.trim(), "%Y-%m-%d")
        .map(|d| d.format("%Y-%m-%d").to_string())
        .map_err(|_| AppError::bad_request("dates must be YYYY-MM-DD"))
}

// ---------------- meal plan ----------------

#[derive(Serialize)]
pub struct MealPlanEntry {
    id: String,
    date: String,
    meal: String,
    recipe_id: Option<String>,
    recipe_name: Option<String>,
    recipe_image_url: Option<String>,
    title: Option<String>,
    note: Option<String>,
    servings: Option<f64>,
    created_by: Option<String>,
    updated_at: String,
}

const MP_SELECT: &str = "SELECT m.id, m.date, m.meal, m.recipe_id, m.title, m.note, m.servings, m.created_by, m.updated_at, \
    r.name AS recipe_name, r.image_id AS recipe_image_id FROM meal_plan m LEFT JOIN recipes r ON r.id = m.recipe_id";

fn mp_from_row(r: &AnyRow) -> MealPlanEntry {
    MealPlanEntry {
        id: r.s("id"),
        date: r.s("date"),
        meal: r.s("meal"),
        recipe_id: r.os("recipe_id"),
        recipe_name: r.os("recipe_name"),
        recipe_image_url: image_url(&r.os("recipe_image_id")),
        title: r.os("title"),
        note: r.os("note"),
        servings: r.of("servings"),
        created_by: r.os("created_by"),
        updated_at: r.s("updated_at"),
    }
}

fn meal_rank(m: &str) -> usize {
    MEALS.iter().position(|x| *x == m).unwrap_or(MEALS.len())
}

pub async fn load_meal_plan(state: &AppState, household: &str, start: &str, end: &str) -> AppResult<Vec<MealPlanEntry>> {
    let rows =
        sqlx::query(&format!("{MP_SELECT} WHERE m.household_id = $1 AND m.date >= $2 AND m.date <= $3 ORDER BY m.date, m.created_at"))
            .bind(household)
            .bind(start)
            .bind(end)
            .fetch_all(&state.db.pool)
            .await?;
    let mut v: Vec<MealPlanEntry> = rows.iter().map(mp_from_row).collect();
    v.sort_by(|a, b| a.date.cmp(&b.date).then(meal_rank(&a.meal).cmp(&meal_rank(&b.meal))));
    Ok(v)
}

#[derive(Deserialize)]
pub struct RangeQuery {
    start: String,
    end: String,
}

pub async fn list_meal_plan(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<RangeQuery>,
) -> AppResult<Json<Vec<MealPlanEntry>>> {
    let (start, end) = (valid_date(&q.start)?, valid_date(&q.end)?);
    Ok(Json(load_meal_plan(&state, &user.household_id, &start, &end).await?))
}

#[derive(Deserialize)]
pub struct MealPlanInput {
    id: Option<String>,
    date: String,
    meal: String,
    recipe_id: Option<String>,
    title: Option<String>,
    note: Option<String>,
    servings: Option<f64>,
}

async fn validate_mp(state: &AppState, user: &AuthUser, input: &MealPlanInput) -> AppResult<(String, Option<String>)> {
    let date = valid_date(&input.date)?;
    if !MEALS.contains(&input.meal.as_str()) {
        return Err(AppError::bad_request("meal must be breakfast, lunch, dinner or snack"));
    }
    let recipe_id = match input.recipe_id.as_deref().filter(|s| !s.is_empty()) {
        Some(rid) => Some(recipes::load(state, &user.household_id, &user.id, rid).await?.id),
        None => None,
    };
    if recipe_id.is_none() && input.title.as_deref().map(str::trim).unwrap_or("").is_empty() {
        return Err(AppError::bad_request("either recipe_id or title is required"));
    }
    Ok((date, recipe_id))
}

async fn load_mp_entry(state: &AppState, household: &str, id: &str) -> AppResult<MealPlanEntry> {
    let row = sqlx::query(&format!("{MP_SELECT} WHERE m.id = $1 AND m.household_id = $2"))
        .bind(id)
        .bind(household)
        .fetch_optional(&state.db.pool)
        .await?
        .ok_or_else(|| AppError::not_found("meal plan entry not found"))?;
    Ok(mp_from_row(&row))
}

pub async fn create_meal_plan(State(state): State<AppState>, user: AuthUser, Json(input): Json<MealPlanInput>) -> AppResult<Response> {
    let (date, recipe_id) = validate_mp(&state, &user, &input).await?;
    let id = id_or_new(input.id.as_deref())?;
    if let Some(row) = sqlx::query("SELECT household_id FROM meal_plan WHERE id = $1").bind(&id).fetch_optional(&state.db.pool).await? {
        if row.s("household_id") == user.household_id {
            return Ok((StatusCode::OK, Json(load_mp_entry(&state, &user.household_id, &id).await?)).into_response());
        }
        return Err(AppError::conflict("id already in use"));
    }
    let ts = now();
    sqlx::query(
        "INSERT INTO meal_plan (id, household_id, date, meal, recipe_id, title, note, servings, created_by, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)",
    )
    .bind(&id)
    .bind(&user.household_id)
    .bind(&date)
    .bind(&input.meal)
    .bind(recipe_id)
    .bind(input.title.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()))
    .bind(input.note.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()))
    .bind(input.servings.filter(|s| *s > 0.0))
    .bind(&user.id)
    .bind(&ts)
    .execute(&state.db.pool)
    .await?;
    Ok((StatusCode::CREATED, Json(load_mp_entry(&state, &user.household_id, &id).await?)).into_response())
}

pub async fn update_meal_plan(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(input): Json<MealPlanInput>,
) -> AppResult<Json<MealPlanEntry>> {
    let (date, recipe_id) = validate_mp(&state, &user, &input).await?;
    let res = sqlx::query(
        "UPDATE meal_plan SET date = $1, meal = $2, recipe_id = $3, title = $4, note = $5, servings = $6, updated_at = $7 \
         WHERE id = $8 AND household_id = $9",
    )
    .bind(&date)
    .bind(&input.meal)
    .bind(recipe_id)
    .bind(input.title.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()))
    .bind(input.note.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()))
    .bind(input.servings.filter(|s| *s > 0.0))
    .bind(now())
    .bind(&id)
    .bind(&user.household_id)
    .execute(&state.db.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::not_found("meal plan entry not found"));
    }
    Ok(Json(load_mp_entry(&state, &user.household_id, &id).await?))
}

pub async fn delete_meal_plan(State(state): State<AppState>, user: AuthUser, Path(id): Path<String>) -> AppResult<StatusCode> {
    sqlx::query("DELETE FROM meal_plan WHERE id = $1 AND household_id = $2")
        .bind(&id)
        .bind(&user.household_id)
        .execute(&state.db.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------- shopping lists ----------------

pub use larder_core::shopping::ShoppingItem;

#[derive(Serialize)]
pub struct ShoppingList {
    id: String,
    name: String,
    created_at: String,
    updated_at: String,
    item_count: i64,
    checked_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    items: Option<Vec<ShoppingItem>>,
}

use larder_core::shopping::item_display;

fn item_from_row(r: &AnyRow) -> ShoppingItem {
    let (food, quantity, unit, note) = (r.os("food"), r.of("quantity"), r.os("unit"), r.os("note"));
    ShoppingItem {
        id: r.s("id"),
        list_id: r.s("list_id"),
        display: item_display(&food, quantity, &unit, &note),
        food,
        quantity,
        unit,
        note,
        aisle: r.s("aisle"),
        checked: r.b("checked"),
        position: r.i("position"),
        recipe_ids: r.json("recipe_ids"),
        updated_at: r.s("updated_at"),
    }
}

async fn load_items(state: &AppState, list_id: &str) -> AppResult<Vec<ShoppingItem>> {
    let rows = sqlx::query("SELECT * FROM shopping_items WHERE list_id = $1 ORDER BY position, food")
        .bind(list_id)
        .fetch_all(&state.db.pool)
        .await?;
    let mut items: Vec<ShoppingItem> = rows.iter().map(item_from_row).collect();
    larder_core::shopping::sort_items(&mut items);
    Ok(items)
}

async fn load_list(state: &AppState, household: &str, id: &str, with_items: bool) -> AppResult<ShoppingList> {
    let row = sqlx::query(
        "SELECT l.id, l.name, l.created_at, l.updated_at, \
         (SELECT COUNT(*) FROM shopping_items i WHERE i.list_id = l.id) AS item_count, \
         (SELECT COUNT(*) FROM shopping_items i WHERE i.list_id = l.id AND i.checked = 1) AS checked_count \
         FROM shopping_lists l WHERE l.id = $1 AND l.household_id = $2",
    )
    .bind(id)
    .bind(household)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or_else(|| AppError::not_found("shopping list not found"))?;
    let items = if with_items { Some(load_items(state, id).await?) } else { None };
    Ok(list_from_row(&row, items))
}

fn list_from_row(r: &AnyRow, items: Option<Vec<ShoppingItem>>) -> ShoppingList {
    ShoppingList {
        id: r.s("id"),
        name: r.s("name"),
        created_at: r.s("created_at"),
        updated_at: r.s("updated_at"),
        item_count: r.i("item_count"),
        checked_count: r.i("checked_count"),
        items,
    }
}

pub async fn load_all_lists(state: &AppState, household: &str, with_items: bool) -> AppResult<Vec<ShoppingList>> {
    let rows = sqlx::query(
        "SELECT l.id, l.name, l.created_at, l.updated_at, \
         (SELECT COUNT(*) FROM shopping_items i WHERE i.list_id = l.id) AS item_count, \
         (SELECT COUNT(*) FROM shopping_items i WHERE i.list_id = l.id AND i.checked = 1) AS checked_count \
         FROM shopping_lists l WHERE l.household_id = $1 ORDER BY l.updated_at DESC",
    )
    .bind(household)
    .fetch_all(&state.db.pool)
    .await?;
    let mut out = vec![];
    for r in &rows {
        let items = if with_items { Some(load_items(state, &r.s("id")).await?) } else { None };
        out.push(list_from_row(r, items));
    }
    Ok(out)
}

async fn touch_list(state: &AppState, id: &str) -> AppResult<()> {
    sqlx::query("UPDATE shopping_lists SET updated_at = $1 WHERE id = $2").bind(now()).bind(id).execute(&state.db.pool).await?;
    Ok(())
}

pub async fn list_lists(State(state): State<AppState>, user: AuthUser) -> AppResult<Json<Vec<ShoppingList>>> {
    Ok(Json(load_all_lists(&state, &user.household_id, false).await?))
}

#[derive(Deserialize)]
pub struct ListInput {
    id: Option<String>,
    name: String,
}

pub async fn create_list(State(state): State<AppState>, user: AuthUser, Json(input): Json<ListInput>) -> AppResult<Response> {
    let name = input.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(AppError::bad_request("name must be 1-100 characters"));
    }
    let id = id_or_new(input.id.as_deref())?;
    if let Some(row) = sqlx::query("SELECT household_id FROM shopping_lists WHERE id = $1").bind(&id).fetch_optional(&state.db.pool).await?
    {
        if row.s("household_id") == user.household_id {
            return Ok((StatusCode::OK, Json(load_list(&state, &user.household_id, &id, true).await?)).into_response());
        }
        return Err(AppError::conflict("id already in use"));
    }
    let ts = now();
    sqlx::query("INSERT INTO shopping_lists (id, household_id, name, created_at, updated_at) VALUES ($1, $2, $3, $4, $4)")
        .bind(&id)
        .bind(&user.household_id)
        .bind(name)
        .bind(&ts)
        .execute(&state.db.pool)
        .await?;
    Ok((StatusCode::CREATED, Json(load_list(&state, &user.household_id, &id, true).await?)).into_response())
}

pub async fn get_list(State(state): State<AppState>, user: AuthUser, Path(id): Path<String>) -> AppResult<Json<ShoppingList>> {
    Ok(Json(load_list(&state, &user.household_id, &id, true).await?))
}

#[derive(Deserialize)]
pub struct ListPatch {
    name: String,
}

pub async fn update_list(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(p): Json<ListPatch>,
) -> AppResult<Json<ShoppingList>> {
    let name = p.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(AppError::bad_request("name must be 1-100 characters"));
    }
    let res = sqlx::query("UPDATE shopping_lists SET name = $1, updated_at = $2 WHERE id = $3 AND household_id = $4")
        .bind(name)
        .bind(now())
        .bind(&id)
        .bind(&user.household_id)
        .execute(&state.db.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::not_found("shopping list not found"));
    }
    Ok(Json(load_list(&state, &user.household_id, &id, true).await?))
}

pub async fn delete_list(State(state): State<AppState>, user: AuthUser, Path(id): Path<String>) -> AppResult<StatusCode> {
    sqlx::query("DELETE FROM shopping_lists WHERE id = $1 AND household_id = $2")
        .bind(&id)
        .bind(&user.household_id)
        .execute(&state.db.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct ItemInput {
    id: Option<String>,
    text: Option<String>,
    food: Option<String>,
    quantity: Option<f64>,
    unit: Option<String>,
    note: Option<String>,
    aisle: Option<String>,
}

fn clean(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn normalize_aisle(a: Option<String>, food: &Option<String>, unit: &Option<String>) -> String {
    match clean(a) {
        Some(a) => aisle::AISLES.iter().find(|x| x.eq_ignore_ascii_case(&a)).map(|x| x.to_string()).unwrap_or(a),
        None => larder_core::shopping::aisle_for(food.as_deref().unwrap_or(""), unit.as_deref()).to_string(),
    }
}

async fn next_position(state: &AppState, list_id: &str) -> AppResult<i64> {
    let row = sqlx::query("SELECT COALESCE(MAX(position), -1) AS p FROM shopping_items WHERE list_id = $1")
        .bind(list_id)
        .fetch_one(&state.db.pool)
        .await?;
    Ok(row.i("p") + 1)
}

pub async fn add_item(
    State(state): State<AppState>,
    user: AuthUser,
    Path(list_id): Path<String>,
    Json(input): Json<ItemInput>,
) -> AppResult<Response> {
    load_list(&state, &user.household_id, &list_id, false).await?;
    let id = id_or_new(input.id.as_deref())?;
    if let Some(row) = sqlx::query("SELECT * FROM shopping_items WHERE id = $1").bind(&id).fetch_optional(&state.db.pool).await? {
        if row.s("list_id") == list_id {
            return Ok((StatusCode::OK, Json(item_from_row(&row))).into_response());
        }
        return Err(AppError::conflict("id already in use"));
    }
    let (food, quantity, unit, note) = match clean(input.text) {
        Some(t) => {
            // Same parsing and food tidy-up as recipe ingredients and standalone mode.
            let it = larder_core::shopping::ShoppingItem::from_text(&t);
            (it.food.or(Some(t)), it.quantity, it.unit, it.note)
        }
        None => (clean(input.food), input.quantity, clean(input.unit), clean(input.note)),
    };
    if food.is_none() && note.is_none() {
        return Err(AppError::bad_request("item text or food is required"));
    }
    let aisle = normalize_aisle(input.aisle, &food, &unit);
    let pos = next_position(&state, &list_id).await?;
    sqlx::query(
        "INSERT INTO shopping_items (id, list_id, food, quantity, unit, note, aisle, checked, position, recipe_ids, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 0, $8, '[]', $9)",
    )
    .bind(&id)
    .bind(&list_id)
    .bind(food)
    .bind(quantity)
    .bind(unit)
    .bind(note)
    .bind(aisle)
    .bind(pos)
    .bind(now())
    .execute(&state.db.pool)
    .await?;
    touch_list(&state, &list_id).await?;
    let row = sqlx::query("SELECT * FROM shopping_items WHERE id = $1").bind(&id).fetch_one(&state.db.pool).await?;
    Ok((StatusCode::CREATED, Json(item_from_row(&row))).into_response())
}

pub async fn update_item(
    State(state): State<AppState>,
    user: AuthUser,
    Path((list_id, item_id)): Path<(String, String)>,
    Json(patch): Json<Value>,
) -> AppResult<Json<ShoppingItem>> {
    load_list(&state, &user.household_id, &list_id, false).await?;
    let row = sqlx::query("SELECT * FROM shopping_items WHERE id = $1 AND list_id = $2")
        .bind(&item_id)
        .bind(&list_id)
        .fetch_optional(&state.db.pool)
        .await?
        .ok_or_else(|| AppError::not_found("item not found"))?;
    let mut item = item_from_row(&row);
    let obj = patch.as_object().ok_or_else(|| AppError::bad_request("expected a JSON object"))?;
    let opt_str = |v: &Value| v.as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    if let Some(v) = obj.get("food") {
        item.food = opt_str(v);
    }
    if let Some(v) = obj.get("quantity") {
        item.quantity = v.as_f64();
    }
    if let Some(v) = obj.get("unit") {
        item.unit = opt_str(v).map(|u| larder_core::units::canonical_unit(&u).map(str::to_string).unwrap_or(u));
    }
    if let Some(v) = obj.get("note") {
        item.note = opt_str(v);
    }
    if let Some(v) = obj.get("aisle") {
        item.aisle = normalize_aisle(opt_str(v), &item.food, &item.unit);
    }
    if let Some(v) = obj.get("checked") {
        item.checked = v.as_bool().unwrap_or(item.checked);
    }
    if let Some(v) = obj.get("position") {
        item.position = v.as_i64().unwrap_or(item.position);
    }
    sqlx::query(
        "UPDATE shopping_items SET food=$1, quantity=$2, unit=$3, note=$4, aisle=$5, checked=$6, position=$7, updated_at=$8 WHERE id=$9",
    )
    .bind(item.food.clone())
    .bind(item.quantity)
    .bind(item.unit.clone())
    .bind(item.note.clone())
    .bind(&item.aisle)
    .bind(item.checked as i64)
    .bind(item.position)
    .bind(now())
    .bind(&item_id)
    .execute(&state.db.pool)
    .await?;
    touch_list(&state, &list_id).await?;
    let row = sqlx::query("SELECT * FROM shopping_items WHERE id = $1").bind(&item_id).fetch_one(&state.db.pool).await?;
    Ok(Json(item_from_row(&row)))
}

pub async fn delete_item(
    State(state): State<AppState>,
    user: AuthUser,
    Path((list_id, item_id)): Path<(String, String)>,
) -> AppResult<StatusCode> {
    load_list(&state, &user.household_id, &list_id, false).await?;
    sqlx::query("DELETE FROM shopping_items WHERE id = $1 AND list_id = $2").bind(&item_id).bind(&list_id).execute(&state.db.pool).await?;
    touch_list(&state, &list_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn clear_checked(State(state): State<AppState>, user: AuthUser, Path(list_id): Path<String>) -> AppResult<Json<ShoppingList>> {
    load_list(&state, &user.household_id, &list_id, false).await?;
    sqlx::query("DELETE FROM shopping_items WHERE list_id = $1 AND checked = 1").bind(&list_id).execute(&state.db.pool).await?;
    touch_list(&state, &list_id).await?;
    Ok(Json(load_list(&state, &user.household_id, &list_id, true).await?))
}

#[derive(Deserialize)]
pub struct RecipeRef {
    recipe_id: String,
    servings: Option<f64>,
}

#[derive(Deserialize)]
pub struct AddRecipesReq {
    recipes: Vec<RecipeRef>,
}

/// Merges scaled recipe ingredients into a list (rules live in `larder_core::shopping`).
async fn merge_recipes(state: &AppState, user: &AuthUser, list_id: &str, refs: Vec<RecipeRef>) -> AppResult<()> {
    let mut items = load_items(state, list_id).await?;
    let mut loaded = vec![];
    for r in refs {
        let recipe = recipes::load(state, &user.household_id, &user.id, &r.recipe_id).await?;
        loaded.push((recipe, r.servings));
    }
    let additions: Vec<larder_core::shopping::RecipeAddition> =
        loaded.iter().map(|(r, s)| larder_core::shopping::RecipeAddition { recipe_id: &r.id, data: &r.data, servings: *s }).collect();
    let ts = now();
    let result = larder_core::shopping::merge_recipes(&mut items, list_id, &additions, crate::db::new_id, &ts);
    let mut tx = state.db.pool.begin().await?;
    for it in items.iter().filter(|i| result.updated.contains(&i.id)) {
        sqlx::query("UPDATE shopping_items SET quantity = $1, unit = $2, recipe_ids = $3, updated_at = $4 WHERE id = $5")
            .bind(it.quantity)
            .bind(it.unit.clone())
            .bind(serde_json::to_string(&it.recipe_ids)?)
            .bind(&ts)
            .bind(&it.id)
            .execute(&mut *tx)
            .await?;
    }
    for it in items.iter().filter(|i| result.created.contains(&i.id)) {
        sqlx::query(
            "INSERT INTO shopping_items (id, list_id, food, quantity, unit, note, aisle, checked, position, recipe_ids, updated_at) \
             VALUES ($1, $2, $3, $4, $5, NULL, $6, 0, $7, $8, $9)",
        )
        .bind(&it.id)
        .bind(list_id)
        .bind(it.food.clone())
        .bind(it.quantity)
        .bind(it.unit.clone())
        .bind(&it.aisle)
        .bind(it.position)
        .bind(serde_json::to_string(&it.recipe_ids)?)
        .bind(&ts)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query("UPDATE shopping_lists SET updated_at = $1 WHERE id = $2").bind(&ts).bind(list_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn add_recipes(
    State(state): State<AppState>,
    user: AuthUser,
    Path(list_id): Path<String>,
    Json(req): Json<AddRecipesReq>,
) -> AppResult<Json<ShoppingList>> {
    load_list(&state, &user.household_id, &list_id, false).await?;
    merge_recipes(&state, &user, &list_id, req.recipes).await?;
    Ok(Json(load_list(&state, &user.household_id, &list_id, true).await?))
}

pub async fn add_mealplan(
    State(state): State<AppState>,
    user: AuthUser,
    Path(list_id): Path<String>,
    Json(q): Json<RangeQuery>,
) -> AppResult<Json<ShoppingList>> {
    load_list(&state, &user.household_id, &list_id, false).await?;
    let (start, end) = (valid_date(&q.start)?, valid_date(&q.end)?);
    let entries = load_meal_plan(&state, &user.household_id, &start, &end).await?;
    let refs: Vec<RecipeRef> =
        entries.into_iter().filter_map(|e| e.recipe_id.map(|rid| RecipeRef { recipe_id: rid, servings: e.servings })).collect();
    merge_recipes(&state, &user, &list_id, refs).await?;
    Ok(Json(load_list(&state, &user.household_id, &list_id, true).await?))
}

// ---------------- sync ----------------

#[derive(Deserialize)]
pub struct SyncQuery {
    since: Option<String>,
}

pub async fn sync(State(state): State<AppState>, user: AuthUser, Query(q): Query<SyncQuery>) -> AppResult<Json<Value>> {
    let cursor = now();
    // Overlap the window slightly so writes committed just before the previous
    // cursor are not missed; clients upsert so duplicates are harmless.
    let since = q
        .since
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| crate::db::format_ts(t.with_timezone(&chrono::Utc) - chrono::Duration::seconds(10)));
    let recipes = recipes::load_all(&state, &user.household_id, &user.id, since.as_deref()).await?;
    let deleted: Vec<String> = match &since {
        Some(s) => sqlx::query("SELECT id FROM deleted_recipes WHERE household_id = $1 AND deleted_at >= $2")
            .bind(&user.household_id)
            .bind(s)
            .fetch_all(&state.db.pool)
            .await?
            .iter()
            .map(|r| r.s("id"))
            .collect(),
        None => vec![],
    };
    let today = chrono::Utc::now().date_naive();
    let start = (today - chrono::Duration::days(31)).format("%Y-%m-%d").to_string();
    let end = (today + chrono::Duration::days(61)).format("%Y-%m-%d").to_string();
    let meal_plan = load_meal_plan(&state, &user.household_id, &start, &end).await?;
    let lists = load_all_lists(&state, &user.household_id, true).await?;
    Ok(Json(json!({
        "cursor": cursor,
        "full": since.is_none(),
        "recipes": recipes,
        "deleted_recipe_ids": deleted,
        "meal_plan": meal_plan,
        "shopping_lists": lists,
    })))
}
