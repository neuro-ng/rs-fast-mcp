//! Server-side authentication.
//!
//! Implement the [`AuthProvider`] trait to validate incoming JSON-RPC requests.
//! A convenience [`SimpleAuthProvider`] is included for static bearer-token auth.
//! Wrap any provider in [`AuthMiddleware`] to plug it into the server middleware
//! pipeline.
//!
//! See also [`crate::server::auth::providers`] for cloud-vendor provider
//! implementations (Google, GitHub, Azure, AWS, etc.).

use crate::error::FastMCPError;
use crate::mcp::types::JsonRpcRequest;
use crate::server::middleware::{BoxFuture, Middleware, Next};
use async_trait::async_trait;
use std::sync::Arc;

pub mod oauth;
pub mod oidc;
pub mod providers;

/// Context information derived from authentication.
#[derive(Debug, Clone, Default)]
pub struct AuthContext {
    pub client_id: Option<String>,
    pub user_id: Option<String>,
    pub scopes: Vec<String>,
}

/// Trait for authentication providers.
/// Providers verify a request and return an AuthContext if successful.
#[async_trait]
pub trait AuthProvider: Send + Sync {
    async fn verify(&self, request: &JsonRpcRequest) -> Result<AuthContext, FastMCPError>;
}

/// Bearer-token authentication provider.
///
/// Validates requests by checking for a matching token in either:
/// 1. The `Authorization: Bearer <token>` transport metadata header, or
/// 2. A `"token"` field in the JSON-RPC `params` (fallback).
pub struct SimpleAuthProvider {
    expected_token: String,
}

impl SimpleAuthProvider {
    pub fn new(token: &str) -> Self {
        Self {
            expected_token: token.to_string(),
        }
    }
}

#[async_trait]
impl AuthProvider for SimpleAuthProvider {
    async fn verify(&self, request: &JsonRpcRequest) -> Result<AuthContext, FastMCPError> {
        let token_opt = request
            .transport_metadata
            .as_ref()
            .and_then(|metadata| {
                metadata
                    .get("authorization")
                    .or_else(|| metadata.get("Authorization"))
            })
            .and_then(|h| h.strip_prefix("Bearer "));

        if token_opt == Some(self.expected_token.as_str()) {
            return Ok(AuthContext {
                client_id: Some("user".to_string()),
                user_id: Some("user".to_string()),
                scopes: vec!["admin".to_string()],
            });
        }

        // Fallback: look for "token" in params
        let valid = if let Some(params) = &request.params {
            if let Some(token) = params.get("token").and_then(|v| v.as_str()) {
                token == self.expected_token
            } else {
                false
            }
        } else {
            false
        };

        if valid {
            Ok(AuthContext {
                client_id: Some("user".to_string()),
                user_id: Some("user".to_string()),
                scopes: vec!["admin".to_string()],
            })
        } else {
            Err(FastMCPError::InvalidRequest(
                "Unauthorized: Invalid or missing token".to_string(),
            ))
        }
    }
}

// Define thread-local (task-local) storage for AuthContext
tokio::task_local! {
    static CURRENT_AUTH_CONTEXT: AuthContext;
}

/// Helper to retrieve the current authentication context.
/// Returns None if no authentication context is active.
pub fn current_context() -> Option<AuthContext> {
    CURRENT_AUTH_CONTEXT.try_with(|ctx| ctx.clone()).ok()
}

/// Middleware that runs every request through an [`AuthProvider`].
///
/// On success the verified [`AuthContext`] is stored in a task-local so that
/// handlers can retrieve it via [`current_context`].
/// On failure the request is rejected with a [`FastMCPError::InvalidRequest`].
pub struct AuthMiddleware {
    provider: Arc<dyn AuthProvider>,
}

impl AuthMiddleware {
    /// Wraps `provider` as a [`Middleware`] suitable for [`FastMCPServer::add_middleware`](crate::server::core::FastMCPServer::add_middleware).
    pub fn new(provider: Arc<dyn AuthProvider>) -> Self {
        Self { provider }
    }
}

impl Middleware for AuthMiddleware {
    fn handle<'a, 'b>(
        &'a self,
        request: JsonRpcRequest,
        next: Next<'b>,
    ) -> BoxFuture<'a, Result<crate::mcp::types::JsonRpcResponse, FastMCPError>>
    where
        'b: 'a,
    {
        Box::pin(async move {
            let auth_context = self.provider.verify(&request).await?;
            // Scope the next handler with the verified context
            CURRENT_AUTH_CONTEXT
                .scope(auth_context, next(request))
                .await
        })
    }
}
