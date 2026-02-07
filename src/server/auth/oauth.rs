use crate::error::FastMCPError;
use crate::server::auth::{AuthProvider, AuthContext};
use crate::mcp::types::JsonRpcRequest;
use async_trait::async_trait;
use reqwest;

/// Generic OAuth2 Provider that can validate tokens.
pub struct OAuthProvider {
    // Storing BasicClient directly is hard due to typestates.
    // For this simple implementation, we only use introspection_url manually via reqwest,
    // so we don't technically *need* BasicClient for introspection if we aren't using its helpers.
    // But if we want to store it for future use...
    // Let's implement introspection purely with reqwest for now to avoid oauth2 crate typestate hell 
    // until we need full flow (which simpler provider doesn't do anyway).
    //
    // Actually, BasicClient is useful for generating URLs, but here we are VALIDATING tokens.
    // So we just need the introspection URL.
    introspection_url: Option<String>,
}

impl OAuthProvider {
    pub fn new(
        _client_id: &str, // Unused for basic introspection?
        _client_secret: Option<&str>,
        _auth_url: &str,
        _token_url: &str,
        introspection_url: Option<&str>,
    ) -> Result<Self, FastMCPError> {
        // We validate URLs just in case
        if let Some(url) = introspection_url {
             let _ = reqwest::Url::parse(url).map_err(|e| FastMCPError::new(format!("Invalid Introspection URL: {}", e)))?;
        }

        Ok(Self {
            introspection_url: introspection_url.map(|s| s.to_string()),
        })
    }
}

#[async_trait]
impl AuthProvider for OAuthProvider {
    async fn verify(&self, request: &JsonRpcRequest) -> Result<AuthContext, FastMCPError> {
        // Extract token
        let token_str = request.params.as_ref()
            .and_then(|p| p.get("token"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| FastMCPError::InvalidRequest("Missing token".to_string()))?;

        if let Some(introspection_url) = &self.introspection_url {
             // Perform Introspection
             let client = reqwest::Client::new();
             let res = client.post(introspection_url)
                .form(&[("token", token_str)])
                .send()
                .await
                .map_err(|e| FastMCPError::new(format!("Introspection failed: {}", e)))?;
                
             if !res.status().is_success() {
                 return Err(FastMCPError::InvalidRequest("Token introspection failed".to_string()));
             }
             
             let body: serde_json::Value = res.json().await
                .map_err(|e| FastMCPError::new(format!("JSON error: {}", e)))?;
                
             // Explicit type hints
             let active = body.get("active").and_then(|v: &serde_json::Value| v.as_bool()) == Some(true);
             
             if active {
                 // Valid
                 Ok(AuthContext {
                     client_id: body.get("client_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                     user_id: body.get("sub").and_then(|v| v.as_str()).map(|s| s.to_string()),
                     scopes: body.get("scope").and_then(|v| v.as_str())
                        .map(|s| s.split_whitespace().map(|x| x.to_string()).collect())
                        .unwrap_or_default(),
                 })
             } else {
                 Err(FastMCPError::InvalidRequest("Token inactive".to_string()))
             }
        } else {
            // No introspection configured.
            Err(FastMCPError::new("No introspection URL configured"))
        }
    }
}
