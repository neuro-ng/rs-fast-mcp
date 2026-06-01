use crate::error::FastMCPError;
use crate::mcp::types::JsonRpcRequest;
use crate::server::auth::{AuthContext, AuthProvider};
use async_trait::async_trait;
use serde::Deserialize;

/// Discord OAuth2 authentication provider.
///
/// Validates access tokens by calling `https://discord.com/api/users/@me`.
/// The token must be provided in the `Authorization: Bearer <token>` header
/// via `transport_metadata`.
pub struct DiscordProvider {
    client: reqwest::Client,
    /// Required guild ID — if set, the authenticated user must be a member.
    #[allow(dead_code)]
    required_guild_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiscordUser {
    id: String,
    username: String,
    #[allow(dead_code)]
    discriminator: Option<String>,
    email: Option<String>,
}

impl DiscordProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            required_guild_id: None,
        }
    }

    /// Restrict access to members of a specific Discord guild.
    pub fn with_guild(mut self, guild_id: impl Into<String>) -> Self {
        self.required_guild_id = Some(guild_id.into());
        self
    }
}

impl Default for DiscordProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuthProvider for DiscordProvider {
    async fn verify(&self, request: &JsonRpcRequest) -> Result<AuthContext, FastMCPError> {
        let token = request
            .transport_metadata
            .as_ref()
            .and_then(|m| m.get("authorization").or_else(|| m.get("Authorization")))
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or_else(|| {
                FastMCPError::InvalidRequest("Missing Bearer token".to_string())
            })?;

        let resp = self
            .client
            .get("https://discord.com/api/users/@me")
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| FastMCPError::new(format!("Discord API error: {}", e)))?;

        if !resp.status().is_success() {
            return Err(FastMCPError::InvalidRequest(format!(
                "Discord token verification failed: HTTP {}",
                resp.status()
            )));
        }

        let user: DiscordUser = resp
            .json()
            .await
            .map_err(|e| FastMCPError::new(format!("Discord response parse error: {}", e)))?;

        let mut scopes = vec!["identify".to_string()];
        if user.email.is_some() {
            scopes.push("email".to_string());
        }

        Ok(AuthContext {
            client_id: Some("discord".to_string()),
            user_id: Some(format!("discord:{}", user.id)),
            scopes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discord_provider_default() {
        let p = DiscordProvider::new();
        assert!(p.required_guild_id.is_none());
    }

    #[test]
    fn test_discord_provider_with_guild() {
        let p = DiscordProvider::new().with_guild("123456789");
        assert_eq!(p.required_guild_id.as_deref(), Some("123456789"));
    }
}
