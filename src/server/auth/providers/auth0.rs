use crate::server::auth::oidc::OIDCProvider;
use crate::error::FastMCPError;

pub struct Auth0;

impl Auth0 {
    /// Create a new Auth0 OIDC Provider.
    /// 
    /// # Arguments
    /// * `domain` - The Auth0 domain (e.g., `dev-xyz.us.auth0.com`).
    /// * `client_id` - The Auth0 Client ID.
    pub async fn create(domain: &str, client_id: &str) -> Result<OIDCProvider, FastMCPError> {
        // Auth0 issuer URL is https://{domain}/
        // Ensure domain doesn't have protocol or trailing slash to avoid double-adding
        let clean_domain = domain
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/');
            
        let issuer_url = format!("https://{}/", clean_domain);
        OIDCProvider::new(&issuer_url, client_id).await
    }
}
