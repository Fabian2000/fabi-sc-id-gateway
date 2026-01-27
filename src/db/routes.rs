//! Route database operations.

use crate::config::Route;
use anyhow::Result;
use sqlx::SqlitePool;
use uuid::Uuid;

/// Get all routes.
pub async fn get_all_routes(pool: &SqlitePool) -> Result<Vec<Route>> {
    let rows = sqlx::query_as::<_, (String, String, String, String, bool, bool, String)>(
        "SELECT id, name, host, upstream_url, requires_auth, enabled, allowed_users FROM routes ORDER BY host",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, name, host, upstream_url, requires_auth, enabled, allowed_users)| Route {
            id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::now_v7()),
            name,
            host,
            upstream_url,
            requires_auth,
            enabled,
            allowed_users: serde_json::from_str(&allowed_users).unwrap_or_default(),
        })
        .collect())
}

/// Get enabled routes only.
pub async fn get_enabled_routes(pool: &SqlitePool) -> Result<Vec<Route>> {
    let rows = sqlx::query_as::<_, (String, String, String, String, bool, bool, String)>(
        "SELECT id, name, host, upstream_url, requires_auth, enabled, allowed_users FROM routes WHERE enabled = 1 ORDER BY host",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, name, host, upstream_url, requires_auth, enabled, allowed_users)| Route {
            id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::now_v7()),
            name,
            host,
            upstream_url,
            requires_auth,
            enabled,
            allowed_users: serde_json::from_str(&allowed_users).unwrap_or_default(),
        })
        .collect())
}

/// Create a new route.
pub async fn create_route(pool: &SqlitePool, route: &Route) -> Result<()> {
    let allowed_users_json = serde_json::to_string(&route.allowed_users).unwrap_or_else(|_| "[]".to_string());
    sqlx::query(
        "INSERT INTO routes (id, name, host, upstream_url, requires_auth, enabled, allowed_users) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(route.id.to_string())
    .bind(&route.name)
    .bind(&route.host)
    .bind(&route.upstream_url)
    .bind(route.requires_auth)
    .bind(route.enabled)
    .bind(&allowed_users_json)
    .execute(pool)
    .await?;

    Ok(())
}

/// Update an existing route.
pub async fn update_route(pool: &SqlitePool, route: &Route) -> Result<()> {
    let allowed_users_json = serde_json::to_string(&route.allowed_users).unwrap_or_else(|_| "[]".to_string());
    sqlx::query(
        "UPDATE routes SET name = ?, host = ?, upstream_url = ?, requires_auth = ?, enabled = ?, allowed_users = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(&route.name)
    .bind(&route.host)
    .bind(&route.upstream_url)
    .bind(route.requires_auth)
    .bind(route.enabled)
    .bind(&allowed_users_json)
    .bind(route.id.to_string())
    .execute(pool)
    .await?;

    Ok(())
}

/// Delete a route.
pub async fn delete_route(pool: &SqlitePool, id: Uuid) -> Result<()> {
    sqlx::query("DELETE FROM routes WHERE id = ?")
        .bind(id.to_string())
        .execute(pool)
        .await?;

    Ok(())
}
