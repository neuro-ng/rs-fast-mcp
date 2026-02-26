//! Client-side transport abstractions.
//!
//! The [`ClientTransport`] trait defines the send/receive interface that concrete
//! transports (Stdio, SSE) implement. The [`Client`](super::Client) drives the
//! transport in a background loop.
//!
//! - [`stdio`] — Spawns a child process and communicates over stdin/stdout.
//! - [`sse`] — Connects to an HTTP/SSE endpoint.

use crate::error::FastMCPError;
use crate::mcp::types::JsonRpcMessage;
use async_trait::async_trait;
use std::fmt::Debug;

/// Low-level send/receive interface implemented by each client transport.
#[async_trait]
pub trait ClientTransport: Send + Sync + Debug {
    /// Sends a JSON-RPC message to the server.
    async fn send(&self, message: JsonRpcMessage) -> Result<(), FastMCPError>;
    /// Waits for and returns the next incoming JSON-RPC message.
    async fn receive(&self) -> Result<JsonRpcMessage, FastMCPError>;
}

pub mod sse;
pub mod stdio;
