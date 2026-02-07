use crate::error::FastMCPError;
use crate::mcp::types::JsonRpcRequest;

use async_trait::async_trait;

pub mod oidc;
pub mod oauth;
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

/// A simple provider that checks for a specific token in the "authorization" header (simulated).
/// Note: JSON-RPC requests don't inherently have headers unless passed in metadata or transport-specific context.
/// For this implementation, we assume the token might be passed in a `_meta` field in params or we rely on Transport integration later.
/// OR, we update `JsonRpcRequest` to carry transport metadata?
/// `FastMCP` context creation happens *after* middleware.
/// Wait, `JsonRpcRequest` is the raw request.
/// If we are doing HTTP Auth, the HTTP Transport parses headers.
/// The `AuthMiddleware` usually sits at the HTTP layer OR the MCP layer if the protocol supports auth frames.
/// MCP Spec says: "Authentication... is handled by the transport".
///
/// So `transport/http.rs` should extract headers and put them somewhere accessible?
/// Currently `JsonRpcRequest` is pure JSON-RPC.
///
/// Use Case:
/// The spec says `Context` holds auth info.
///
/// Let's define the trait to accept `&JsonRpcRequest`.
/// But for HTTP Bearer tokens, they are in HTTP headers, not the JSON-RPC body.
///
/// This implies `AuthProvider` needs access to connection metadata.
///
/// For now, let's implement a `SimpleAuthProvider` that assumes a magic parameter or just a stub,
/// until we bridge Transport Metadata -> Request Context.
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
        let token_opt = request.transport_metadata.as_ref()
            .and_then(|metadata| {
                 metadata.get("authorization")
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
             Err(FastMCPError::InvalidRequest("Unauthorized: Invalid or missing token".to_string()))
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

use crate::server::middleware::{Middleware, Next, BoxFuture};
use std::sync::Arc;

pub struct AuthMiddleware {
    provider: Arc<dyn AuthProvider>,
}

impl AuthMiddleware {
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
            CURRENT_AUTH_CONTEXT.scope(auth_context, next(request)).await
        })
    }
}
