//! Event store abstraction for stateless HTTP/SSE horizontal scaling.
//!
//! In stateless mode the SSE server does not keep per-connection in-memory
//! queues; instead, messages are published to an external event store (e.g.
//! Redis) and each SSE client polls its channel, enabling multiple server
//! instances behind a load balancer to share the same logical session.

use crate::error::FastMCPError;
use async_trait::async_trait;

/// Pluggable backend for distributing SSE events across stateless server nodes.
#[async_trait]
pub trait EventStore: Send + Sync + std::fmt::Debug {
    /// Publish a JSON-RPC message to the named channel.
    async fn publish(
        &self,
        channel: &str,
        message: serde_json::Value,
    ) -> Result<(), FastMCPError>;

    /// Subscribe to a channel and return a stream of incoming messages.
    async fn subscribe(
        &self,
        channel: &str,
    ) -> Result<
        tokio_stream::wrappers::ReceiverStream<serde_json::Value>,
        FastMCPError,
    >;
}

/// In-process event store backed by `tokio::sync::broadcast`.
///
/// Useful for testing and single-node deployments where statefulness is
/// acceptable. For true stateless scaling replace this with a Redis backend.
#[derive(Debug)]
pub struct InMemoryEventStore {
    sender: tokio::sync::broadcast::Sender<(String, serde_json::Value)>,
}

impl Default for InMemoryEventStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryEventStore {
    pub fn new() -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(1024);
        Self { sender: tx }
    }
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    async fn publish(
        &self,
        channel: &str,
        message: serde_json::Value,
    ) -> Result<(), FastMCPError> {
        let _ = self.sender.send((channel.to_string(), message));
        Ok(())
    }

    async fn subscribe(
        &self,
        channel: &str,
    ) -> Result<
        tokio_stream::wrappers::ReceiverStream<serde_json::Value>,
        FastMCPError,
    > {
        let mut rx = self.sender.subscribe();
        let (tx, local_rx) = tokio::sync::mpsc::channel(256);
        let ch = channel.to_string();

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok((c, msg)) if c == ch => {
                        if tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {} // different channel — skip
                    Err(_) => break,
                }
            }
        });

        Ok(tokio_stream::wrappers::ReceiverStream::new(local_rx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn test_in_memory_event_store_pubsub() {
        let store = InMemoryEventStore::new();
        let mut stream = store.subscribe("test_channel").await.unwrap();

        store
            .publish("test_channel", serde_json::json!({"event": "ping"}))
            .await
            .unwrap();

        let msg = stream.next().await.unwrap();
        assert_eq!(msg["event"], "ping");
    }

    #[tokio::test]
    async fn test_channel_isolation() {
        let store = InMemoryEventStore::new();
        let mut stream = store.subscribe("channel_a").await.unwrap();

        // Publish to a different channel
        store
            .publish("channel_b", serde_json::json!({"event": "noise"}))
            .await
            .unwrap();

        store
            .publish("channel_a", serde_json::json!({"event": "signal"}))
            .await
            .unwrap();

        let msg = stream.next().await.unwrap();
        assert_eq!(msg["event"], "signal");
    }
}
