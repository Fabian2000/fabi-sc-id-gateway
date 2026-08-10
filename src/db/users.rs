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

/// Replace the set of admin users.
///
/// The demotion and the grants run in one transaction, so a failure part-way
/// through can never leave the gateway with no administrator. An empty list is
/// rejected for the same reason.
pub async fn set_admins(pool: &SqlitePool, usernames: &[&str]) -> Result<()> {
    if usernames.iter().all(|u| u.trim().is_empty()) {
        anyhow::bail!("refusing to set an empty admin list");
    }

    let mut tx = pool.begin().await?;

    sqlx::query("UPDATE users SET is_admin = 0")
        .execute(&mut *tx)
        .await?;

    for username in usernames {
        sqlx::query(
            "INSERT INTO users (username, is_admin) VALUES (?, 1)
             ON CONFLICT(username) DO UPDATE SET is_admin = 1",
        )
        .bind(username)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(())
}
