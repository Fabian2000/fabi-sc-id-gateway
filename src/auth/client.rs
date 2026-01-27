use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::config::IdConfig;

pub struct IdClient {
    client: Client,
    config: IdConfig,
}

#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

impl<T> ApiResponse<T> {
    fn into_result(self) -> Result<T> {
        if self.success {
            self.data.ok_or_else(|| anyhow::anyhow!("No data in response"))
        } else {
            Err(anyhow::anyhow!(
                self.error.unwrap_or_else(|| "Unknown error".to_string())
            ))
        }
    }
}

#[derive(Debug, Serialize)]
struct ExchangeRequest {
    code: String,
}

#[derive(Debug, Deserialize)]
pub struct ExchangeResponse {
    pub token: String,
}

#[derive(Debug, Serialize)]
struct RevokeRequest {
    token: String,
}

#[derive(Debug, Deserialize)]
struct RevokeResponse {}

#[derive(Debug, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub username: Option<String>,
}

impl IdClient {
    pub fn new(config: IdConfig) -> Self {
        let client = Client::builder()
            .build()
            .expect("Failed to create HTTP client");

        Self { client, config }
    }

    /// Returns the admin origin with https:// prefix, trailing slash removed.
    pub fn admin_origin(&self) -> String {
        let raw = self.config.admin_origin.trim_end_matches('/');
        if raw.starts_with("https://") || raw.starts_with("http://") {
            raw.to_string()
        } else {
            format!("https://{}", raw)
        }
    }

    pub fn consent_url(&self, callback_url: &str) -> String {
        format!(
            "{}/consent/{}?redirect_uri={}",
            self.config.server_url,
            self.config.app_id,
            urlencoding::encode(callback_url)
        )
    }

    pub async fn exchange_code(&self, code: &str, origin: &str) -> Result<ExchangeResponse> {
        let url = format!("{}/api/v1/exchange", self.config.server_url);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Origin", origin)
            .json(&ExchangeRequest { code: code.to_string() })
            .send()
            .await?;

        resp.json::<ApiResponse<ExchangeResponse>>().await?.into_result()
    }

    pub async fn get_user(&self, token: &str, origin: &str) -> Result<UserInfo> {
        let url = format!("{}/api/v1/user", self.config.server_url);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("X-User-Token", token)
            .header("Origin", origin)
            .send()
            .await?;

        resp.json::<ApiResponse<UserInfo>>().await?.into_result()
    }

    pub async fn revoke_token(&self, token: &str, origin: &str) -> Result<()> {
        let url = format!("{}/api/v1/revoke", self.config.server_url);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Origin", origin)
            .json(&RevokeRequest { token: token.to_string() })
            .send()
            .await?;

        resp.json::<ApiResponse<RevokeResponse>>().await?.into_result()?;
        Ok(())
    }
}
