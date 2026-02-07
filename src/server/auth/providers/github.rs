use crate::error::FastMCPError;
use crate::mcp::types::JsonRpcRequest;
use crate::server::auth::{AuthContext, AuthProvider};
use async_trait::async_trait;
use reqwest;
use serde::Deserialize;

/// GitHub Authentication Provider.
/// Validates tokens by fetching user profile from GitHub API.
pub struct GitHubProvider {
    user_agent: String,
}

#[derive(Deserialize, Debug)]
pub struct GitHubUser {
    pub id: u64,
    pub login: String,
    // email requires user:email scope, optional here
    pub email: Option<String>,
}

impl GitHubProvider {
    pub fn new() -> Self {
        Self {
            user_agent: "rs-fast-mcp-server".to_string(),
        }
    }
}

impl Default for GitHubProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GitHubProvider {
    pub fn with_user_agent(agent: &str) -> Self {
        Self {
            user_agent: agent.to_string(),
        }
    }
}

#[async_trait]
impl AuthProvider for GitHubProvider {
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

        let client = reqwest::Client::new();
        let resp = client
            .get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", &self.user_agent)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .map_err(|e| FastMCPError::new(format!("GitHub validation request failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(FastMCPError::InvalidRequest(
                "Invalid GitHub Token".to_string(),
            ));
        }

        let user: GitHubUser = resp
            .json()
            .await
            .map_err(|e| FastMCPError::new(format!("Failed to parse GitHub user info: {}", e)))?;

        Ok(AuthContext {
            // Using login as client_id for readability, id as user_id for stability
            client_id: Some(user.login.clone()),
            user_id: Some(user.id.to_string()),
            scopes: vec![], // GitHub API response doesn't strictly return scopes in body, usually in headers (X-OAuth-Scopes).
                            // For now, we return empty scopes unless we parse headers.
        })
    }
}
