//! Recipe persistence: loading, inserting, updating and deleting recipes,
//! shared by the HTTP handlers, importers and sync.

use std::collections::BTreeMap;

use larder_core::{Ingredient, Instruction, RecipeData, RecipeStatus, TrialState};
use serde::Serialize;
use sqlx::any::AnyRow;

use crate::db::{now, RowExt};
use crate::error::{AppError, AppResult};
use crate::images::image_url;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct Recipe {
    pub id: String,
    pub household_id: String,
    pub slug: String,
    #[serde(flatten)]
    pub data: RecipeData,
    /// `status`, `tried_count` and `last_tried_at` (household-wide).
    #[serde(flatten)]
    pub trial: TrialState,
    pub image_url: Option<String>,
    #[serde(skip)]
    pub image_id: Option<String>,
    pub is_favorite: bool,
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecipeSummary {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub image_url: Option<String>,
    pub tags: Vec<String>,
    pub categories: Vec<String>,
    pub rating: Option<i64>,
    pub total_time_minutes: Option<i64>,
    pub yield_quantity: Option<f64>,
    pub yield_unit: String,
    pub is_favorite: bool,
    pub status: RecipeStatus,
    pub tried_count: i64,
    pub last_tried_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub const RECIPE_COLS: &str = "r.id, r.household_id, r.slug, r.name, r.description, r.yield_quantity, r.yield_unit, \
    r.prep_time_minutes, r.cook_time_minutes, r.total_time_minutes, r.source_url, r.source_name, r.notes, r.rating, \
    r.ingredients, r.instructions, r.tags, r.categories, r.nutrition, r.image_id, r.created_by, r.created_at, \
    r.updated_at, r.version, r.status, r.tried_count, r.last_tried_at";

/// SQL fragment yielding `is_fav` (0/1) for user `$1`.
pub const FAV_EXPR: &str = "(SELECT COUNT(*) FROM favorites f WHERE f.recipe_id = r.id AND f.user_id = $1) AS is_fav";

pub fn recipe_from_row(r: &AnyRow) -> Recipe {
    let image_id = r.os("image_id");
    Recipe {
        id: r.s("id"),
        household_id: r.s("household_id"),
        slug: r.s("slug"),
        data: RecipeData {
            name: r.s("name"),
            description: r.s("description"),
            yield_quantity: r.of("yield_quantity"),
            yield_unit: r.s("yield_unit"),
            prep_time_minutes: r.oi("prep_time_minutes"),
            cook_time_minutes: r.oi("cook_time_minutes"),
            total_time_minutes: r.oi("total_time_minutes"),
            source_url: r.os("source_url"),
            source_name: r.os("source_name"),
            notes: r.s("notes"),
            rating: r.oi("rating"),
            ingredients: r.json::<Vec<Ingredient>>("ingredients"),
            instructions: r.json::<Vec<Instruction>>("instructions"),
            tags: r.json::<Vec<String>>("tags"),
            categories: r.json::<Vec<String>>("categories"),
            nutrition: r.json::<BTreeMap<String, String>>("nutrition"),
        },
        trial: TrialState {
            status: RecipeStatus::parse(&r.s("status")).unwrap_or_default(),
            tried_count: r.i("tried_count"),
            last_tried_at: r.os("last_tried_at"),
        },
        image_url: image_url(&image_id),
        image_id,
        is_favorite: r.oi("is_fav").unwrap_or(0) > 0,
        created_by: r.os("created_by"),
        created_at: r.s("created_at"),
        updated_at: r.s("updated_at"),
        version: r.i("version"),
    }
}

impl Recipe {
    pub fn summary(&self) -> RecipeSummary {
        RecipeSummary {
            id: self.id.clone(),
            slug: self.slug.clone(),
            name: self.data.name.clone(),
            description: self.data.description.clone(),
            image_url: self.image_url.clone(),
            tags: self.data.tags.clone(),
            categories: self.data.categories.clone(),
            rating: self.data.rating,
            total_time_minutes: self.data.total_time_minutes,
            yield_quantity: self.data.yield_quantity,
            yield_unit: self.data.yield_unit.clone(),
            is_favorite: self.is_favorite,
            status: self.trial.status,
            tried_count: self.trial.tried_count,
            last_tried_at: self.trial.last_tried_at.clone(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }
}

/// Loads a recipe by id or slug within a household.
pub async fn load(state: &AppState, household: &str, user_id: &str, id_or_slug: &str) -> AppResult<Recipe> {
    let sql = format!("SELECT {RECIPE_COLS}, {FAV_EXPR} FROM recipes r WHERE r.household_id = $2 AND (r.id = $3 OR r.slug = $3)");
    let row = sqlx::query(&sql)
        .bind(user_id)
        .bind(household)
        .bind(id_or_slug)
        .fetch_optional(&state.db.pool)
        .await?
        .ok_or_else(|| AppError::not_found("recipe not found"))?;
    Ok(recipe_from_row(&row))
}

/// Loads many recipes (full) for a household; used by sync and exports.
pub async fn load_all(state: &AppState, household: &str, user_id: &str, since: Option<&str>) -> AppResult<Vec<Recipe>> {
    let rows = match since {
        Some(s) => {
            sqlx::query(&format!(
                "SELECT {RECIPE_COLS}, {FAV_EXPR} FROM recipes r WHERE r.household_id = $2 AND r.updated_at >= $3 ORDER BY r.updated_at"
            ))
            .bind(user_id)
            .bind(household)
            .bind(s)
            .fetch_all(&state.db.pool)
            .await?
        }
        None => {
            sqlx::query(&format!("SELECT {RECIPE_COLS}, {FAV_EXPR} FROM recipes r WHERE r.household_id = $2 ORDER BY r.name_lower"))
                .bind(user_id)
                .bind(household)
                .fetch_all(&state.db.pool)
                .await?
        }
    };
    Ok(rows.iter().map(recipe_from_row).collect())
}

async fn unique_slug(state: &AppState, household: &str, name: &str, exclude_id: Option<&str>) -> AppResult<String> {
    let base = larder_core::text::slugify(name);
    let rows = sqlx::query("SELECT slug, id FROM recipes WHERE household_id = $1 AND (slug = $2 OR slug LIKE $3)")
        .bind(household)
        .bind(&base)
        .bind(format!("{base}-%"))
        .fetch_all(&state.db.pool)
        .await?;
    let taken: std::collections::HashSet<String> =
        rows.iter().filter(|r| Some(r.s("id").as_str()) != exclude_id).map(|r| r.s("slug")).collect();
    if !taken.contains(&base) {
        return Ok(base);
    }
    for n in 2.. {
        let cand = format!("{base}-{n}");
        if !taken.contains(&cand) {
            return Ok(cand);
        }
    }
    unreachable!()
}

pub struct InsertOpts<'a> {
    pub id: Option<String>,
    pub created_by: Option<&'a str>,
    pub created_at: Option<String>,
    pub image_id: Option<String>,
    /// Status and trial counters for the new recipe (normally just a status;
    /// restored backups and migrations also carry the counters).
    pub trial: TrialState,
}

/// Inserts a new recipe. Returns `(recipe, created)`; if a recipe with the same
/// id already exists in the household it is returned unchanged with `created = false`.
pub async fn insert(
    state: &AppState,
    household: &str,
    user_id: &str,
    mut data: RecipeData,
    opts: InsertOpts<'_>,
) -> AppResult<(Recipe, bool)> {
    data.normalize();
    data.validate().map_err(AppError::bad_request)?;
    let mut trial = opts.trial;
    trial.normalize();
    let id = crate::db::id_or_new(opts.id.as_deref())?;
    if let Some(row) = sqlx::query("SELECT household_id FROM recipes WHERE id = $1").bind(&id).fetch_optional(&state.db.pool).await? {
        if row.s("household_id") == household {
            return Ok((load(state, household, user_id, &id).await?, false));
        }
        return Err(AppError::conflict("recipe id already in use"));
    }
    let ts = now();
    let created_at = opts.created_at.unwrap_or_else(|| ts.clone());
    // Slug collisions under concurrent inserts are retried a few times.
    for attempt in 0..5 {
        let slug = unique_slug(state, household, &data.name, None).await?;
        let mut tx = state.db.pool.begin().await?;
        let res = sqlx::query(
            "INSERT INTO recipes (id, household_id, slug, name, description, yield_quantity, yield_unit, prep_time_minutes, \
             cook_time_minutes, total_time_minutes, source_url, source_name, notes, rating, ingredients, instructions, tags, \
             categories, nutrition, image_id, search_text, name_lower, created_by, created_at, updated_at, version, status, \
             tried_count, last_tried_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,1,$26,$27,$28)",
        )
        .bind(&id)
        .bind(household)
        .bind(&slug)
        .bind(&data.name)
        .bind(&data.description)
        .bind(data.yield_quantity)
        .bind(&data.yield_unit)
        .bind(data.prep_time_minutes)
        .bind(data.cook_time_minutes)
        .bind(data.total_time_minutes)
        .bind(data.source_url.clone())
        .bind(data.source_name.clone())
        .bind(&data.notes)
        .bind(data.rating)
        .bind(serde_json::to_string(&data.ingredients)?)
        .bind(serde_json::to_string(&data.instructions)?)
        .bind(serde_json::to_string(&data.tags)?)
        .bind(serde_json::to_string(&data.categories)?)
        .bind(serde_json::to_string(&data.nutrition)?)
        .bind(opts.image_id.clone())
        .bind(data.search_text())
        .bind(larder_core::text::fold(&data.name))
        .bind(opts.created_by.map(str::to_string))
        .bind(&created_at)
        .bind(&ts)
        .bind(trial.status.as_str())
        .bind(trial.tried_count)
        .bind(trial.last_tried_at.clone())
        .execute(&mut *tx)
        .await;
        match res {
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() && attempt < 4 => {
                let _ = tx.rollback().await;
                continue;
            }
            Err(e) => return Err(e.into()),
            Ok(_) => {}
        }
        write_tags(&mut tx, &id, household, &data).await?;
        sqlx::query("DELETE FROM deleted_recipes WHERE id = $1").bind(&id).execute(&mut *tx).await?;
        tx.commit().await?;
        return Ok((load(state, household, user_id, &id).await?, true));
    }
    Err(AppError::conflict("could not allocate a unique slug"))
}

async fn write_tags(tx: &mut sqlx::Transaction<'_, sqlx::Any>, id: &str, household: &str, data: &RecipeData) -> AppResult<()> {
    sqlx::query("DELETE FROM recipe_tags WHERE recipe_id = $1").bind(id).execute(&mut **tx).await?;
    for (kind, list) in [("tag", &data.tags), ("category", &data.categories)] {
        for name in list {
            sqlx::query("INSERT INTO recipe_tags (recipe_id, household_id, kind, name, name_lower) VALUES ($1, $2, $3, $4, $5)")
                .bind(id)
                .bind(household)
                .bind(kind)
                .bind(name)
                .bind(name.to_lowercase())
                .execute(&mut **tx)
                .await?;
        }
    }
    Ok(())
}

/// Replaces a recipe's content and, when given, its status (the trial
/// counters are only changed by [`record_tried`]). `expected_version` enables
/// optimistic concurrency.
pub async fn update(
    state: &AppState,
    household: &str,
    user_id: &str,
    id: &str,
    mut data: RecipeData,
    status: Option<RecipeStatus>,
    expected_version: Option<i64>,
) -> AppResult<Recipe> {
    data.normalize();
    data.validate().map_err(AppError::bad_request)?;
    let current = load(state, household, user_id, id).await?;
    if let Some(v) = expected_version {
        if v != current.version {
            return Err(AppError::conflict("the recipe was changed by someone else").with("current", serde_json::to_value(&current)?));
        }
    }
    let slug = if current.data.name == data.name {
        current.slug.clone()
    } else {
        unique_slug(state, household, &data.name, Some(&current.id)).await?
    };
    let mut tx = state.db.pool.begin().await?;
    sqlx::query(
        "UPDATE recipes SET slug=$1, name=$2, description=$3, yield_quantity=$4, yield_unit=$5, prep_time_minutes=$6, \
         cook_time_minutes=$7, total_time_minutes=$8, source_url=$9, source_name=$10, notes=$11, rating=$12, ingredients=$13, \
         instructions=$14, tags=$15, categories=$16, nutrition=$17, search_text=$18, name_lower=$19, updated_at=$20, \
         status=$23, version = version + 1 WHERE id = $21 AND household_id = $22",
    )
    .bind(&slug)
    .bind(&data.name)
    .bind(&data.description)
    .bind(data.yield_quantity)
    .bind(&data.yield_unit)
    .bind(data.prep_time_minutes)
    .bind(data.cook_time_minutes)
    .bind(data.total_time_minutes)
    .bind(data.source_url.clone())
    .bind(data.source_name.clone())
    .bind(&data.notes)
    .bind(data.rating)
    .bind(serde_json::to_string(&data.ingredients)?)
    .bind(serde_json::to_string(&data.instructions)?)
    .bind(serde_json::to_string(&data.tags)?)
    .bind(serde_json::to_string(&data.categories)?)
    .bind(serde_json::to_string(&data.nutrition)?)
    .bind(data.search_text())
    .bind(larder_core::text::fold(&data.name))
    .bind(now())
    .bind(&current.id)
    .bind(household)
    .bind(status.unwrap_or(current.trial.status).as_str())
    .execute(&mut *tx)
    .await?;
    write_tags(&mut tx, &current.id, household, &data).await?;
    tx.commit().await?;
    load(state, household, user_id, &current.id).await
}

/// "I made this": the household's verdict after making a recipe.
#[derive(Debug, Clone)]
pub struct Verdict {
    /// Move the recipe to the cookbook ("Keep it"); otherwise the status is
    /// left alone ("Try again").
    pub keep: bool,
    /// When it was made (normalised RFC 3339). Also identifies the verdict:
    /// a replay with the same `tried_at` changes nothing.
    pub tried_at: String,
    /// New star rating (1-5), or `None` to keep the current one.
    pub rating: Option<i64>,
    /// Dated line to append to the notes (see [`larder_core::trial::note_line`]).
    pub note_line: Option<String>,
}

/// Records that a recipe was made: `tried_count + 1`, `last_tried_at`, and
/// optionally the cookbook status, a rating and a note line, in one atomic
/// statement so verdicts from several devices never lose an increment.
/// Replaying the same verdict (same `tried_at`, e.g. an offline outbox retry
/// after a lost response) is a no-op that returns the recipe.
pub async fn record_tried(state: &AppState, household: &str, user_id: &str, id: &str, v: Verdict) -> AppResult<Recipe> {
    let current = load(state, household, user_id, id).await?;
    let line = v.note_line.unwrap_or_default();
    sqlx::query(
        "UPDATE recipes SET tried_count = tried_count + 1, \
         last_tried_at = CASE WHEN last_tried_at IS NULL OR last_tried_at < $1 THEN $1 ELSE last_tried_at END, \
         status = COALESCE($2, status), rating = COALESCE($3, rating), \
         notes = CASE WHEN $4 = '' THEN notes WHEN notes = '' THEN $4 ELSE notes || $5 END, \
         updated_at = $6, version = version + 1 \
         WHERE id = $7 AND household_id = $8 AND (last_tried_at IS NULL OR last_tried_at <> $1)",
    )
    .bind(&v.tried_at)
    .bind(v.keep.then_some(RecipeStatus::Cookbook.as_str()))
    .bind(v.rating)
    .bind(&line)
    .bind(format!("\n{line}"))
    .bind(now())
    .bind(&current.id)
    .bind(household)
    .execute(&state.db.pool)
    .await?;
    load(state, household, user_id, &current.id).await
}

/// Bumps updated_at (e.g. after a favorite change) so sync picks it up. The
/// content version is left alone so concurrent edits don't conflict spuriously.
pub async fn touch(state: &AppState, id: &str) -> AppResult<()> {
    sqlx::query("UPDATE recipes SET updated_at = $1 WHERE id = $2").bind(now()).bind(id).execute(&state.db.pool).await?;
    Ok(())
}

pub async fn set_image(state: &AppState, household: &str, id: &str, image_id: Option<String>) -> AppResult<()> {
    let old = sqlx::query("SELECT image_id FROM recipes WHERE id = $1 AND household_id = $2")
        .bind(id)
        .bind(household)
        .fetch_optional(&state.db.pool)
        .await?
        .ok_or_else(|| AppError::not_found("recipe not found"))?
        .os("image_id");
    sqlx::query("UPDATE recipes SET image_id = $1, updated_at = $2 WHERE id = $3")
        .bind(image_id)
        .bind(now())
        .bind(id)
        .execute(&state.db.pool)
        .await?;
    if let Some(old) = old {
        state.images.delete(&old).await;
    }
    Ok(())
}

pub async fn delete(state: &AppState, household: &str, id: &str) -> AppResult<()> {
    let row = sqlx::query("SELECT id, image_id FROM recipes WHERE household_id = $1 AND (id = $2 OR slug = $2)")
        .bind(household)
        .bind(id)
        .fetch_optional(&state.db.pool)
        .await?
        .ok_or_else(|| AppError::not_found("recipe not found"))?;
    let rid = row.s("id");
    let mut tx = state.db.pool.begin().await?;
    sqlx::query("DELETE FROM recipes WHERE id = $1").bind(&rid).execute(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO deleted_recipes (id, household_id, deleted_at) VALUES ($1, $2, $3) ON CONFLICT (id) DO UPDATE SET deleted_at = $3",
    )
    .bind(&rid)
    .bind(household)
    .bind(now())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    if let Some(img) = row.os("image_id") {
        state.images.delete(&img).await;
    }
    Ok(())
}

/// Finds an existing recipe that looks like a duplicate (same source URL or same name).
pub async fn find_duplicate(state: &AppState, household: &str, data: &RecipeData) -> AppResult<Option<String>> {
    if let Some(url) = &data.source_url {
        if let Some(r) = sqlx::query("SELECT id FROM recipes WHERE household_id = $1 AND source_url = $2")
            .bind(household)
            .bind(url)
            .fetch_optional(&state.db.pool)
            .await?
        {
            return Ok(Some(r.s("id")));
        }
    }
    let r = sqlx::query("SELECT id FROM recipes WHERE household_id = $1 AND name_lower = $2")
        .bind(household)
        .bind(larder_core::text::fold(&data.name))
        .fetch_optional(&state.db.pool)
        .await?;
    Ok(r.map(|r| r.s("id")))
}
