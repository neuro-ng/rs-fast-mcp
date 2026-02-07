use async_trait::async_trait;
use rs_fast_mcp::client::{Client, ClientTransport};
use rs_fast_mcp::error::FastMCPError;
use rs_fast_mcp::mcp::types::{ErrorData, JsonRpcError, JsonRpcMessage};

use std::time::Duration;
use tokio::sync::Mutex;

// MockTransport removed (unused)

// We need interior mutability for the mock to respond to specific requests?
// Or simpler: The mock just returns a pre-configured response when receive() is called.
// Since send() and receive() are decoupled in the trait, receive() is called in a loop.
// We need to coordinate: send() happens, then receive() returns something matching the ID.
// The Client generates IDs. "1" is the first one.

#[derive(Debug)]
struct CoordinatedMockTransport {
    rx: Mutex<tokio::sync::mpsc::Receiver<JsonRpcMessage>>,
}

#[async_trait]
impl ClientTransport for CoordinatedMockTransport {
    async fn send(&self, _message: JsonRpcMessage) -> Result<(), FastMCPError> {
        Ok(())
    }
    async fn receive(&self) -> Result<JsonRpcMessage, FastMCPError> {
        let mut rx = self.rx.lock().await;
        if let Some(msg) = rx.recv().await {
            Ok(msg)
        } else {
            std::future::pending().await
        }
    }
}

#[tokio::test]
async fn test_client_error_handling() {
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    let transport = CoordinatedMockTransport { rx: Mutex::new(rx) };
    let client = Client::builder(Box::new(transport)).build();

    // Prepare error response to be "received"
    let err_response = JsonRpcMessage::Error(JsonRpcError {
        jsonrpc: "2.0".to_string(),
        id: rs_fast_mcp::mcp::types::RequestId::Int(1),
        error: ErrorData {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
        },
    });

    // Spawn a task to send the error response *after* the client sends request?
    // Actually Client starts background loop immediately.
    // We can send to tx immediately.
    tx.send(err_response).await.unwrap();

    let result = client.request("unknown_method", None).await;

    match result {
        Err(FastMCPError::JsonRpcError { code, message, .. }) => {
            assert_eq!(code, -32601);
            assert_eq!(message, "Method not found");
        }
        _ => panic!("Expected JsonRpcError, got {:?}", result),
    }
}

#[tokio::test]
async fn test_client_timeout() {
    let (_tx, rx) = tokio::sync::mpsc::channel(1);
    let transport = CoordinatedMockTransport { rx: Mutex::new(rx) };

    // Configure client with short timeout
    let client = Client::builder(Box::new(transport))
        .timeout(Duration::from_millis(100))
        .build();

    // Do NOT send any response.

    let result = client.request("slow_method", None).await;

    match result {
        Err(FastMCPError::InvalidRequest(msg)) => {
            assert_eq!(msg, "Request timed out");
        }
        _ => panic!("Expected Timeout error, got {:?}", result),
    }
}
