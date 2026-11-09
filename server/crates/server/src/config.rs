use std::path::PathBuf;

use anyhow::Context;

/// Runtime configuration, read from `LARDER_*` environment variables.
#[derive(Clone, Debug)]
pub struct Config {
    pub bind: String,
    pub database_url: String,
    pub data_dir: PathBuf,
    pub web_dir: Option<PathBuf>,
    pub public_url: String,
    pub jwt_secret: Vec<u8>,
    pub allow_registration: bool,
    /// Whether household members may create public share links for recipes.
    pub public_sharing: bool,
    pub cors_origins: Vec<String>,
    pub max_upload_bytes: usize,
    pub oidc: Option<OidcConfig>,
    pub llm: Option<LlmConfig>,
}

#[derive(Clone, Debug)]
pub struct OidcConfig {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub display_name: String,
    pub scopes: String,
    pub auto_register: bool,
    /// Claim used as username for new users.
    pub username_claim: String,
    /// Members of this group (in the `groups` claim) become server admins.
    pub admin_group: Option<String>,
}

pub use larder_assist::{LlmConfig, LlmProvider};

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn env_bool(key: &str, default: bool) -> bool {
    env(key).map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on")).unwrap_or(default)
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let data_dir = PathBuf::from(env("LARDER_DATA_DIR").unwrap_or_else(|| "./data".into()));
        std::fs::create_dir_all(&data_dir).with_context(|| format!("creating data dir {}", data_dir.display()))?;
        let database_url = env("LARDER_DATABASE_URL")
            .or_else(|| env("DATABASE_URL"))
            .unwrap_or_else(|| format!("sqlite://{}", data_dir.join("larder.db").display()));
        let bind = env("LARDER_BIND").unwrap_or_else(|| "0.0.0.0:8080".into());
        let public_url = env("LARDER_PUBLIC_URL")
            .unwrap_or_else(|| format!("http://localhost:{}", bind.rsplit(':').next().unwrap_or("8080")))
            .trim_end_matches('/')
            .to_string();
        let jwt_secret = match env("LARDER_JWT_SECRET") {
            Some(s) => s.into_bytes(),
            None => load_or_create_secret(&data_dir)?,
        };
        let web_dir = env("LARDER_WEB_DIR").map(PathBuf::from).or_else(|| {
            let p = PathBuf::from("./web");
            p.join("index.html").exists().then_some(p)
        });

        let oidc = match (env("LARDER_OIDC_ISSUER"), env("LARDER_OIDC_CLIENT_ID")) {
            (Some(issuer), Some(client_id)) => Some(OidcConfig {
                issuer: issuer.trim_end_matches('/').to_string(),
                client_id,
                client_secret: env("LARDER_OIDC_CLIENT_SECRET"),
                display_name: env("LARDER_OIDC_NAME").unwrap_or_else(|| "SSO".into()),
                scopes: env("LARDER_OIDC_SCOPES").unwrap_or_else(|| "openid profile email".into()),
                auto_register: env_bool("LARDER_OIDC_AUTO_REGISTER", true),
                username_claim: env("LARDER_OIDC_USERNAME_CLAIM").unwrap_or_else(|| "preferred_username".into()),
                admin_group: env("LARDER_OIDC_ADMIN_GROUP"),
            }),
            _ => None,
        };

        let provider = env("LARDER_LLM_PROVIDER").unwrap_or_default();
        let key = env("LARDER_LLM_API_KEY").or_else(|| match provider.to_ascii_lowercase().as_str() {
            "anthropic" | "claude" => env("ANTHROPIC_API_KEY"),
            "openai" => env("OPENAI_API_KEY"),
            _ => None,
        });
        let llm = LlmConfig::from_parts(&provider, key, env("LARDER_LLM_MODEL"), env("LARDER_LLM_BASE_URL")).map_err(anyhow::Error::msg)?;

        Ok(Self {
            bind,
            database_url,
            data_dir,
            web_dir,
            public_url,
            jwt_secret,
            allow_registration: env_bool("LARDER_ALLOW_REGISTRATION", false),
            public_sharing: env_bool("LARDER_PUBLIC_SHARING", true),
            cors_origins: env("LARDER_CORS_ORIGINS")
                .map(|s| s.split(',').map(|o| o.trim().to_string()).filter(|o| !o.is_empty()).collect())
                .unwrap_or_default(),
            max_upload_bytes: env("LARDER_MAX_UPLOAD_MB").and_then(|v| v.parse::<usize>().ok()).unwrap_or(200) * 1024 * 1024,
            oidc,
            llm,
        })
    }

    /// Configuration for tests: temp data dir, in-memory-ish sqlite.
    pub fn for_tests(data_dir: PathBuf, database_url: String) -> Self {
        Self {
            bind: "127.0.0.1:0".into(),
            database_url,
            data_dir,
            web_dir: None,
            public_url: "http://localhost:8080".into(),
            jwt_secret: b"test-secret-test-secret-test-secret".to_vec(),
            allow_registration: false,
            public_sharing: true,
            cors_origins: vec![],
            max_upload_bytes: 50 * 1024 * 1024,
            oidc: None,
            llm: None,
        }
    }
}

fn load_or_create_secret(data_dir: &std::path::Path) -> anyhow::Result<Vec<u8>> {
    let path = data_dir.join("jwt_secret");
    if let Ok(s) = std::fs::read_to_string(&path) {
        if s.trim().len() >= 32 {
            return Ok(s.trim().as_bytes().to_vec());
        }
    }
    use rand::RngCore;
    let mut bytes = [0u8; 48];
    rand::thread_rng().fill_bytes(&mut bytes);
    let secret = hex::encode(bytes);
    std::fs::write(&path, &secret).with_context(|| format!("writing {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(secret.into_bytes())
}
