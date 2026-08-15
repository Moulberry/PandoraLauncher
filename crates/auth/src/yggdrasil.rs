use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Yggdrasil authentication client for authlib-injector compatible servers (e.g. Ely.by).
pub struct YggdrasilClient {
    client: reqwest::Client,
}

#[derive(thiserror::Error, Debug)]
pub enum YggdrasilError {
    #[error("Connection error: {0}")]
    ConnectionError(#[from] reqwest::Error),
    #[error("Serialization error")]
    SerializationError,
    #[error("Non-OK HTTP status: {0}")]
    NonOkHttpStatus(reqwest::StatusCode),
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
}

impl YggdrasilError {
    pub fn is_connection_error(&self) -> bool {
        matches!(self, Self::ConnectionError(_))
    }
}

// --- Request/Response types ---

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct YggdrasilAuthenticateRequest<'a> {
    agent: YggdrasilAgent,
    username: &'a str,
    password: &'a str,
    client_token: &'a str,
    request_user: bool,
}

#[derive(Serialize)]
struct YggdrasilAgent {
    name: &'static str,
    version: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YggdrasilAuthenticateResponse {
    pub access_token: Arc<str>,
    pub client_token: Arc<str>,
    pub selected_profile: Option<YggdrasilProfile>,
    pub available_profiles: Option<Vec<YggdrasilProfile>>,
}

#[derive(Deserialize, Clone)]
pub struct YggdrasilProfile {
    pub id: Arc<str>,
    pub name: Arc<str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct YggdrasilRefreshRequest<'a> {
    access_token: &'a str,
    client_token: &'a str,
    request_user: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YggdrasilRefreshResponse {
    pub access_token: Arc<str>,
    pub client_token: Arc<str>,
    pub selected_profile: Option<YggdrasilProfile>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct YggdrasilValidateRequest<'a> {
    access_token: &'a str,
    client_token: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct YggdrasilErrorResponse {
    #[serde(default)]
    error_message: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

impl YggdrasilClient {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Authenticate with a Yggdrasil-compatible server.
    /// `server_url` should be the base URL of the auth server, e.g. `https://authserver.ely.by`.
    pub async fn authenticate(
        &self,
        server_url: &str,
        username: &str,
        password: &str,
        client_token: &str,
    ) -> Result<YggdrasilAuthenticateResponse, YggdrasilError> {
        let url = format!("{}/authserver/authenticate", server_url.trim_end_matches('/'));

        let request = YggdrasilAuthenticateRequest {
            agent: YggdrasilAgent {
                name: "Minecraft",
                version: 1,
            },
            username,
            password,
            client_token,
            request_user: true,
        };

        let response = self.client.post(&url).json(&request).send().await?;

        if response.status() != reqwest::StatusCode::OK {
            let status = response.status();
            if let Ok(bytes) = response.bytes().await {
                if let Ok(err) = serde_json::from_slice::<YggdrasilErrorResponse>(&bytes) {
                    let msg = err.error_message.or(err.error).unwrap_or_else(|| format!("HTTP {}", status));
                    return Err(YggdrasilError::AuthenticationFailed(msg));
                }
            }
            return Err(YggdrasilError::NonOkHttpStatus(status));
        }

        let bytes = response.bytes().await?;
        serde_json::from_slice(&bytes).map_err(|_| YggdrasilError::SerializationError)
    }

    /// Refresh an existing Yggdrasil access token.
    pub async fn refresh(
        &self,
        server_url: &str,
        access_token: &str,
        client_token: &str,
    ) -> Result<YggdrasilRefreshResponse, YggdrasilError> {
        let url = format!("{}/authserver/refresh", server_url.trim_end_matches('/'));

        let request = YggdrasilRefreshRequest {
            access_token,
            client_token,
            request_user: true,
        };

        let response = self.client.post(&url).json(&request).send().await?;

        if response.status() != reqwest::StatusCode::OK {
            let status = response.status();
            if let Ok(bytes) = response.bytes().await {
                if let Ok(err) = serde_json::from_slice::<YggdrasilErrorResponse>(&bytes) {
                    let msg = err.error_message.or(err.error).unwrap_or_else(|| format!("HTTP {}", status));
                    return Err(YggdrasilError::AuthenticationFailed(msg));
                }
            }
            return Err(YggdrasilError::NonOkHttpStatus(status));
        }

        let bytes = response.bytes().await?;
        serde_json::from_slice(&bytes).map_err(|_| YggdrasilError::SerializationError)
    }

    /// Validate an existing Yggdrasil access token. Returns true if valid, false otherwise.
    pub async fn validate(
        &self,
        server_url: &str,
        access_token: &str,
        client_token: &str,
    ) -> Result<bool, YggdrasilError> {
        let url = format!("{}/authserver/validate", server_url.trim_end_matches('/'));

        let request = YggdrasilValidateRequest {
            access_token,
            client_token,
        };

        let response = self.client.post(&url).json(&request).send().await?;

        // 204 No Content = valid, 403 = invalid
        Ok(response.status() == reqwest::StatusCode::NO_CONTENT)
    }

    /// Parse a UUID from a Yggdrasil profile ID (which lacks hyphens).
    pub fn parse_profile_uuid(id: &str) -> Option<Uuid> {
        // Yggdrasil returns UUIDs without hyphens
        Uuid::try_parse(id).ok()
            .or_else(|| {
                if id.len() == 32 {
                    let with_hyphens = format!(
                        "{}-{}-{}-{}-{}",
                        &id[0..8], &id[8..12], &id[12..16], &id[16..20], &id[20..32]
                    );
                    Uuid::try_parse(&with_hyphens).ok()
                } else {
                    None
                }
            })
    }
}
