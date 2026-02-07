use crate::error::FastMCPError;
use async_trait::async_trait;

pub mod http;
pub mod stdio;

use crate::mcp::types::{JsonRpcRequest, JsonRpcResponse};
use std::sync::Arc;

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

#[async_trait]
pub trait Transport: Send + Sync {
    /// Start the transport, listening for incoming messages and sending responses.
    /// This method should run indefinitely until the server shuts down or an error occurs.
    async fn start(
        &self,
        handler: Arc<dyn RequestHandler>,
        outbound_rx: Option<tokio::sync::broadcast::Receiver<crate::mcp::types::JsonRpcMessage>>,
    ) -> Result<(), FastMCPError>;
}
