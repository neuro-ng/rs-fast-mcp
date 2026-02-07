use crate::client::auth::AuthHandler;
use crate::error::FastMCPError;
use async_trait::async_trait;

pub struct BearerAuth {
    token: String,
}

impl BearerAuth {
    pub fn new(token: String) -> Self {
        Self { token }
    }
}

#[async_trait]
impl AuthHandler for BearerAuth {
    async fn get_auth_header(&self) -> Result<Option<String>, FastMCPError> {
        Ok(Some(format!("Bearer {}", self.token)))
    }
}
