use crate::error::FastMCPError;
use crate::mcp::types::{JsonRpcRequest, JsonRpcResponse};
use std::future::Future;
use std::pin::Pin;

pub mod caching;
pub mod rate_limiting;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

// Next is a function that takes a request and returns a future of response
// We use Box<dyn ...> to type erase the specific future/closure
pub type Next<'a> = Box<
    dyn FnOnce(JsonRpcRequest) -> BoxFuture<'a, Result<JsonRpcResponse, FastMCPError>> + Send + 'a,
>;

pub trait Middleware: Send + Sync + 'static {
    fn handle<'a, 'b>(
        &'a self,
        req: JsonRpcRequest,
        next: Next<'b>,
    ) -> BoxFuture<'a, Result<JsonRpcResponse, FastMCPError>>
    where
        'b: 'a;
}

// Allow passing simple functions as middleware
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
