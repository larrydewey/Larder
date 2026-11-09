//! Config, authentication, users, households and admin endpoints.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::{self, AuthUser};
use crate::db::{format_ts, now, RowExt};
use crate::error::{AppError, AppResult};
use crate::models::{self, NewUser, User};
use crate::state::AppState;

pub async fn public_config(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let users = models::user_count(&state).await?;
    Ok(Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "setup_required": users == 0,
        "registration_open": state.config.allow_registration,
        "public_sharing": state.config.public_sharing,
        "oidc": {
            "enabled": state.config.oidc.is_some(),
            "name": state.config.oidc.as_ref().map(|o| o.display_name.clone()),
        },
        "llm_enabled": state.ai.client().is_some(),
        "database": state.db.kind.as_str(),
    })))
}

#[derive(Deserialize)]
pub struct RegisterReq {
    username: String,
    password: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    invite_code: Option<String>,
}

pub async fn register(State(state): State<AppState>, Json(req): Json<RegisterReq>) -> AppResult<impl IntoResponse> {
    let username = models::validate_username(&req.username)?;
    auth::validate_password(&req.password)?;
    let email = models::normalize_email(req.email)?;
    let display_name = req.display_name.map(|d| d.trim().to_string()).filter(|d| !d.is_empty()).unwrap_or_else(|| username.clone());
    let first = models::user_count(&state).await? == 0;
    let invite = match req.invite_code.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
        Some(code) => {
            Some(models::invite_household(&state, code).await?.ok_or_else(|| AppError::bad_request("invalid or expired invite code"))?)
        }
        None => None,
    };
    let (household_id, role) = if let Some(h) = invite {
        (h, "member")
    } else if first || state.config.allow_registration {
        (models::create_household(&state, &format!("{display_name}'s Kitchen")).await?, "owner")
    } else {
        return Err(AppError::forbidden("registration is closed; ask a household owner for an invite code"));
    };
    let user = models::insert_user(
        &state,
        NewUser {
            username: &username,
            display_name: &display_name,
            email,
            password_hash: Some(auth::hash_password(&req.password)?),
            oidc_subject: None,
            is_admin: first,
            household_id: &household_id,
            role,
        },
    )
    .await?;
    if first {
        tracing::info!(username = %user.username, "created initial administrator");
    }
    Ok((StatusCode::CREATED, Json(auth::auth_response(&state, user).await?)))
}

#[derive(Deserialize)]
pub struct LoginReq {
    username: String,
    password: String,
}

pub async fn login(State(state): State<AppState>, Json(req): Json<LoginReq>) -> AppResult<Json<auth::AuthResponse>> {
    let user = models::find_user_by_login(&state, &req.username).await?;
    let ok = match &user {
        Some(u) => u.password_hash.as_deref().is_some_and(|h| auth::verify_password(&req.password, h)),
        None => {
            // Spend comparable time to avoid user enumeration via timing.
            static DUMMY: std::sync::LazyLock<String> =
                std::sync::LazyLock::new(|| auth::hash_password("timing-equaliser").unwrap_or_default());
            let _ = auth::verify_password(&req.password, &DUMMY);
            false
        }
    };
    let user = user.filter(|_| ok).ok_or_else(|| AppError::unauthorized("invalid username or password"))?;
    if user.disabled {
        return Err(AppError::forbidden("this account is disabled"));
    }
    auth::prune_expired_tokens(&state).await;
    Ok(Json(auth::auth_response(&state, user).await?))
}

#[derive(Deserialize)]
pub struct RefreshReq {
    refresh_token: String,
}

pub async fn refresh(State(state): State<AppState>, Json(req): Json<RefreshReq>) -> AppResult<Json<auth::AuthResponse>> {
    let user_id = auth::consume_refresh_token(&state, &req.refresh_token).await?;
    let user = models::load_user(&state, &user_id).await?.ok_or_else(|| AppError::unauthorized("user no longer exists"))?;
    if user.disabled {
        return Err(AppError::unauthorized("account disabled"));
    }
    Ok(Json(auth::auth_response(&state, user).await?))
}

pub async fn logout(State(state): State<AppState>, _user: AuthUser, Json(req): Json<RefreshReq>) -> AppResult<StatusCode> {
    auth::revoke_refresh_token(&state, &req.refresh_token).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn me(user: AuthUser) -> Json<User> {
    Json(user.user)
}

#[derive(Deserialize)]
pub struct UpdateMeReq {
    display_name: Option<String>,
    /// Absent: unchanged. `null` or "": clear. String: set.
    #[serde(default, deserialize_with = "present_or_null")]
    email: Option<Option<String>>,
    preferences: Option<Value>,
    current_password: Option<String>,
    new_password: Option<String>,
    /// The caller's refresh token, kept when the password changes so this
    /// device stays signed in while every other session is revoked.
    refresh_token: Option<String>,
}

/// Distinguishes an explicit `null` (Some(None)) from an absent field (None).
fn present_or_null<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<String>>, D::Error> {
    Ok(Some(Option::<String>::deserialize(d)?))
}

pub async fn update_me(State(state): State<AppState>, user: AuthUser, Json(req): Json<UpdateMeReq>) -> AppResult<Json<User>> {
    let ts = now();
    if let Some(d) = req.display_name.as_deref().map(str::trim) {
        if d.is_empty() || d.len() > 100 {
            return Err(AppError::bad_request("display name must be 1-100 characters"));
        }
        sqlx::query("UPDATE users SET display_name = $1, updated_at = $2 WHERE id = $3")
            .bind(d)
            .bind(&ts)
            .bind(&user.id)
            .execute(&state.db.pool)
            .await?;
    }
    if let Some(e) = req.email {
        let e = models::normalize_email(e)?;
        sqlx::query("UPDATE users SET email = $1, email_lower = $2, updated_at = $3 WHERE id = $4")
            .bind(e.clone())
            .bind(e.map(|e| e.to_lowercase()))
            .bind(&ts)
            .bind(&user.id)
            .execute(&state.db.pool)
            .await?;
    }
    if let Some(p) = req.preferences {
        if !p.is_object() {
            return Err(AppError::bad_request("preferences must be an object"));
        }
        let mut merged = user.user.preferences.clone();
        for (k, v) in p.as_object().unwrap() {
            merged[k] = v.clone();
        }
        if let Some(u) = merged.get("units").and_then(|u| u.as_str()) {
            if !matches!(u, "original" | "metric" | "imperial") {
                return Err(AppError::bad_request("units must be original, metric or imperial"));
            }
        }
        sqlx::query("UPDATE users SET preferences = $1, updated_at = $2 WHERE id = $3")
            .bind(merged.to_string())
            .bind(&ts)
            .bind(&user.id)
            .execute(&state.db.pool)
            .await?;
    }
    if let Some(np) = req.new_password {
        auth::validate_password(&np)?;
        if let Some(h) = &user.user.password_hash {
            let cur = req.current_password.unwrap_or_default();
            if !auth::verify_password(&cur, h) {
                return Err(AppError::forbidden("current password is incorrect"));
            }
        }
        sqlx::query("UPDATE users SET password_hash = $1, updated_at = $2 WHERE id = $3")
            .bind(auth::hash_password(&np)?)
            .bind(&ts)
            .bind(&user.id)
            .execute(&state.db.pool)
            .await?;
        // Sign out every other session; keep the caller's own refresh token if it sent one.
        let keep = req.refresh_token.as_deref().map(auth::sha256_hex).unwrap_or_default();
        sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1 AND token_hash <> $2")
            .bind(&user.id)
            .bind(keep)
            .execute(&state.db.pool)
            .await?;
    }
    Ok(Json(models::load_user(&state, &user.id).await?.ok_or_else(|| AppError::not_found("user not found"))?))
}

// ---------- household ----------

pub async fn get_household(State(state): State<AppState>, user: AuthUser) -> AppResult<Json<models::Household>> {
    Ok(Json(models::load_household(&state, &user.household_id).await?))
}

#[derive(Deserialize)]
pub struct HouseholdPatch {
    name: String,
}

pub async fn update_household(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<HouseholdPatch>,
) -> AppResult<Json<models::Household>> {
    user.require_owner()?;
    let name = req.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(AppError::bad_request("name must be 1-100 characters"));
    }
    sqlx::query("UPDATE households SET name = $1 WHERE id = $2").bind(name).bind(&user.household_id).execute(&state.db.pool).await?;
    Ok(Json(models::load_household(&state, &user.household_id).await?))
}

fn invite_json(r: &sqlx::any::AnyRow) -> Value {
    json!({ "code": r.s("code"), "created_at": r.s("created_at"), "expires_at": r.s("expires_at"), "created_by": r.s("created_by") })
}

pub async fn list_invites(State(state): State<AppState>, user: AuthUser) -> AppResult<Json<Vec<Value>>> {
    user.require_owner()?;
    let rows = sqlx::query("SELECT * FROM invites WHERE household_id = $1 AND expires_at > $2 ORDER BY created_at DESC")
        .bind(&user.household_id)
        .bind(now())
        .fetch_all(&state.db.pool)
        .await?;
    Ok(Json(rows.iter().map(invite_json).collect()))
}

#[derive(Deserialize, Default)]
pub struct InviteReq {
    expires_in_days: Option<i64>,
}

pub async fn create_invite(State(state): State<AppState>, user: AuthUser, body: axum::body::Bytes) -> AppResult<impl IntoResponse> {
    user.require_owner()?;
    let req: InviteReq = serde_json::from_slice(&body).unwrap_or_default();
    let days = req.expires_in_days.unwrap_or(7).clamp(1, 365);
    // Unambiguous alphabet (no 0/O/1/I).
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let code: String = {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..8).map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char).collect()
    };
    let expires = format_ts(chrono::Utc::now() + chrono::Duration::days(days));
    sqlx::query("INSERT INTO invites (code, household_id, created_by, created_at, expires_at) VALUES ($1, $2, $3, $4, $5)")
        .bind(&code)
        .bind(&user.household_id)
        .bind(&user.id)
        .bind(now())
        .bind(&expires)
        .execute(&state.db.pool)
        .await?;
    let row = sqlx::query("SELECT * FROM invites WHERE code = $1").bind(&code).fetch_one(&state.db.pool).await?;
    Ok((StatusCode::CREATED, Json(invite_json(&row))))
}

pub async fn delete_invite(State(state): State<AppState>, user: AuthUser, Path(code): Path<String>) -> AppResult<StatusCode> {
    user.require_owner()?;
    sqlx::query("DELETE FROM invites WHERE code = $1 AND household_id = $2")
        .bind(code.to_uppercase())
        .bind(&user.household_id)
        .execute(&state.db.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct RoleReq {
    role: String,
}

pub async fn update_member(
    State(state): State<AppState>,
    user: AuthUser,
    Path(member_id): Path<String>,
    Json(req): Json<RoleReq>,
) -> AppResult<Json<models::Household>> {
    user.require_owner()?;
    if !matches!(req.role.as_str(), "owner" | "member") {
        return Err(AppError::bad_request("role must be owner or member"));
    }
    if member_id == user.id && req.role != "owner" {
        let owners = sqlx::query("SELECT COUNT(*) AS n FROM users WHERE household_id = $1 AND household_role = 'owner'")
            .bind(&user.household_id)
            .fetch_one(&state.db.pool)
            .await?
            .i("n");
        if owners <= 1 {
            return Err(AppError::bad_request("a household needs at least one owner"));
        }
    }
    let res = sqlx::query("UPDATE users SET household_role = $1 WHERE id = $2 AND household_id = $3")
        .bind(&req.role)
        .bind(&member_id)
        .bind(&user.household_id)
        .execute(&state.db.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::not_found("member not found"));
    }
    Ok(Json(models::load_household(&state, &user.household_id).await?))
}

pub async fn remove_member(State(state): State<AppState>, user: AuthUser, Path(member_id): Path<String>) -> AppResult<StatusCode> {
    user.require_owner()?;
    if member_id == user.id {
        return Err(AppError::bad_request("you cannot remove yourself"));
    }
    let member = models::load_user(&state, &member_id).await?.filter(|m| m.household_id == user.household_id);
    let member = member.ok_or_else(|| AppError::not_found("member not found"))?;
    let new_household = models::create_household(&state, &format!("{}'s Kitchen", member.display_name)).await?;
    sqlx::query("UPDATE users SET household_id = $1, household_role = 'owner' WHERE id = $2")
        .bind(&new_household)
        .bind(&member.id)
        .execute(&state.db.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------- admin ----------

pub async fn admin_list_users(State(state): State<AppState>, user: AuthUser) -> AppResult<Json<Vec<Value>>> {
    user.require_admin()?;
    let rows = sqlx::query(&format!("SELECT {} FROM users ORDER BY username_lower", models::USER_COLS)).fetch_all(&state.db.pool).await?;
    Ok(Json(
        rows.iter()
            .map(|r| {
                let u = models::user_from_row(r);
                let mut v = serde_json::to_value(&u).unwrap_or_default();
                v["disabled"] = json!(u.disabled);
                v
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct AdminPatch {
    is_admin: Option<bool>,
    disabled: Option<bool>,
    new_password: Option<String>,
}

pub async fn admin_update_user(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    Json(req): Json<AdminPatch>,
) -> AppResult<Json<User>> {
    user.require_admin()?;
    let target = models::load_user(&state, &id).await?.ok_or_else(|| AppError::not_found("user not found"))?;
    if target.id == user.id && (req.is_admin == Some(false) || req.disabled == Some(true)) {
        return Err(AppError::bad_request("you cannot demote or disable yourself"));
    }
    if let Some(a) = req.is_admin {
        sqlx::query("UPDATE users SET is_admin = $1 WHERE id = $2").bind(a as i64).bind(&id).execute(&state.db.pool).await?;
    }
    if let Some(d) = req.disabled {
        sqlx::query("UPDATE users SET disabled = $1 WHERE id = $2").bind(d as i64).bind(&id).execute(&state.db.pool).await?;
        if d {
            sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1").bind(&id).execute(&state.db.pool).await?;
        }
    }
    if let Some(p) = req.new_password {
        auth::validate_password(&p)?;
        sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
            .bind(auth::hash_password(&p)?)
            .bind(&id)
            .execute(&state.db.pool)
            .await?;
        sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1").bind(&id).execute(&state.db.pool).await?;
    }
    Ok(Json(models::load_user(&state, &id).await?.ok_or_else(|| AppError::not_found("user not found"))?))
}

pub async fn admin_delete_user(State(state): State<AppState>, user: AuthUser, Path(id): Path<String>) -> AppResult<StatusCode> {
    user.require_admin()?;
    if id == user.id {
        return Err(AppError::bad_request("you cannot delete yourself"));
    }
    let target = models::load_user(&state, &id).await?.ok_or_else(|| AppError::not_found("user not found"))?;
    let mut tx = state.db.pool.begin().await?;
    sqlx::query("DELETE FROM favorites WHERE user_id = $1").bind(&id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM refresh_tokens WHERE user_id = $1").bind(&id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM users WHERE id = $1").bind(&id).execute(&mut *tx).await?;
    // Drop the household if it is now empty.
    let remaining =
        sqlx::query("SELECT COUNT(*) AS n FROM users WHERE household_id = $1").bind(&target.household_id).fetch_one(&mut *tx).await?.i("n");
    tx.commit().await?;
    if remaining == 0 {
        let images = sqlx::query("SELECT image_id FROM recipes WHERE household_id = $1 AND image_id IS NOT NULL")
            .bind(&target.household_id)
            .fetch_all(&state.db.pool)
            .await?;
        sqlx::query("DELETE FROM households WHERE id = $1").bind(&target.household_id).execute(&state.db.pool).await?;
        for r in images {
            state.images.delete(&r.s("image_id")).await;
        }
    } else if target.household_role == "owner" {
        // Make sure the household still has an owner.
        let owners = sqlx::query("SELECT COUNT(*) AS n FROM users WHERE household_id = $1 AND household_role = 'owner'")
            .bind(&target.household_id)
            .fetch_one(&state.db.pool)
            .await?
            .i("n");
        if owners == 0 {
            let first = sqlx::query("SELECT id FROM users WHERE household_id = $1 ORDER BY created_at LIMIT 1")
                .bind(&target.household_id)
                .fetch_one(&state.db.pool)
                .await?
                .s("id");
            sqlx::query("UPDATE users SET household_role = 'owner' WHERE id = $1").bind(first).execute(&state.db.pool).await?;
        }
    }
    Ok(StatusCode::NO_CONTENT)
}
