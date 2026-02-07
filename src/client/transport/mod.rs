use async_trait::async_trait;
use crate::error::FastMCPError;
use crate::mcp::types::JsonRpcMessage;
use std::fmt::Debug;

#[async_trait]
pub trait ClientTransport: Send + Sync + Debug {
    // Sends a message (Request, Response, or Notification)
    async fn send(&self, message: JsonRpcMessage) -> Result<(), FastMCPError>;
    // Receives the next message. Should block/await until a message is available.
    async fn receive(&self) -> Result<JsonRpcMessage, FastMCPError>;
}

pub mod sse;
pub mod stdio;
