use crate::error::FastMCPError;
use async_trait::async_trait;

pub mod bearer;
pub mod oauth;

#[async_trait]
pub trait AuthHandler: Send + Sync {
    /// Returns the value for the `Authorization` header.
    /// Returns None if no authentication is currently available/needed (or fails gracefully).
    async fn get_auth_header(&self) -> Result<Option<String>, FastMCPError>;
}
