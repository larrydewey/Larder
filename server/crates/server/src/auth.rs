//! Password hashing, JWT access tokens, refresh tokens and the `AuthUser` extractor.

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::db::{now, RowExt};
use crate::error::{AppError, AppResult};
use crate::models::User;
use crate::state::AppState;

pub const ACCESS_TTL_SECS: i64 = 15 * 60;
pub const REFRESH_TTL_DAYS: i64 = 60;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
    pub iat: i64,
}

pub fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::internal(format!("hashing password: {e}")))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).map(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()).unwrap_or(false)
}

pub fn validate_password(password: &str) -> AppResult<()> {
    if password.chars().count() < 8 {
        return Err(AppError::bad_request("password must be at least 8 characters"));
    }
    if password.len() > 1024 {
        return Err(AppError::bad_request("password is too long"));
    }
    Ok(())
}

pub fn issue_access_token(state: &AppState, user_id: &str) -> AppResult<String> {
    let iat = chrono::Utc::now().timestamp();
    let claims = Claims { sub: user_id.to_string(), iat, exp: iat + ACCESS_TTL_SECS };
    encode(&Header::new(Algorithm::HS256), &claims, &EncodingKey::from_secret(&state.config.jwt_secret))
        .map_err(|e| AppError::internal(format!("signing token: {e}")))
}

pub fn random_token(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

pub fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

pub async fn issue_refresh_token(state: &AppState, user_id: &str) -> AppResult<String> {
    let token = random_token(32);
    let expires = crate::db::format_ts(chrono::Utc::now() + chrono::Duration::days(REFRESH_TTL_DAYS));
    sqlx::query("INSERT INTO refresh_tokens (token_hash, user_id, created_at, expires_at) VALUES ($1, $2, $3, $4)")
        .bind(sha256_hex(&token))
        .bind(user_id)
        .bind(now())
        .bind(expires)
        .execute(&state.db.pool)
        .await?;
    Ok(token)
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub user: User,
}

pub async fn auth_response(state: &AppState, user: User) -> AppResult<AuthResponse> {
    Ok(AuthResponse {
        access_token: issue_access_token(state, &user.id)?,
        refresh_token: issue_refresh_token(state, &user.id).await?,
        expires_in: ACCESS_TTL_SECS,
        user,
    })
}

/// Exchanges a refresh token for a new one (rotation) and returns the user id.
pub async fn consume_refresh_token(state: &AppState, token: &str) -> AppResult<String> {
    let hash = sha256_hex(token);
    let row = sqlx::query("DELETE FROM refresh_tokens WHERE token_hash = $1 RETURNING user_id, expires_at")
        .bind(&hash)
        .fetch_optional(&state.db.pool)
        .await?
        .ok_or_else(|| AppError::unauthorized("invalid or expired refresh token"))?;
    if row.s("expires_at") < now() {
        return Err(AppError::unauthorized("invalid or expired refresh token"));
    }
    Ok(row.s("user_id"))
}

pub async fn revoke_refresh_token(state: &AppState, token: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM refresh_tokens WHERE token_hash = $1").bind(sha256_hex(token)).execute(&state.db.pool).await?;
    Ok(())
}

pub async fn prune_expired_tokens(state: &AppState) {
    let _ = sqlx::query("DELETE FROM refresh_tokens WHERE expires_at < $1").bind(now()).execute(&state.db.pool).await;
}

/// The authenticated user making a request.
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: String,
    pub household_id: String,
    pub is_admin: bool,
    pub is_owner: bool,
    pub user: User,
}

impl AuthUser {
    pub fn require_owner(&self) -> AppResult<()> {
        if self.is_owner {
            Ok(())
        } else {
            Err(AppError::forbidden("only household owners can do this"))
        }
    }
    pub fn require_admin(&self) -> AppResult<()> {
        if self.is_admin {
            Ok(())
        } else {
            Err(AppError::forbidden("administrator access required"))
        }
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| AppError::unauthorized("missing bearer token"))?;
        let token = header
            .strip_prefix("Bearer ")
            .or_else(|| header.strip_prefix("bearer "))
            .ok_or_else(|| AppError::unauthorized("missing bearer token"))?;
        let data = decode::<Claims>(token.trim(), &DecodingKey::from_secret(&state.config.jwt_secret), &Validation::new(Algorithm::HS256))
            .map_err(|_| AppError::unauthorized("invalid or expired token"))?;
        let user =
            crate::models::load_user(state, &data.claims.sub).await?.ok_or_else(|| AppError::unauthorized("user no longer exists"))?;
        if user.disabled {
            return Err(AppError::unauthorized("account disabled"));
        }
        Ok(AuthUser {
            id: user.id.clone(),
            household_id: user.household_id.clone(),
            is_admin: user.is_admin,
            is_owner: user.household_role == "owner",
            user,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_roundtrip() {
        let h = hash_password("correct horse").unwrap();
        assert!(verify_password("correct horse", &h));
        assert!(!verify_password("wrong", &h));
        assert!(!verify_password("x", "not a hash"));
    }
}
