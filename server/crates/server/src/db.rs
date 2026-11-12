//! Database access. A single `sqlx::AnyPool` serves both SQLite and
//! PostgreSQL; SQL is written in the common subset (`$N` placeholders, TEXT
//! timestamps, integer booleans) and separate migration sets create
//! equivalent schemas.

use std::str::FromStr;
use std::time::Duration;

use anyhow::Context;
use sqlx::any::{AnyPoolOptions, AnyRow};
use sqlx::{AnyPool, Row};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DbKind {
    Sqlite,
    Postgres,
}

impl DbKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DbKind::Sqlite => "sqlite",
            DbKind::Postgres => "postgres",
        }
    }
}

#[derive(Clone)]
pub struct Db {
    pub pool: AnyPool,
    pub kind: DbKind,
}

static SQLITE_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations/sqlite");
static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations/postgres");

impl Db {
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        sqlx::any::install_default_drivers();
        let kind = if url.starts_with("postgres://") || url.starts_with("postgresql://") {
            DbKind::Postgres
        } else if url.starts_with("sqlite:") {
            DbKind::Sqlite
        } else {
            anyhow::bail!("unsupported database url (expected sqlite:// or postgres://)");
        };
        let url = if kind == DbKind::Sqlite { prepare_sqlite_url(url)? } else { prepare_postgres_url(url) };
        let mut opts = AnyPoolOptions::new().acquire_timeout(Duration::from_secs(30));
        opts = match kind {
            DbKind::Sqlite => opts.max_connections(8).after_connect(|conn, _meta| {
                Box::pin(async move {
                    sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await?;
                    sqlx::query("PRAGMA journal_mode = WAL").execute(&mut *conn).await?;
                    sqlx::query("PRAGMA busy_timeout = 10000").execute(&mut *conn).await?;
                    sqlx::query("PRAGMA synchronous = NORMAL").execute(&mut *conn).await?;
                    Ok(())
                })
            }),
            DbKind::Postgres => opts.max_connections(16),
        };
        let pool = opts.connect(&url).await.with_context(|| format!("connecting to {} database", kind.as_str()))?;
        Ok(Self { pool, kind })
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        match self.kind {
            DbKind::Sqlite => SQLITE_MIGRATOR.run(&self.pool).await?,
            DbKind::Postgres => POSTGRES_MIGRATOR.run(&self.pool).await?,
        }
        Ok(())
    }

    /// Case-insensitive LIKE operator for the backend.
    pub fn ilike(&self) -> &'static str {
        match self.kind {
            DbKind::Sqlite => "LIKE",
            DbKind::Postgres => "ILIKE",
        }
    }
}

/// sqlx 0.8's Any driver encodes a NULL `f64` as a Postgres `float4`
/// parameter (the Real/Double mapping is swapped). With the statement cache
/// on, a statement first prepared with a NULL then rejects real `float8`
/// values ("incorrect binary data format"). Disabling the cache makes each
/// execution prepare with the types actually bound.
fn prepare_postgres_url(url: &str) -> String {
    if url.contains("statement-cache-capacity") {
        return url.to_string();
    }
    let sep = if url.contains('?') { '&' } else { '?' };
    format!("{url}{sep}statement-cache-capacity=0")
}

fn prepare_sqlite_url(url: &str) -> anyhow::Result<String> {
    // Ensure the parent directory exists and the file is created on demand.
    let path_part = url.trim_start_matches("sqlite://").trim_start_matches("sqlite:");
    let path_only = path_part.split('?').next().unwrap_or("");
    if !path_only.is_empty() && path_only != ":memory:" {
        if let Some(parent) = std::path::Path::new(path_only).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
    }
    let mut url = url.to_string();
    if !url.contains("mode=") {
        url.push_str(if url.contains('?') { "&mode=rwc" } else { "?mode=rwc" });
    }
    // Validate early so a bad URL fails with a clear message.
    let _ = sqlx::sqlite::SqliteConnectOptions::from_str(&url).context("invalid sqlite url")?;
    Ok(url)
}

/// Current UTC time as fixed-width RFC 3339 with microseconds (sortable as text).
pub fn now() -> String {
    format_ts(chrono::Utc::now())
}

pub fn format_ts(t: chrono::DateTime<chrono::Utc>) -> String {
    t.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Validates a client-supplied id (must be a UUID) or generates one.
pub fn id_or_new(id: Option<&str>) -> Result<String, crate::error::AppError> {
    match id {
        Some(s) => uuid::Uuid::parse_str(s).map(|u| u.to_string()).map_err(|_| crate::error::AppError::bad_request("id must be a UUID")),
        None => Ok(new_id()),
    }
}

/// Convenience accessors for `AnyRow` that keep call sites short.
pub trait RowExt {
    fn s(&self, col: &str) -> String;
    fn os(&self, col: &str) -> Option<String>;
    fn i(&self, col: &str) -> i64;
    fn oi(&self, col: &str) -> Option<i64>;
    fn of(&self, col: &str) -> Option<f64>;
    fn b(&self, col: &str) -> bool;
    fn json<T: serde::de::DeserializeOwned + Default>(&self, col: &str) -> T;
}

impl RowExt for AnyRow {
    fn s(&self, col: &str) -> String {
        self.try_get::<String, _>(col).unwrap_or_default()
    }
    fn os(&self, col: &str) -> Option<String> {
        self.try_get::<Option<String>, _>(col).ok().flatten()
    }
    fn i(&self, col: &str) -> i64 {
        self.oi(col).unwrap_or(0)
    }
    fn oi(&self, col: &str) -> Option<i64> {
        match self.try_get::<Option<i64>, _>(col) {
            Ok(v) => v,
            Err(_) => self.try_get::<Option<i32>, _>(col).ok().flatten().map(i64::from),
        }
    }
    fn of(&self, col: &str) -> Option<f64> {
        match self.try_get::<Option<f64>, _>(col) {
            Ok(v) => v,
            Err(_) => self.oi(col).map(|v| v as f64),
        }
    }
    fn b(&self, col: &str) -> bool {
        self.i(col) != 0
    }
    fn json<T: serde::de::DeserializeOwned + Default>(&self, col: &str) -> T {
        self.os(col).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Applies only the migrations before `0004` (trial recipes), as an
    /// installation of an earlier release has them.
    async fn migrate_before_trial(db: &Db, dir: &std::path::Path) {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations").join(db.kind.as_str());
        for entry in std::fs::read_dir(&src).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if name.as_str() < "0004" {
                std::fs::copy(&path, dir.join(&name)).unwrap();
            }
        }
        sqlx::migrate::Migrator::new(dir).await.unwrap().run(&db.pool).await.unwrap();
    }

    /// Recipes and meal plan entries saved before the upgrade stay in the
    /// cookbook, untried and unanswered.
    async fn trial_migration_keeps_existing_data(db: Db) {
        let old = tempfile::tempdir().unwrap();
        migrate_before_trial(&db, old.path()).await;
        let ts = now();
        sqlx::query("INSERT INTO households (id, name, created_at) VALUES ('h1', 'Home', $1)").bind(&ts).execute(&db.pool).await.unwrap();
        sqlx::query(
            "INSERT INTO recipes (id, household_id, slug, name, created_at, updated_at) VALUES ('r1', 'h1', 'stew', 'Stew', $1, $1)",
        )
        .bind(&ts)
        .execute(&db.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO meal_plan (id, household_id, date, meal, recipe_id, created_at, updated_at) \
             VALUES ('m1', 'h1', '2026-11-01', 'dinner', 'r1', $1, $1)",
        )
        .bind(&ts)
        .execute(&db.pool)
        .await
        .unwrap();

        db.migrate().await.unwrap();

        let r = sqlx::query("SELECT status, tried_count, last_tried_at FROM recipes WHERE id = 'r1'").fetch_one(&db.pool).await.unwrap();
        assert_eq!(r.s("status"), "cookbook");
        assert_eq!(r.i("tried_count"), 0);
        assert_eq!(r.os("last_tried_at"), None);
        let m = sqlx::query("SELECT trial_outcome, trial_resolved_at FROM meal_plan WHERE id = 'm1'").fetch_one(&db.pool).await.unwrap();
        assert_eq!(m.os("trial_outcome"), None);
        assert_eq!(m.os("trial_resolved_at"), None);
        // rows written by code that does not know the columns get the same defaults
        sqlx::query(
            "INSERT INTO recipes (id, household_id, slug, name, created_at, updated_at) VALUES ('r2', 'h1', 'soup', 'Soup', $1, $1)",
        )
        .bind(&ts)
        .execute(&db.pool)
        .await
        .unwrap();
        let r = sqlx::query("SELECT status, tried_count FROM recipes WHERE id = 'r2'").fetch_one(&db.pool).await.unwrap();
        assert_eq!((r.s("status").as_str(), r.i("tried_count")), ("cookbook", 0));
    }

    #[tokio::test]
    async fn trial_migration_sqlite() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::connect(&format!("sqlite://{}", dir.path().join("old.db").display())).await.unwrap();
        trial_migration_keeps_existing_data(db).await;
    }

    #[tokio::test]
    async fn trial_migration_postgres() {
        let Ok(url) = std::env::var("LARDER_TEST_POSTGRES_URL") else {
            eprintln!("skipping: LARDER_TEST_POSTGRES_URL not set");
            return;
        };
        let db = Db::connect(&url).await.unwrap();
        sqlx::query("DROP SCHEMA public CASCADE").execute(&db.pool).await.unwrap();
        sqlx::query("CREATE SCHEMA public").execute(&db.pool).await.unwrap();
        trial_migration_keeps_existing_data(db).await;
    }
}
