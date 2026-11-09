//! The AI assistant (LLM) used for photo and text imports. It is configured either by
//! `LARDER_LLM_*` environment variables, which take precedence and cannot be changed at
//! runtime, or by a server admin in the app, stored in `server_settings` with the API key
//! encrypted (see [`crate::secrets`]). Changes made in the app apply without a restart.

use std::sync::{Arc, RwLock};

use larder_assist::{Llm, LlmConfig, LlmProvider};
use serde::{Deserialize, Serialize};

use crate::db::{now, RowExt};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Row name in `server_settings`.
const SETTING: &str = "ai";
/// Encryption context for the stored API key.
const KEY_CONTEXT: &str = "server_settings.ai.api_key";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AiSource {
    /// Set by `LARDER_LLM_*` environment variables (read-only in the app).
    Environment,
    /// Set by an admin in the app.
    Server,
    /// Not configured.
    Off,
}

/// What admins see about the configuration. Never contains the key itself.
#[derive(Clone, Debug, Serialize)]
pub struct AiStatus {
    pub source: AiSource,
    pub enabled: bool,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub has_key: bool,
    /// The last four characters of the key, for recognising it.
    pub key_hint: Option<String>,
    /// A key is stored but cannot be decrypted (the signing secret changed); enter it again.
    pub key_unreadable: bool,
}

/// The live assistant plus its description.
pub struct Ai {
    client: RwLock<Option<Arc<Llm>>>,
    status: RwLock<AiStatus>,
}

impl Ai {
    pub fn new(env: Option<&LlmConfig>, http: &reqwest::Client) -> Self {
        let (client, status) = match env {
            Some(cfg) => (Some(Arc::new(Llm::new(cfg.clone(), http.clone()))), status_for(AiSource::Environment, cfg, None, false)),
            None => (None, off_status(false)),
        };
        Self { client: RwLock::new(client), status: RwLock::new(status) }
    }

    /// The configured assistant, if any.
    pub fn client(&self) -> Option<Arc<Llm>> {
        self.client.read().expect("ai lock").clone()
    }

    pub fn status(&self) -> AiStatus {
        self.status.read().expect("ai lock").clone()
    }

    fn set(&self, client: Option<Arc<Llm>>, status: AiStatus) {
        *self.client.write().expect("ai lock") = client;
        *self.status.write().expect("ai lock") = status;
    }
}

fn provider_name(cfg: &LlmConfig, stored: Option<&str>) -> String {
    match (stored, cfg.provider) {
        (Some(p), _) => p.to_string(),
        (None, LlmProvider::Anthropic) => "anthropic".into(),
        (None, LlmProvider::OpenAi) => "openai".into(),
    }
}

fn hint(key: &str) -> Option<String> {
    let chars: Vec<char> = key.chars().collect();
    (chars.len() >= 12).then(|| chars[chars.len() - 4..].iter().collect())
}

fn status_for(source: AiSource, cfg: &LlmConfig, provider: Option<&str>, key_unreadable: bool) -> AiStatus {
    AiStatus {
        source,
        enabled: true,
        provider: Some(provider_name(cfg, provider)),
        model: Some(cfg.model.clone()),
        base_url: Some(cfg.base_url.clone()),
        has_key: cfg.api_key.is_some(),
        key_hint: cfg.api_key.as_deref().and_then(hint),
        key_unreadable,
    }
}

fn off_status(key_unreadable: bool) -> AiStatus {
    AiStatus {
        source: AiSource::Off,
        enabled: false,
        provider: None,
        model: None,
        base_url: None,
        has_key: false,
        key_hint: None,
        key_unreadable,
    }
}

/// The stored form. `model` and `base_url` are kept as entered (`None` = the provider's
/// default), so defaults can improve with new versions.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Stored {
    provider: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    api_key: Option<String>,
}

async fn read_stored(state: &AppState) -> AppResult<Option<Stored>> {
    let row = sqlx::query("SELECT value FROM server_settings WHERE name = $1").bind(SETTING).fetch_optional(&state.db.pool).await?;
    Ok(row.and_then(|r| serde_json::from_str(&r.s("value")).ok()))
}

/// Applies the stored settings at startup, unless environment variables configure the assistant.
pub async fn load(state: &AppState) -> anyhow::Result<()> {
    if state.config.llm.is_some() {
        return Ok(());
    }
    let Some(stored) = read_stored(state).await.map_err(|e| anyhow::anyhow!(e.message))? else {
        return Ok(());
    };
    let key = match &stored.api_key {
        Some(sealed) => {
            match state.secrets.open(sealed, KEY_CONTEXT) {
                Some(k) => Some(k),
                None => {
                    tracing::warn!("the saved AI assistant key cannot be decrypted (was the signing secret changed?); enter it again in Server settings");
                    state.ai.set(None, off_status(true));
                    return Ok(());
                }
            }
        }
        None => None,
    };
    match LlmConfig::from_parts(&stored.provider, key, stored.model.clone(), stored.base_url.clone()) {
        Ok(Some(cfg)) => {
            let status = status_for(AiSource::Server, &cfg, Some(&stored.provider), false);
            state.ai.set(Some(Arc::new(Llm::new(cfg, state.http.clone()))), status);
        }
        Ok(None) => {}
        Err(e) => tracing::warn!("ignoring the saved AI assistant settings: {e}"),
    }
    Ok(())
}

/// Admin input. `api_key` absent keeps the saved key (only if the provider and address are
/// unchanged, so a saved key is never sent somewhere new without being entered again).
#[derive(Debug, Deserialize)]
pub struct AiUpdate {
    pub provider: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
}

fn clean(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn validate(u: AiUpdate) -> AppResult<Stored> {
    let provider = u.provider.trim().to_ascii_lowercase();
    if !matches!(provider.as_str(), "anthropic" | "openai" | "ollama") {
        return Err(AppError::bad_request("provider must be anthropic, openai or ollama"));
    }
    let api_key = clean(u.api_key);
    if let Some(k) = &api_key {
        if k.len() < 8 || k.len() > 512 || !k.chars().all(|c| c.is_ascii_graphic()) {
            return Err(AppError::bad_request("the API key looks wrong: it should be 8 to 512 characters with no spaces"));
        }
    }
    let model = clean(u.model);
    if let Some(m) = &model {
        if m.len() > 128 || !m.chars().all(|c| c.is_ascii_alphanumeric() || "-._:/@".contains(c)) {
            return Err(AppError::bad_request("the model name may only contain letters, digits and - . _ : / @"));
        }
    }
    let base_url = clean(u.base_url);
    if let Some(b) = &base_url {
        let ok =
            b.len() <= 512 && url::Url::parse(b).map(|p| matches!(p.scheme(), "http" | "https") && p.host_str().is_some()).unwrap_or(false);
        if !ok {
            return Err(AppError::bad_request("the base URL must be an http or https address"));
        }
    }
    Ok(Stored { provider, model, base_url, api_key })
}

/// Saves new settings from an admin and switches to them immediately.
pub async fn save(state: &AppState, admin_id: &str, update: AiUpdate) -> AppResult<AiStatus> {
    if state.config.llm.is_some() {
        return Err(AppError::conflict("the AI assistant is set by LARDER_LLM_* environment variables; change them on the server instead"));
    }
    let mut new = validate(update)?;
    let mut key = new.api_key.clone();
    if key.is_none() {
        // Keep the saved key only for the same provider and address.
        if let Some(old) = read_stored(state).await? {
            if old.provider == new.provider && old.base_url == new.base_url {
                key = old.api_key.as_deref().and_then(|s| state.secrets.open(s, KEY_CONTEXT));
            }
        }
    }
    if key.is_none() && new.provider != "ollama" {
        return Err(AppError::bad_request("enter the API key"));
    }
    let cfg = LlmConfig::from_parts(&new.provider, key.clone(), new.model.clone(), new.base_url.clone())
        .map_err(AppError::bad_request)?
        .ok_or_else(|| AppError::bad_request("choose a provider"))?;
    new.api_key = match &key {
        Some(k) => Some(state.secrets.seal(k, KEY_CONTEXT).map_err(|e| AppError::internal(format!("{e:#}")))?),
        None => None,
    };
    let value = serde_json::to_string(&new).map_err(|e| AppError::internal(e.to_string()))?;
    sqlx::query(
        "INSERT INTO server_settings (name, value, updated_at, updated_by) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (name) DO UPDATE SET value = $2, updated_at = $3, updated_by = $4",
    )
    .bind(SETTING)
    .bind(&value)
    .bind(now())
    .bind(admin_id)
    .execute(&state.db.pool)
    .await?;
    let status = status_for(AiSource::Server, &cfg, Some(&new.provider), false);
    tracing::info!(admin = admin_id, provider = %new.provider, model = %cfg.model, "AI assistant settings changed");
    state.ai.set(Some(Arc::new(Llm::new(cfg, state.http.clone()))), status.clone());
    Ok(status)
}

/// Removes the saved settings and turns the assistant off.
pub async fn clear(state: &AppState, admin_id: &str) -> AppResult<()> {
    if state.config.llm.is_some() {
        return Err(AppError::conflict("the AI assistant is set by LARDER_LLM_* environment variables; change them on the server instead"));
    }
    sqlx::query("DELETE FROM server_settings WHERE name = $1").bind(SETTING).execute(&state.db.pool).await?;
    tracing::info!(admin = admin_id, "AI assistant turned off");
    state.ai.set(None, off_status(false));
    Ok(())
}

/// Defaults per provider, for the settings form.
pub fn defaults() -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for p in ["anthropic", "openai", "ollama"] {
        if let Ok(Some(c)) = LlmConfig::from_parts(p, None, None, None) {
            out.insert(p.into(), serde_json::json!({ "model": c.model, "base_url": c.base_url, "needs_key": p != "ollama" }));
        }
    }
    serde_json::Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upd(provider: &str, key: Option<&str>, model: Option<&str>, url: Option<&str>) -> AiUpdate {
        AiUpdate { provider: provider.into(), api_key: key.map(Into::into), model: model.map(Into::into), base_url: url.map(Into::into) }
    }

    #[test]
    fn validation() {
        assert!(validate(upd("anthropic", Some("sk-ant-0123456789"), None, None)).is_ok());
        assert!(validate(upd("Claude", None, None, None)).is_err());
        assert!(validate(upd("anthropic", Some("short"), None, None)).is_err());
        assert!(validate(upd("anthropic", Some("has space in it"), None, None)).is_err());
        assert!(validate(upd("openai", None, Some("gpt-4o; rm"), None)).is_err());
        assert!(validate(upd("ollama", None, Some("llama3.2-vision:11b"), Some("http://ollama:11434/v1"))).is_ok());
        assert!(validate(upd("ollama", None, None, Some("file:///etc/passwd"))).is_err());
        let s = validate(upd("openai", Some("  sk-abcdefgh  "), Some(" "), Some(""))).unwrap();
        assert_eq!(s.api_key.as_deref(), Some("sk-abcdefgh"));
        assert!(s.model.is_none() && s.base_url.is_none());
    }

    #[test]
    fn key_hint_only_for_real_keys() {
        assert_eq!(hint("sk-ant-api03-abcdWXYZ").as_deref(), Some("WXYZ"));
        assert_eq!(hint("shortkey"), None);
    }
}
