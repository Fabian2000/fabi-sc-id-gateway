//! Configuration types and utilities.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Fabi-SC ID configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdConfig {
    pub server_url: String,
    pub app_id: String,
    pub api_key: String,
    pub admin_origin: String,
}

/// A proxy route configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub id: Uuid,
    pub name: String,
    pub host: String,
    pub upstream_url: String,
    pub requires_auth: bool,
    pub enabled: bool,
    pub allowed_users: Vec<String>,
}
