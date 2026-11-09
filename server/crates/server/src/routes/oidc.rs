//! OpenID Connect login (authorization code flow with PKCE).
//!
//! The ID token is obtained directly from the provider's token endpoint over
//! TLS using the client credentials, and user claims are read from the
//! userinfo endpoint, so no local JWT signature verification is required.

use std::time::{Duration, Instant};

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use base64::Engine;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::auth::{self, random_token};
use crate::db::RowExt;
use crate::error::{AppError, AppResult};
use crate::models::{self, NewUser};
use crate::state::{AppState, OidcPending};

const APP_REDIRECT: &str = "larder://auth";

fn callback_url(state: &AppState) -> String {
    format!("{}/api/auth/oidc/callback", state.config.public_url)
}

async fn discovery(state: &AppState) -> AppResult<Value> {
    let oidc = state.config.oidc.as_ref().ok_or_else(|| AppError::not_found("OIDC is not configured"))?;
    let mut cache = state.oidc_discovery.lock().await;
    if let Some((at, doc)) = cache.as_ref() {
        if at.elapsed() < Duration::from_secs(3600) {
            return Ok(doc.clone());
        }
    }
    let url = format!("{}/.well-known/openid-configuration", oidc.issuer);
    let doc: Value = state
        .http
        .get(&url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| AppError::internal(format!("OIDC discovery failed: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::internal(format!("OIDC discovery returned invalid JSON: {e}")))?;
    *cache = Some((Instant::now(), doc.clone()));
    Ok(doc)
}

fn allowed_redirect(state: &AppState, uri: &str) -> bool {
    if uri == APP_REDIRECT || uri.starts_with("larder://") {
        return true;
    }
    if uri.starts_with('/') && !uri.starts_with("//") {
        return true;
    }
    let public = &state.config.public_url;
    uri == public.as_str()
        || uri.starts_with(&format!("{public}/"))
        || state.config.cors_origins.iter().any(|o| uri.starts_with(o.as_str()))
}

#[derive(Deserialize)]
pub struct LoginQuery {
    redirect_uri: Option<String>,
    invite_code: Option<String>,
}

pub async fn login(State(state): State<AppState>, Query(q): Query<LoginQuery>) -> AppResult<Response> {
    let oidc = state.config.oidc.clone().ok_or_else(|| AppError::not_found("OIDC is not configured"))?;
    let redirect_uri = q.redirect_uri.unwrap_or_else(|| "/oidc-callback".into());
    if !allowed_redirect(&state, &redirect_uri) {
        return Err(AppError::bad_request("redirect_uri is not allowed"));
    }
    let doc = discovery(&state).await?;
    let auth_endpoint =
        doc["authorization_endpoint"].as_str().ok_or_else(|| AppError::internal("OIDC provider has no authorization_endpoint"))?;
    let verifier = random_token(32);
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let st = random_token(16);
    let nonce = random_token(16);
    {
        let mut pending = state.oidc_pending.lock().unwrap();
        pending.retain(|_, p| p.created.elapsed() < Duration::from_secs(600));
        pending.insert(
            st.clone(),
            OidcPending { verifier, nonce: nonce.clone(), redirect_uri, invite_code: q.invite_code, created: Instant::now() },
        );
    }
    let mut url = url::Url::parse(auth_endpoint).map_err(|e| AppError::internal(format!("bad authorization_endpoint: {e}")))?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &oidc.client_id)
        .append_pair("redirect_uri", &callback_url(&state))
        .append_pair("scope", &oidc.scopes)
        .append_pair("state", &st)
        .append_pair("nonce", &nonce)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(Redirect::to(url.as_str()).into_response())
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

fn redirect_with_fragment(target: &str, fragment: &str) -> Response {
    Redirect::to(&format!("{target}#{fragment}")).into_response()
}

pub async fn callback(State(state): State<AppState>, Query(q): Query<CallbackQuery>) -> Response {
    let pending = q.state.as_deref().and_then(|s| state.oidc_pending.lock().unwrap().remove(s));
    let Some(pending) = pending else {
        return AppError::bad_request("unknown or expired login attempt; please try again").into_response();
    };
    if pending.created.elapsed() > Duration::from_secs(600) {
        return redirect_with_fragment(&pending.redirect_uri, "error=login%20expired");
    }
    if let Some(e) = q.error {
        let msg = q.error_description.unwrap_or(e);
        return redirect_with_fragment(&pending.redirect_uri, &format!("error={}", urlencoding::encode(&msg)));
    }
    match finish(&state, q.code.as_deref().unwrap_or(""), &pending).await {
        Ok(resp) => redirect_with_fragment(
            &pending.redirect_uri,
            &format!("access_token={}&refresh_token={}&expires_in={}", resp.access_token, resp.refresh_token, resp.expires_in),
        ),
        Err(e) => redirect_with_fragment(&pending.redirect_uri, &format!("error={}", urlencoding::encode(&e.message))),
    }
}

async fn finish(state: &AppState, code: &str, pending: &OidcPending) -> AppResult<auth::AuthResponse> {
    let oidc = state.config.oidc.clone().ok_or_else(|| AppError::not_found("OIDC is not configured"))?;
    if code.is_empty() {
        return Err(AppError::bad_request("missing authorization code"));
    }
    let doc = discovery(state).await?;
    let token_endpoint = doc["token_endpoint"].as_str().ok_or_else(|| AppError::internal("OIDC provider has no token_endpoint"))?;
    let callback = callback_url(state);
    let mut form = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", callback.as_str()),
        ("client_id", oidc.client_id.as_str()),
        ("code_verifier", pending.verifier.as_str()),
    ];
    if let Some(secret) = &oidc.client_secret {
        form.push(("client_secret", secret.as_str()));
    }
    let body = url::form_urlencoded::Serializer::new(String::new()).extend_pairs(form).finish();
    let tokens: Value = state
        .http
        .post(token_endpoint)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|e| AppError::internal(format!("token request failed: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::internal(format!("token response invalid: {e}")))?;
    let access = tokens["access_token"].as_str().ok_or_else(|| {
        AppError::unauthorized(format!(
            "token exchange failed: {}",
            tokens["error_description"].as_str().or(tokens["error"].as_str()).unwrap_or("no access token")
        ))
    })?;
    // Check the nonce from the ID token payload (transport-authenticated, see module docs).
    if let Some(id_token) = tokens["id_token"].as_str() {
        let payload =
            id_token.split('.').nth(1).and_then(|p| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(p.trim_end_matches('=')).ok());
        if let Some(claims) = payload.and_then(|p| serde_json::from_slice::<Value>(&p).ok()) {
            if claims["nonce"].as_str().is_some_and(|n| n != pending.nonce) {
                return Err(AppError::unauthorized("OIDC nonce mismatch"));
            }
        }
    }
    let userinfo_endpoint =
        doc["userinfo_endpoint"].as_str().ok_or_else(|| AppError::internal("OIDC provider has no userinfo_endpoint"))?;
    let info: Value = state
        .http
        .get(userinfo_endpoint)
        .bearer_auth(access)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| AppError::internal(format!("userinfo request failed: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::internal(format!("userinfo invalid: {e}")))?;
    let sub = info["sub"].as_str().ok_or_else(|| AppError::unauthorized("userinfo has no subject"))?;
    let subject = format!("{}|{}", oidc.issuer, sub);
    let is_group_admin = oidc
        .admin_group
        .as_ref()
        .is_some_and(|g| info["groups"].as_array().is_some_and(|gs| gs.iter().any(|x| x.as_str() == Some(g.as_str()))));

    // Existing user by subject?
    if let Some(row) = sqlx::query("SELECT id FROM users WHERE oidc_subject = $1").bind(&subject).fetch_optional(&state.db.pool).await? {
        let user = models::load_user(state, &row.s("id")).await?.ok_or_else(|| AppError::unauthorized("user not found"))?;
        if user.disabled {
            return Err(AppError::forbidden("this account is disabled"));
        }
        if is_group_admin && !user.is_admin {
            sqlx::query("UPDATE users SET is_admin = 1 WHERE id = $1").bind(&user.id).execute(&state.db.pool).await?;
        }
        let user = models::load_user(state, &user.id).await?.unwrap_or(user);
        return auth::auth_response(state, user).await;
    }
    // Link to an existing local account with the same verified email.
    let email = info["email"].as_str().map(str::to_string);
    let verified = info["email_verified"].as_bool().unwrap_or(false);
    if let (Some(e), true) = (&email, verified) {
        if let Some(row) = sqlx::query("SELECT id FROM users WHERE email_lower = $1 AND oidc_subject IS NULL")
            .bind(e.to_lowercase())
            .fetch_optional(&state.db.pool)
            .await?
        {
            let existing = models::load_user(state, &row.s("id")).await?.ok_or_else(|| AppError::unauthorized("user not found"))?;
            if existing.disabled {
                return Err(AppError::forbidden("this account is disabled"));
            }
            sqlx::query("UPDATE users SET oidc_subject = $1 WHERE id = $2")
                .bind(&subject)
                .bind(&existing.id)
                .execute(&state.db.pool)
                .await?;
            if is_group_admin && !existing.is_admin {
                sqlx::query("UPDATE users SET is_admin = 1 WHERE id = $1").bind(&existing.id).execute(&state.db.pool).await?;
            }
            let user = models::load_user(state, &existing.id).await?.unwrap_or(existing);
            return auth::auth_response(state, user).await;
        }
    }
    if !oidc.auto_register {
        return Err(AppError::forbidden("no account is linked to this identity"));
    }
    let first = models::user_count(state).await? == 0;
    let base_username = info[oidc.username_claim.as_str()]
        .as_str()
        .or(info["preferred_username"].as_str())
        .or(info["nickname"].as_str())
        .or(email.as_deref().and_then(|e| e.split('@').next()))
        .unwrap_or("user");
    let base_username: String = base_username.chars().filter(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.')).take(48).collect();
    let base_username = if base_username.len() < 2 { "user".to_string() } else { base_username };
    let mut username = base_username.clone();
    for n in 2..1000 {
        if models::find_user_by_login(state, &username).await?.is_none() {
            break;
        }
        username = format!("{base_username}{n}");
    }
    let display_name = info["name"].as_str().unwrap_or(&username).to_string();
    let (household_id, role) = match pending.invite_code.as_deref() {
        Some(code) => match models::invite_household(state, code).await? {
            Some(h) => (h, "member"),
            None => return Err(AppError::bad_request("invalid or expired invite code")),
        },
        None => (models::create_household(state, &format!("{display_name}'s Kitchen")).await?, "owner"),
    };
    // Don't attach an email that already belongs to someone else.
    let email = match &email {
        Some(e) if models::find_user_by_login(state, e).await?.is_some() => None,
        other => other.clone(),
    };
    let user = models::insert_user(
        state,
        NewUser {
            username: &username,
            display_name: &display_name,
            email,
            password_hash: None,
            oidc_subject: Some(subject),
            is_admin: first || is_group_admin,
            household_id: &household_id,
            role,
        },
    )
    .await?;
    auth::auth_response(state, user).await
}
