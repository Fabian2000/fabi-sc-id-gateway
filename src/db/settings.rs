//! Settings and ID configuration database operations.

use crate::config::IdConfig;
use anyhow::Result;
use sqlx::SqlitePool;

/// Get a setting value.
pub async fn get_setting(pool: &SqlitePool, key: &str) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await?;

    Ok(row.map(|(v,)| v))
}

/// Get Fabi-SC ID configuration.
pub async fn get_id_config(pool: &SqlitePool) -> Result<IdConfig> {
    let row: (String, Option<String>, Option<String>, Option<String>) =
        sqlx::query_as("SELECT server_url, app_id, api_key, admin_origin FROM id_config WHERE id = 1")
            .fetch_one(pool)
            .await?;

    Ok(IdConfig {
        server_url: row.0,
        app_id: row.1.unwrap_or_default(),
        api_key: row.2.unwrap_or_default(),
        admin_origin: row.3.unwrap_or_default(),
    })
}

/// Update Fabi-SC ID configuration.
pub async fn set_id_config(pool: &SqlitePool, config: &IdConfig) -> Result<()> {
    sqlx::query(
        "UPDATE id_config SET server_url = ?, app_id = ?, api_key = ?, admin_origin = ?, updated_at = CURRENT_TIMESTAMP WHERE id = 1",
    )
    .bind(&config.server_url)
    .bind(&config.app_id)
    .bind(&config.api_key)
    .bind(&config.admin_origin)
    .execute(pool)
    .await?;

    Ok(())
}

/// Check if ID configuration is complete.
pub async fn is_id_configured(pool: &SqlitePool) -> Result<bool> {
    let config = get_id_config(pool).await?;
    Ok(!config.app_id.is_empty() && !config.api_key.is_empty())
}
