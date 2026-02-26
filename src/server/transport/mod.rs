//! Server-side transport layer.
//!
//! A [`Transport`] listens for incoming connections and dispatches messages to a
//! [`RequestHandler`]. The server can run multiple transports simultaneously.
//!
//! Built-in implementations:
//!
//! - [`stdio`] — Reads JSON-RPC messages from stdin, writes responses to stdout.
//! - [`http`] — Actix-Web HTTP server with SSE session management.

pub mod http;
pub mod stdio;

use crate::error::FastMCPError;
use crate::mcp::types::{JsonRpcRequest, JsonRpcResponse};
use async_trait::async_trait;
use std::sync::Arc;

/// Handles a single incoming JSON-RPC request or notification.
///
/// Implemented by [`FastMCPServer`](crate::server::core::FastMCPServer), which
/// routes each request through middleware and then to the appropriate handler.
#[async_trait]
pub trait RequestHandler: Send + Sync {
    async fn handle_request(
        &self,
        request: JsonRpcRequest,
    ) -> Result<JsonRpcResponse, FastMCPError>;

    async fn handle_notification(
        &self,
        notification: crate::mcp::types::JsonRpcNotification,
    ) -> Result<(), FastMCPError>;
}

/// A server-side transport that listens for incoming connections and dispatches
/// JSON-RPC messages to a [`RequestHandler`].
///
/// Implement this trait to add new transports (e.g. WebSocket, Unix socket).
/// `start` should run until shutdown or a fatal error, then return.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Begins accepting messages, driving `handler` for each request.
    ///
    /// `outbound_rx` receives server-initiated notifications (e.g. list-changed
    /// events) that should be forwarded to connected clients.
    async fn start(
        &self,
        handler: Arc<dyn RequestHandler>,
        outbound_rx: Option<tokio::sync::broadcast::Receiver<crate::mcp::types::JsonRpcMessage>>,
    ) -> Result<(), FastMCPError>;
}
