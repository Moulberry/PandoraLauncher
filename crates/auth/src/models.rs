use std::{collections::HashMap, sync::Arc};

use chrono::{DateTime, Utc};
use oauth2::{CsrfToken, PkceCodeVerifier};
use serde::{Deserialize, Serialize};
use url::Url;

pub struct MinecraftAccessToken(pub(crate) Arc<str>);

impl MinecraftAccessToken {
    pub fn secret(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize, Serialize)]
pub struct TokenWithExpiry {
    pub token: Arc<str>,
    pub expiry: DateTime<Utc>,
}

#[derive(Deserialize, Serialize)]
pub struct XstsToken {
    pub token: Arc<str>,
    pub expiry: DateTime<Utc>,
    pub userhash: Arc<str>,
}

pub struct PendingAuthorization {
    pub url: Url,
    pub csrf_token: CsrfToken,
    pub pkce_verifier: PkceCodeVerifier,
}

pub struct FinishedAuthorization {
    pub pending: PendingAuthorization,
    pub code: String,
}

pub struct MsaTokens {
    pub access: TokenWithExpiry,
    pub refresh: Option<Arc<str>>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct XboxLiveAuthenticateRequest<'a> {
    pub properties: XboxLiveAuthenticateRequestProperties<'a>,
    pub relying_party: &'a str,
    pub token_type: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct XboxLiveAuthenticateRequestProperties<'a> {
    pub auth_method: &'a str,
    pub site_name: &'a str,
    pub rps_ticket: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct XboxLiveAuthenticateResponse {
    pub issue_instant: DateTime<Utc>,
    pub not_after: DateTime<Utc>,
    pub token: Arc<str>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct XboxLiveSecurityTokenRequest<'a> {
    pub properties: XboxLiveSecurityTokenRequestProperties<'a>,
    pub relying_party: &'a str,
    pub token_type: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct XboxLiveSecurityTokenRequestProperties<'a> {
    pub sandbox_id: &'a str,
    pub user_tokens: &'a [&'a str],
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct XboxLiveSecurityTokenResponse {
    pub issue_instant: DateTime<Utc>,
    pub not_after: DateTime<Utc>,
    pub token: Arc<str>,
    pub display_claims: XboxUserIdentityDisplayClaims,
}

#[derive(Deserialize)]
pub struct XboxUserIdentityDisplayClaims {
    pub xui: Vec<HashMap<String, String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftLoginWithXboxRequest<'a> {
    pub identity_token: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MinecraftLoginWithXboxResponse {
    pub username: Arc<str>,
    pub access_token: Arc<str>,
    pub expires_in: usize,
}
