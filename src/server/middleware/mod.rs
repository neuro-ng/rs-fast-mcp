//! Composable request middleware.
//!
//! Middleware intercepts every incoming JSON-RPC request before it reaches the
//! server core. Implement the [`Middleware`] trait and register it via
//! [`FastMCPServer::add_middleware`](crate::server::core::FastMCPServer::add_middleware).
//!
//! Built-in middleware:
//!
//! - [`caching`] — Response caching keyed by method + params.
//! - [`rate_limiting`] — Token-bucket rate limiting per session.

use crate::error::FastMCPError;
use crate::mcp::types::{JsonRpcRequest, JsonRpcResponse};
use std::future::Future;
use std::pin::Pin;

pub mod caching;
pub mod rate_limiting;

/// A boxed, pinned, `Send` future — the return type for async middleware.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Continuation passed to [`Middleware::handle`].
///
/// Call `next(request)` to forward the (possibly modified) request to the next
/// middleware in the chain, or directly to the server core if this is the last one.
pub type Next<'a> = Box<
    dyn FnOnce(JsonRpcRequest) -> BoxFuture<'a, Result<JsonRpcResponse, FastMCPError>> + Send + 'a,
>;

/// Trait for request middleware.
///
/// Each middleware receives the incoming [`JsonRpcRequest`] and a [`Next`]
/// continuation. It may inspect or transform the request, short-circuit with an
/// error, or call `next(req)` to proceed.
pub trait Middleware: Send + Sync + 'static {
    fn handle<'a, 'b>(
        &'a self,
        req: JsonRpcRequest,
        next: Next<'b>,
    ) -> BoxFuture<'a, Result<JsonRpcResponse, FastMCPError>>
    where
        'b: 'a;
}

/// Blanket implementation allowing bare functions / closures to be used as middleware.
impl<F> Middleware for F
where
    F: Fn(JsonRpcRequest, Next<'_>) -> BoxFuture<'_, Result<JsonRpcResponse, FastMCPError>>
        + Send
        + Sync
        + 'static,
{
    fn handle<'a, 'b>(
        &'a self,
        req: JsonRpcRequest,
        next: Next<'b>,
    ) -> BoxFuture<'a, Result<JsonRpcResponse, FastMCPError>>
    where
        'b: 'a,
    {
        (self)(req, next)
    }
}
