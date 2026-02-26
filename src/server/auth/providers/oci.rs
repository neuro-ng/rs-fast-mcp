use crate::error::FastMCPError;
use crate::mcp::types::JsonRpcRequest;
use crate::server::auth::oidc::OIDCProvider;
use crate::server::auth::{AuthContext, AuthProvider};
use async_trait::async_trait;
use std::env;

/// OCI (Oracle Cloud Infrastructure) Authentication Provider.
///
/// Wraps [`OIDCProvider`] with OCI-specific configuration.
/// See <https://docs.oracle.com/en-us/iaas/Content/Identity/home.htm>
pub struct OCIProvider {
    inner: OIDCProvider,
    /// OCI-specific settings if needed later
    #[allow(dead_code)]
    base_url: String,
}

impl OCIProvider {
    /// Create a new OCIProvider.
    ///
    /// # Arguments
    /// * `issuer_url` - The OCI Identity Domain URL (e.g., `https://idcs-xxx.identity.oraclecloud.com`)
    /// * `client_id` - The OCI App Client ID
    /// * `base_url` - The base URL of the service (used for OCI-specific flows if needed)
    pub async fn new(
        issuer_url: &str,
        client_id: &str,
        base_url: &str,
    ) -> Result<Self, FastMCPError> {
        let issuer = issuer_url.trim_end_matches('/');

        let inner = OIDCProvider::new(issuer, client_id)
            .await
            .map_err(|e| FastMCPError::new(format!("Failed to initialize OCI provider: {}", e)))?;

        Ok(Self {
            inner,
            base_url: base_url.to_string(),
        })
    }

    /// Load configuration from environment variables.
    ///
    /// Expected variables:
    /// - `OXFASTMCP_SERVER_AUTH_OCI_ISSUER_URL` (or config URL base)
    /// - `OXFASTMCP_SERVER_AUTH_OCI_CLIENT_ID`
    /// - `OXFASTMCP_SERVER_AUTH_OCI_BASE_URL`
    pub async fn from_env() -> Result<Self, FastMCPError> {
        let issuer_url = env::var("OXFASTMCP_SERVER_AUTH_OCI_ISSUER_URL")
             .or_else(|_| env::var("OXFASTMCP_SERVER_AUTH_OCI_CONFIG_URL").map(|s| s.replace("/.well-known/openid-configuration", "")))
             .map_err(|_| FastMCPError::new("Missing OXFASTMCP_SERVER_AUTH_OCI_ISSUER_URL or OXFASTMCP_SERVER_AUTH_OCI_CONFIG_URL".to_string()))?;

        let client_id = env::var("OXFASTMCP_SERVER_AUTH_OCI_CLIENT_ID").map_err(|_| {
            FastMCPError::new("Missing OXFASTMCP_SERVER_AUTH_OCI_CLIENT_ID".to_string())
        })?;

        let base_url = env::var("OXFASTMCP_SERVER_AUTH_OCI_BASE_URL").map_err(|_| {
            FastMCPError::new("Missing OXFASTMCP_SERVER_AUTH_OCI_BASE_URL".to_string())
        })?;

        Self::new(&issuer_url, &client_id, &base_url).await
    }
}

#[async_trait]
impl AuthProvider for OCIProvider {
    async fn verify(&self, request: &JsonRpcRequest) -> Result<AuthContext, FastMCPError> {
        self.inner.verify(request).await
    }
}
