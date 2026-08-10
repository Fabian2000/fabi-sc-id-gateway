//! Audit log database operations.

use sqlx::SqlitePool;
use tracing::warn;

/// Record an entry in the audit log.
///
/// Best-effort: a failure to record must never fail the request that triggered it,
/// so errors are logged and swallowed.
pub async fn audit(
    pool: &SqlitePool,
    action: &str,
    details: &str,
    user: Option<&str>,
    ip: Option<&str>,
) {
    let result = sqlx::query(
        "INSERT INTO audit_log (action, details, user_id, ip_address) VALUES (?, ?, ?, ?)",
    )
    .bind(action)
    .bind(details)
    .bind(user)
    .bind(ip)
    .execute(pool)
    .await;

    if let Err(e) = result {
        warn!("Failed to write audit log entry '{}': {}", action, e);
    }
}
