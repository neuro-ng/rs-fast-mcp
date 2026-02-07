use crate::error::FastMCPError;
use crate::mcp::types::JsonRpcRequest;
use crate::server::auth::{AuthContext, AuthProvider};
use async_trait::async_trait;
use reqwest;
use serde::Deserialize;

/// Google Authentication Provider using OAuth2 Token Introspection (tokeninfo).
pub struct GoogleProvider {
    client_id: String,
    validation_url: String,
    // We might add support for validating against specific hosted domains (hd) later
}

#[derive(Deserialize, Debug)]
struct GoogleTokenInfo {
    // Defines standard fields returned by https://oauth2.googleapis.com/tokeninfo
    // The subject (user ID)
    sub: String,
    // The audience (client ID)
    aud: String,
    // Space-separated scopes
    scope: Option<String>,
    #[allow(dead_code)]
    email: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    email_verified: Option<String>,
}

impl GoogleProvider {
    pub fn new(client_id: &str) -> Self {
        Self {
            client_id: client_id.to_string(),
            validation_url: "https://oauth2.googleapis.com/tokeninfo".to_string(),
        }
    }

    pub fn with_validation_url(mut self, url: &str) -> Self {
        self.validation_url = url.to_string();
        self
    }
}

use url::Url;

// ...

#[async_trait]
impl AuthProvider for GoogleProvider {
    async fn verify(&self, request: &JsonRpcRequest) -> Result<AuthContext, FastMCPError> {
        let token = request
            .transport_metadata
            .as_ref()
            .and_then(|metadata| {
                metadata
                    .get("authorization")
                    .or_else(|| metadata.get("Authorization"))
            })
            .and_then(|h| h.strip_prefix("Bearer "))
            .or_else(|| {
                request
                    .params
                    .as_ref()
                    .and_then(|p| p.get("token"))
                    .and_then(|v| v.as_str())
            })
            .ok_or_else(|| FastMCPError::InvalidRequest("Missing token parameter".to_string()))?;

        // 1. Validate token with Google
        let url = Url::parse_with_params(&self.validation_url, &[("access_token", token)])
            .map_err(|e| FastMCPError::new(format!("Failed to build URL: {}", e)))?;

        let client = reqwest::Client::new();
        let resp = client.get(url).send().await.map_err(|e| {
            FastMCPError::new(format!("Google token validation request failed: {}", e))
        })?;

        if !resp.status().is_success() {
            return Err(FastMCPError::InvalidRequest(
                "Invalid Google Access Token".to_string(),
            ));
        }

        let info = resp
            .json::<GoogleTokenInfo>()
            .await
            .map_err(|e| FastMCPError::new(format!("Failed to parse Google token info: {}", e)))?;

        // 2. Verify Audience (Match Client ID)
        // Note: Google's 'aud' field in tokeninfo is the Client ID.
        if info.aud != self.client_id {
            return Err(FastMCPError::InvalidRequest(format!(
                "Token audience mismatch. Expected {}, got {}",
                self.client_id, info.aud
            )));
        }

        // 3. Return Context
        Ok(AuthContext {
            client_id: Some(self.client_id.clone()),
            user_id: Some(info.sub),
            scopes: info
                .scope
                .map(|s| {
                    s.split_whitespace()
                        .map(|x| x.to_string())
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default(),
        })
    }
}
