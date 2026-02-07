use crate::error::FastMCPError;
use crate::mcp::types::JsonRpcRequest;
use crate::server::auth::oidc::OIDCProvider;
use crate::server::auth::{AuthContext, AuthProvider};
use async_trait::async_trait;
use std::env;

/// Azure Active Directory Authentication Provider
/// Wraps OIDCProvider with Azure-specific configuration.
pub struct AzureProvider {
    inner: OIDCProvider,
}

impl AzureProvider {
    /// Create a new AzureProvider.
    ///
    /// # Arguments
    /// * `client_id` - The Azure App Client ID (Application ID)
    /// * `tenant_id` - The Azure Tenant ID
    /// * `base_authority` - Optional base authority, defaults to "login.microsoftonline.com"
    pub async fn new(
        client_id: &str,
        tenant_id: &str,
        base_authority: Option<&str>,
    ) -> Result<Self, FastMCPError> {
        let authority = base_authority.unwrap_or("login.microsoftonline.com");
        let issuer_url = format!("https://{}/{}/v2.0", authority, tenant_id);

        let inner = OIDCProvider::new(&issuer_url, client_id).await?;
        Ok(Self { inner })
    }

    /// Load configuration from environment variables.
    ///
    /// Expected variables:
    /// - `FASTMCP_SERVER_AUTH_AZURE_CLIENT_ID`
    /// - `FASTMCP_SERVER_AUTH_AZURE_TENANT_ID`
    /// - `FASTMCP_SERVER_AUTH_AZURE_BASE_AUTHORITY` (Optional)
    pub async fn from_env() -> Result<Self, FastMCPError> {
        let client_id = env::var("FASTMCP_SERVER_AUTH_AZURE_CLIENT_ID").map_err(|_| {
            FastMCPError::new("Missing FASTMCP_SERVER_AUTH_AZURE_CLIENT_ID".to_string())
        })?;

        let tenant_id = env::var("FASTMCP_SERVER_AUTH_AZURE_TENANT_ID").map_err(|_| {
            FastMCPError::new("Missing FASTMCP_SERVER_AUTH_AZURE_TENANT_ID".to_string())
        })?;

        let base_authority = env::var("FASTMCP_SERVER_AUTH_AZURE_BASE_AUTHORITY").ok();

        Self::new(&client_id, &tenant_id, base_authority.as_deref()).await
    }
}

#[async_trait]
impl AuthProvider for AzureProvider {
    async fn verify(&self, request: &JsonRpcRequest) -> Result<AuthContext, FastMCPError> {
        self.inner.verify(request).await
    }
}
