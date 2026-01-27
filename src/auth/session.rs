//! Session management for the admin UI.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

/// Interval between token validations at ID service.
const VALIDATION_INTERVAL_MINUTES: i64 = 2;

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub last_validated: DateTime<Utc>,
}

impl Session {
    /// Returns true if the token should be re-validated at the ID service.
    pub fn needs_validation(&self) -> bool {
        Utc::now() - self.last_validated > Duration::minutes(VALIDATION_INTERVAL_MINUTES)
    }
}

pub async fn create_session(
    pool: &SqlitePool,
    user_id: &str,
    username: &str,
    token: &str,
) -> Result<Session> {
    let now = Utc::now();
    let session = Session {
        id: Uuid::now_v7().to_string(),
        user_id: user_id.to_string(),
        username: username.to_string(),
        token: token.to_string(),
        expires_at: now + Duration::hours(24),
        last_validated: now,
    };

    sqlx::query(
        "INSERT INTO sessions (id, user_id, username, token, expires_at, last_validated) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&session.id)
    .bind(&session.user_id)
    .bind(&session.username)
    .bind(&session.token)
    .bind(session.expires_at.to_rfc3339())
    .bind(session.last_validated.to_rfc3339())
    .execute(pool)
    .await?;

    Ok(session)
}

pub async fn get_session(pool: &SqlitePool, session_id: &str) -> Result<Option<Session>> {
    let row: Option<(String, String, String, String, String, String)> = sqlx::query_as(
        "SELECT id, user_id, username, token, expires_at, last_validated FROM sessions WHERE id = ? AND expires_at > datetime('now')",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(id, user_id, username, token, expires_at, last_validated)| {
        let parse_dt = |s: &str| {
            DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now())
        };
        Session {
            id,
            user_id,
            username,
            token,
            expires_at: parse_dt(&expires_at),
            last_validated: parse_dt(&last_validated),
        }
    }))
}

pub async fn delete_session(pool: &SqlitePool, session_id: &str) -> Result<()> {
    sqlx::query("DELETE FROM sessions WHERE id = ?")
        .bind(session_id)
        .execute(pool)
        .await?;

    Ok(())
}

/// Update last_validated timestamp after successful token validation.
pub async fn update_last_validated(pool: &SqlitePool, session_id: &str) -> Result<()> {
    sqlx::query("UPDATE sessions SET last_validated = ? WHERE id = ?")
        .bind(Utc::now().to_rfc3339())
        .bind(session_id)
        .execute(pool)
        .await?;

    Ok(())
}
