//! Database module for SQLite persistence.

mod migrations;
mod routes;
mod settings;
mod users;

use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;
use tracing::info;

pub use routes::*;
pub use settings::*;
pub use users::*;

pub type DbPool = SqlitePool;

/// Initialize the database connection and run migrations.
pub async fn init(path: &Path) -> Result<DbPool> {
    let db_exists = path.exists();

    let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", path.display()))?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    if !db_exists {
        info!("Creating new database at {:?}", path);
    }

    // Run migrations
    migrations::run(&pool).await?;

    Ok(pool)
}

/// Check if this is the first run (no configuration exists).
pub async fn is_first_run(pool: &DbPool) -> Result<bool> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM settings")
        .fetch_one(pool)
        .await?;

    Ok(count.0 == 0)
}
