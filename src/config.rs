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

impl Route {
    /// Whether `username` may access this route.
    ///
    /// An empty `allowed_users` list deliberately means "any authenticated user":
    /// the whitelist narrows access, it does not grant it. Access is still gated
    /// by `requires_auth`, and an anonymous or nameless user never reaches here.
    pub fn allows_user(&self, username: &str) -> bool {
        self.allowed_users.is_empty() || self.allowed_users.iter().any(|u| u == username)
    }
}
