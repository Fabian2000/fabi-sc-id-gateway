//! User database operations.

use anyhow::Result;
use sqlx::SqlitePool;

/// Check if a user is an admin.
pub async fn is_admin(pool: &SqlitePool, username: &str) -> Result<bool> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM users WHERE username = ? AND is_admin = 1")
            .bind(username)
            .fetch_optional(pool)
            .await?;

    Ok(row.is_some())
}

/// Set admin users (adds them as admins, keeps existing non-admin users).
pub async fn set_admins(pool: &SqlitePool, usernames: &[&str]) -> Result<()> {
    // Reset all to non-admin
    sqlx::query("UPDATE users SET is_admin = 0")
        .execute(pool)
        .await?;

    // Set specified users as admin
    for username in usernames {
        sqlx::query(
            "INSERT INTO users (username, is_admin) VALUES (?, 1)
             ON CONFLICT(username) DO UPDATE SET is_admin = 1",
        )
        .bind(username)
        .execute(pool)
        .await?;
    }

    Ok(())
}
