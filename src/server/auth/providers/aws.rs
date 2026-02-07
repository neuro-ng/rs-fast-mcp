use crate::server::auth::oidc::OIDCProvider;
use crate::error::FastMCPError;

pub struct AwsCognito;

impl AwsCognito {
    /// Create a new AWS Cognito OIDC Provider.
    ///
    /// # Arguments
    /// * `user_pool_id` - The Cognito User Pool ID (e.g., `us-east-1_xxxxxxxxx`).
    /// * `region` - The AWS Region (e.g., `us-east-1`).
    /// * `client_id` - The Cognito App Client ID.
    pub async fn create(
        user_pool_id: &str, 
        region: &str, 
        client_id: &str
    ) -> Result<OIDCProvider, FastMCPError> {
        let issuer_url = format!("https://cognito-idp.{}.amazonaws.com/{}", region, user_pool_id);
        OIDCProvider::new(&issuer_url, client_id).await
    }
}
