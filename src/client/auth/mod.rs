//! Client-side authentication handlers.
//!
//! The [`AuthHandler`] trait produces an `Authorization` header value that is
//! injected into outgoing transport messages. Built-in implementations:
//!
//! - [`bearer`] — Static bearer-token authentication.
//! - [`oauth`] — OAuth 2.0 token refresh flow.

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
