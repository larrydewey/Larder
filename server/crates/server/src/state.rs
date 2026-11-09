use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::config::Config;
use crate::db::Db;
use crate::images::ImageStore;
use larder_assist::Llm;

pub type AppState = Arc<Inner>;

pub struct Inner {
    pub config: Config,
    pub db: Db,
    pub images: ImageStore,
    pub http: reqwest::Client,
    /// Client for user-supplied URLs (imports); refuses local addresses.
    pub fetch: reqwest::Client,
    pub llm: Option<Llm>,
    /// Pending OIDC logins keyed by `state`.
    pub oidc_pending: Mutex<HashMap<String, OidcPending>>,
    /// Cached OIDC discovery document.
    pub oidc_discovery: tokio::sync::Mutex<Option<(Instant, serde_json::Value)>>,
}

#[derive(Clone, Debug)]
pub struct OidcPending {
    pub verifier: String,
    pub nonce: String,
    pub redirect_uri: String,
    pub invite_code: Option<String>,
    pub created: Instant,
}

impl Inner {
    pub fn new(config: Config, db: Db) -> anyhow::Result<AppState> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("Mozilla/5.0 (compatible; Larder/", env!("CARGO_PKG_VERSION"), "; +https://github.com/larrydewey/larder)"))
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(8))
            .build()?;
        let fetch = crate::netguard::guarded_client(larder_import::USER_AGENT)?;
        let images = ImageStore::new(config.data_dir.join("images"))?;
        let llm = config.llm.clone().map(|c| Llm::new(c, http.clone()));
        Ok(Arc::new(Self {
            config,
            db,
            images,
            http,
            fetch,
            llm,
            oidc_pending: Mutex::new(HashMap::new()),
            oidc_discovery: tokio::sync::Mutex::new(None),
        }))
    }
}
