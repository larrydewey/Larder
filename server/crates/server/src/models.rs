//! User / household models and lookups shared by several route modules.

use serde::Serialize;
use serde_json::Value;
use sqlx::any::AnyRow;

use crate::db::{new_id, now, RowExt};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub is_admin: bool,
    pub household_id: String,
    pub household_role: String,
    pub preferences: Value,
    pub created_at: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
    #[serde(skip)]
    pub password_hash: Option<String>,
}

pub const USER_COLS: &str =
    "id, username, display_name, email, is_admin, disabled, household_id, household_role, preferences, created_at, password_hash";

pub fn user_from_row(r: &AnyRow) -> User {
    let prefs: Value = r.os("preferences").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_else(|| serde_json::json!({}));
    User {
        id: r.s("id"),
        username: r.s("username"),
        display_name: r.s("display_name"),
        email: r.os("email"),
        is_admin: r.b("is_admin"),
        household_id: r.s("household_id"),
        household_role: r.s("household_role"),
        preferences: with_default_prefs(prefs),
        created_at: r.s("created_at"),
        disabled: r.b("disabled"),
        password_hash: r.os("password_hash"),
    }
}

fn with_default_prefs(mut v: Value) -> Value {
    if !v.is_object() {
        v = serde_json::json!({});
    }
    if v.get("units").and_then(|u| u.as_str()).is_none() {
        v["units"] = Value::String("original".into());
    }
    v
}

pub async fn load_user(state: &AppState, id: &str) -> AppResult<Option<User>> {
    let row = sqlx::query(&format!("SELECT {USER_COLS} FROM users WHERE id = $1")).bind(id).fetch_optional(&state.db.pool).await?;
    Ok(row.as_ref().map(user_from_row))
}

pub async fn find_user_by_login(state: &AppState, login: &str) -> AppResult<Option<User>> {
    let l = login.trim().to_lowercase();
    let row = sqlx::query(&format!("SELECT {USER_COLS} FROM users WHERE username_lower = $1 OR email_lower = $1"))
        .bind(&l)
        .fetch_optional(&state.db.pool)
        .await?;
    Ok(row.as_ref().map(user_from_row))
}

pub async fn user_count(state: &AppState) -> AppResult<i64> {
    let row = sqlx::query("SELECT COUNT(*) AS n FROM users").fetch_one(&state.db.pool).await?;
    Ok(row.i("n"))
}

pub fn validate_username(u: &str) -> AppResult<String> {
    let u = u.trim();
    if u.len() < 2 || u.len() > 64 {
        return Err(AppError::bad_request("username must be 2-64 characters"));
    }
    if !u.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '@')) {
        return Err(AppError::bad_request("username may only contain letters, numbers, '_', '-', '.', '@'"));
    }
    Ok(u.to_string())
}

pub fn normalize_email(e: Option<String>) -> AppResult<Option<String>> {
    match e.map(|e| e.trim().to_string()).filter(|e| !e.is_empty()) {
        None => Ok(None),
        Some(e) if e.contains('@') && e.len() <= 254 => Ok(Some(e)),
        Some(_) => Err(AppError::bad_request("invalid email address")),
    }
}

pub async fn create_household(state: &AppState, name: &str) -> AppResult<String> {
    let id = new_id();
    sqlx::query("INSERT INTO households (id, name, created_at) VALUES ($1, $2, $3)")
        .bind(&id)
        .bind(name)
        .bind(now())
        .execute(&state.db.pool)
        .await?;
    Ok(id)
}

pub struct NewUser<'a> {
    pub username: &'a str,
    pub display_name: &'a str,
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub oidc_subject: Option<String>,
    pub is_admin: bool,
    pub household_id: &'a str,
    pub role: &'a str,
}

pub async fn insert_user(state: &AppState, u: NewUser<'_>) -> AppResult<User> {
    let id = new_id();
    let ts = now();
    let res = sqlx::query(
        "INSERT INTO users (id, username, username_lower, display_name, email, email_lower, password_hash, oidc_subject, \
         is_admin, disabled, household_id, household_role, preferences, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 0, $10, $11, '{}', $12, $12)",
    )
    .bind(&id)
    .bind(u.username)
    .bind(u.username.to_lowercase())
    .bind(u.display_name)
    .bind(u.email.clone())
    .bind(u.email.as_ref().map(|e| e.to_lowercase()))
    .bind(u.password_hash)
    .bind(u.oidc_subject)
    .bind(u.is_admin as i64)
    .bind(u.household_id)
    .bind(u.role)
    .bind(&ts)
    .execute(&state.db.pool)
    .await;
    match res {
        Err(e) if matches!(&e, sqlx::Error::Database(d) if d.is_unique_violation()) => {
            return Err(AppError::conflict("username or email is already taken"))
        }
        Err(e) => return Err(e.into()),
        Ok(_) => {}
    }
    load_user(state, &id).await?.ok_or_else(|| AppError::internal("user vanished after insert"))
}

#[derive(Serialize)]
pub struct Member {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
}

#[derive(Serialize)]
pub struct Household {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub members: Vec<Member>,
}

pub async fn load_household(state: &AppState, id: &str) -> AppResult<Household> {
    let row = sqlx::query("SELECT id, name, created_at FROM households WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db.pool)
        .await?
        .ok_or_else(|| AppError::not_found("household not found"))?;
    let members = sqlx::query(
        "SELECT id, username, display_name, household_role FROM users WHERE household_id = $1 ORDER BY household_role DESC, username_lower",
    )
    .bind(id)
    .fetch_all(&state.db.pool)
    .await?
    .iter()
    .map(|r| Member { id: r.s("id"), username: r.s("username"), display_name: r.s("display_name"), role: r.s("household_role") })
    .collect();
    Ok(Household { id: row.s("id"), name: row.s("name"), created_at: row.s("created_at"), members })
}

/// Looks up a valid (unexpired) invite and returns its household id.
pub async fn invite_household(state: &AppState, code: &str) -> AppResult<Option<String>> {
    let row = sqlx::query("SELECT household_id, expires_at FROM invites WHERE code = $1")
        .bind(code.trim().to_uppercase())
        .fetch_optional(&state.db.pool)
        .await?;
    Ok(row.filter(|r| r.s("expires_at") > now()).map(|r| r.s("household_id")))
}
