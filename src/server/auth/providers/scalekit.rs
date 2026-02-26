use crate::error::FastMCPError;
use crate::mcp::types::JsonRpcRequest;
use crate::server::auth::oidc::OIDCProvider;
use crate::server::auth::{AuthContext, AuthProvider};
use async_trait::async_trait;
use std::env;

/// Scalekit Authentication Provider.
///
/// Implements authentication using Scalekit (B2B Auth) via OIDC/OAuth2.1.
/// See <https://scalekit.com/docs>
pub struct ScalekitProvider {
    inner: OIDCProvider,
    /// Scalekit Environment URL (e.g., `https://your-env.scalekit.com`).
    #[allow(dead_code)]
    environment_url: String,
    /// Resource ID associated with the MCP server
    #[allow(dead_code)]
    resource_id: String,
}

impl ScalekitProvider {
    /// Create a new ScalekitProvider.
    ///
    /// # Arguments
    /// * `environment_url` - The Scalekit Environment URL
    /// * `client_id` - The Client ID (often the resource ID in some configs, or app client id)
    /// * `resource_id` - The Resource ID
    pub async fn new(
        environment_url: &str,
        client_id: &str,
        resource_id: &str,
    ) -> Result<Self, FastMCPError> {
        let env_url = environment_url.trim_end_matches('/');

        // Scalekit OIDC discovery is typically at the environment root or specific path.
        // Assuming standard OIDC discovery for now.
        let inner = OIDCProvider::new(env_url, client_id).await.map_err(|e| {
            FastMCPError::new(format!("Failed to initialize Scalekit provider: {}", e))
        })?;

        Ok(Self {
            inner,
            environment_url: env_url.to_string(),
            resource_id: resource_id.to_string(),
        })
    }

    /// Load configuration from environment variables.
    ///
    /// Expected variables:
    /// - `OXFASTMCP_SERVER_AUTH_SCALEKITPROVIDER_ENVIRONMENT_URL`
    /// - `OXFASTMCP_SERVER_AUTH_SCALEKITPROVIDER_RESOURCE_ID`
    /// - `OXFASTMCP_SERVER_AUTH_SCALEKITPROVIDER_CLIENT_ID` (Added for completeness, though OCaml code doesn't explicitly require it for create, it's needed for OIDC validation)
    pub async fn from_env() -> Result<Self, FastMCPError> {
        let environment_url = env::var("OXFASTMCP_SERVER_AUTH_SCALEKITPROVIDER_ENVIRONMENT_URL")
            .map_err(|_| {
                FastMCPError::new(
                    "Missing OXFASTMCP_SERVER_AUTH_SCALEKITPROVIDER_ENVIRONMENT_URL".to_string(),
                )
            })?;

        let resource_id =
            env::var("OXFASTMCP_SERVER_AUTH_SCALEKITPROVIDER_RESOURCE_ID").map_err(|_| {
                FastMCPError::new(
                    "Missing OXFASTMCP_SERVER_AUTH_SCALEKITPROVIDER_RESOURCE_ID".to_string(),
                )
            })?;

        // We need a client_id/audience for validation. If not provided, we might default to resource_id
        // but strict OIDC checking usually requires a specific client ID.
        // OCaml implementation seems to ignore client_id in `create` but we need it for `OIDCProvider`.
        // Let's assume passed via env or use resource_id as audience if strictly resource server?
        // Reuse RESOURCE_ID as client_id/audience if CLIENT_ID is missing, common in some B2B patterns?
        let client_id = env::var("OXFASTMCP_SERVER_AUTH_SCALEKITPROVIDER_CLIENT_ID")
            .unwrap_or_else(|_| resource_id.clone());

        Self::new(&environment_url, &client_id, &resource_id).await
    }
}

#[async_trait]
impl AuthProvider for ScalekitProvider {
    async fn verify(&self, request: &JsonRpcRequest) -> Result<AuthContext, FastMCPError> {
        self.inner.verify(request).await
    }
}
