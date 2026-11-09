use larder_server::config::Config;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,tower_http=warn,sqlx=warn")))
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("larder {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.get(1).map(String::as_str) == Some("healthcheck") {
        return healthcheck();
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{}", HELP);
        return Ok(());
    }

    let config = Config::from_env()?;
    let bind = config.bind.clone();
    let state = larder_server::build_state(config).await?;
    tracing::info!(
        database = state.db.kind.as_str(),
        data_dir = %state.config.data_dir.display(),
        web = state.config.web_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "disabled".into()),
        oidc = state.config.oidc.is_some(),
        llm = state.ai.client().map(|l| l.describe()).unwrap_or_else(|| "disabled".into()),
        "starting Larder"
    );
    let app = larder_server::app(state);
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app).with_graceful_shutdown(shutdown()).await?;
    Ok(())
}

/// Minimal HTTP probe for container health checks (no curl in the image).
fn healthcheck() -> anyhow::Result<()> {
    use std::io::{Read, Write};
    let bind = std::env::var("LARDER_BIND").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let port = bind.rsplit(':').next().unwrap_or("8080");
    let mut stream = std::net::TcpStream::connect(format!("127.0.0.1:{port}"))?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    stream.write_all(b"GET /healthz HTTP/1.0\r\nHost: localhost\r\n\r\n")?;
    let mut buf = String::new();
    stream.read_to_string(&mut buf)?;
    if buf.starts_with("HTTP/1.0 200") || buf.starts_with("HTTP/1.1 200") {
        Ok(())
    } else {
        anyhow::bail!("unhealthy: {}", buf.lines().next().unwrap_or(""))
    }
}

async fn shutdown() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
    tracing::info!("shutting down");
}

const HELP: &str = "Larder server

Usage: larder [healthcheck]

Configuration is read from environment variables:
  LARDER_BIND                 listen address (default 0.0.0.0:8080)
  LARDER_DATABASE_URL         sqlite://path/to.db or postgres://user:pass@host/db
                                (default sqlite://$LARDER_DATA_DIR/larder.db)
  LARDER_DATA_DIR             data directory for images and secrets (default ./data)
  LARDER_WEB_DIR              directory with the Flutter web build (default ./web if present)
  LARDER_PUBLIC_URL           external URL, used for OIDC redirects
  LARDER_JWT_SECRET           token signing secret (generated and stored in the data dir if unset)
  LARDER_ALLOW_REGISTRATION   allow open sign-up (default false; invites always work)
  LARDER_CORS_ORIGINS         comma-separated origins allowed for cross-origin API use
  LARDER_MAX_UPLOAD_MB        maximum upload size in MB (default 200)
  LARDER_OIDC_ISSUER / _CLIENT_ID / _CLIENT_SECRET / _NAME / _SCOPES /
  LARDER_OIDC_AUTO_REGISTER / _USERNAME_CLAIM / _ADMIN_GROUP
  LARDER_LLM_PROVIDER         anthropic | openai | ollama | none
  LARDER_LLM_API_KEY / LARDER_LLM_MODEL / LARDER_LLM_BASE_URL
  RUST_LOG                      log filter (default info)";
