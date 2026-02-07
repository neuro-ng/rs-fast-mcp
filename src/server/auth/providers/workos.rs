use crate::error::FastMCPError;
use crate::mcp::types::JsonRpcRequest;
use crate::server::auth::oidc::OIDCProvider;
use crate::server::auth::{AuthContext, AuthProvider};
use async_trait::async_trait;
use std::env;

/// WorkOS Authentication Provider
///
/// Implements authentication using WorkOS AuthKit (User Management) via OIDC/OAuth2.
/// See https://workos.com/docs/user-management
pub struct WorkOSProvider {
    inner: OIDCProvider,
    /// WorkOS specific domain e.g. https://api.workos.com or a custom domain
    #[allow(dead_code)]
    domain: String,
}

impl WorkOSProvider {
    /// Create a new WorkOSProvider.
    ///
    /// # Arguments
    /// * `client_id` - The WorkOS Client ID
    /// * `authkit_domain` - The WorkOS AuthKit Domain (e.g., https://<your-domain>.authkit.app)
    pub async fn new(client_id: &str, authkit_domain: &str) -> Result<Self, FastMCPError> {
        // AuthKit supports OIDC discovery at /.well-known/openid-configuration
        // The issuer URL is effectively the authkit_domain.
        let issuer_url = authkit_domain.trim_end_matches('/');

        let inner = OIDCProvider::new(issuer_url, client_id)
            .await
            .map_err(|e| {
                FastMCPError::new(format!("Failed to initialize WorkOS provider: {}", e))
            })?;

        Ok(Self {
            inner,
            domain: issuer_url.to_string(),
        })
    }

    /// Load configuration from environment variables.
    ///
    /// Expected variables:
    /// - `OXFASTMCP_SERVER_AUTH_WORKOS_CLIENT_ID`
    /// - `OXFASTMCP_SERVER_AUTH_WORKOS_AUTHKIT_DOMAIN`
    pub async fn from_env() -> Result<Self, FastMCPError> {
        let client_id = env::var("OXFASTMCP_SERVER_AUTH_WORKOS_CLIENT_ID").map_err(|_| {
            FastMCPError::new("Missing OXFASTMCP_SERVER_AUTH_WORKOS_CLIENT_ID".to_string())
        })?;

        let authkit_domain =
            env::var("OXFASTMCP_SERVER_AUTH_WORKOS_AUTHKIT_DOMAIN").map_err(|_| {
                FastMCPError::new("Missing OXFASTMCP_SERVER_AUTH_WORKOS_AUTHKIT_DOMAIN".to_string())
            })?;

        Self::new(&client_id, &authkit_domain).await
    }
}

#[async_trait]
impl AuthProvider for WorkOSProvider {
    async fn verify(&self, request: &JsonRpcRequest) -> Result<AuthContext, FastMCPError> {
        self.inner.verify(request).await
    }
}
