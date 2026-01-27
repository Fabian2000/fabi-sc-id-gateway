//! Database migrations.

use anyhow::Result;
use sqlx::SqlitePool;
use tracing::info;

/// Run all database migrations.
pub async fn run(pool: &SqlitePool) -> Result<()> {
    info!("Running database migrations...");

    // Create migrations table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS _migrations (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Run migrations in order
    run_migration(pool, "001_initial", include_str!("../../migrations/001_initial.sql")).await?;
    run_migration(pool, "002_users", include_str!("../../migrations/002_users.sql")).await?;
    run_migration(pool, "003_allowed_users", include_str!("../../migrations/003_allowed_users.sql")).await?;
    run_migration(pool, "004_host_routing", include_str!("../../migrations/004_host_routing.sql")).await?;
    run_migration(pool, "005_session_validation", include_str!("../../migrations/005_session_validation.sql")).await?;

    info!("Database migrations complete");
    Ok(())
}

async fn run_migration(pool: &SqlitePool, name: &str, sql: &str) -> Result<()> {
    // Check if already applied
    let exists: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM _migrations WHERE name = ?")
        .bind(name)
        .fetch_optional(pool)
        .await?;

    if exists.is_some() {
        return Ok(());
    }

    info!("Applying migration: {}", name);

    // Execute migration
    sqlx::query(sql).execute(pool).await?;

    // Record migration
    sqlx::query("INSERT INTO _migrations (name) VALUES (?)")
        .bind(name)
        .execute(pool)
        .await?;

    Ok(())
}
