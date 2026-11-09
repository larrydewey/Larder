//! Larder server library: configuration, database, HTTP routes.

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod images;
pub mod models;
pub mod netguard;
pub mod recipes;
pub mod routes;
pub mod state;

use state::AppState;

/// Builds the application state (connects + migrates the database).
pub async fn build_state(config: config::Config) -> anyhow::Result<AppState> {
    let db = db::Db::connect(&config.database_url).await?;
    db.migrate().await?;
    let state = state::Inner::new(config, db)?;
    routes::transfer::fail_stale_jobs(&state).await;
    Ok(state)
}

pub fn app(state: AppState) -> axum::Router {
    routes::router(state)
}
