//! Server settings for admins: the AI assistant.

use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};

use crate::ai::{self, AiUpdate};
use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

fn ai_json(state: &AppState) -> Value {
    let mut v = serde_json::to_value(state.ai.status()).unwrap_or_default();
    v["defaults"] = ai::defaults();
    v
}

/// `GET /api/admin/settings/ai`
pub async fn get_ai(State(state): State<AppState>, user: AuthUser) -> AppResult<Json<Value>> {
    user.require_admin()?;
    Ok(Json(ai_json(&state)))
}

/// `PUT /api/admin/settings/ai`
pub async fn put_ai(State(state): State<AppState>, user: AuthUser, Json(update): Json<AiUpdate>) -> AppResult<Json<Value>> {
    user.require_admin()?;
    ai::save(&state, &user.id, update).await?;
    Ok(Json(ai_json(&state)))
}

/// `DELETE /api/admin/settings/ai`
pub async fn delete_ai(State(state): State<AppState>, user: AuthUser) -> AppResult<StatusCode> {
    user.require_admin()?;
    ai::clear(&state, &user.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/admin/settings/ai/test`: one small request to check the key and model.
pub async fn test_ai(State(state): State<AppState>, user: AuthUser) -> AppResult<Json<Value>> {
    user.require_admin()?;
    let llm = state.ai.client().ok_or_else(AppError::llm_disabled)?;
    let lines = vec!["2 cups flour".to_string()];
    match tokio::time::timeout(Duration::from_secs(60), llm.parse_ingredients(&lines)).await {
        Ok(Ok(_)) => Ok(Json(json!({ "ok": true, "model": llm.describe() }))),
        Ok(Err(e)) => {
            let msg: String = format!("{e:#}").chars().take(300).collect();
            Err(AppError::unprocessable(format!("the AI assistant did not answer correctly: {msg}")))
        }
        Err(_) => Err(AppError::unprocessable("the AI assistant did not answer within a minute")),
    }
}
