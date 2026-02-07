use super::{RequestHandler, Transport};
use crate::error::FastMCPError;
use crate::mcp::types::{JsonRpcError, JsonRpcMessage, RequestId};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{error, info};

pub struct StdioTransport {}

impl StdioTransport {
    pub fn new() -> Self {
        Self {}
    }

    /// Internal method to handle streams, useful for testing
    pub async fn run_loop<R, W>(
        &self,
        reader: R,
        mut writer: W,
        handler: Arc<dyn RequestHandler>,
    ) -> Result<(), FastMCPError>
    where
        R: tokio::io::AsyncRead + Unpin + Send,
        W: tokio::io::AsyncWrite + Unpin + Send,
    {
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    info!("EOF on input, shutting down");
                    break;
                }
                Ok(_) => {
                    // Parse line as JSON
                    match serde_json::from_str::<JsonRpcMessage>(&line) {
                        Ok(message) => {
                            info!("Received message: {:?}", message);

                            match message {
                                JsonRpcMessage::Request(req) => {
                                    // Dispatch to handler
                                    let id = req.id.clone();
                                    match handler.handle_request(req).await {
                                        Ok(resp) => {
                                            let resp_str = serde_json::to_string(
                                                &JsonRpcMessage::Response(resp),
                                            )
                                            .map_err(FastMCPError::Json)?;
                                            writer
                                                .write_all(resp_str.as_bytes())
                                                .await
                                                .map_err(FastMCPError::StdIo)?;
                                            writer
                                                .write_all(b"\n")
                                                .await
                                                .map_err(FastMCPError::StdIo)?;
                                        }
                                        Err(e) => {
                                            error!("Handler error: {}", e);
                                            // Send generic error or detailed?
                                            // The error type in FastMCPError might not map perfectly to JsonRpcError yet without a helper.
                                            // For now, simpler error response.
                                            let err_resp = JsonRpcError::new(
                                                id,
                                                -32603, // Internal error
                                                &format!("Internal error: {}", e),
                                                None,
                                            );
                                            let resp_str = serde_json::to_string(
                                                &JsonRpcMessage::Error(err_resp),
                                            )
                                            .map_err(FastMCPError::Json)?;
                                            writer
                                                .write_all(resp_str.as_bytes())
                                                .await
                                                .map_err(FastMCPError::StdIo)?;
                                            writer
                                                .write_all(b"\n")
                                                .await
                                                .map_err(FastMCPError::StdIo)?;
                                        }
                                    }
                                }
                                JsonRpcMessage::Notification(notif) => {
                                    if let Err(e) = handler.handle_notification(notif).await {
                                        error!("Notification handler error: {}", e);
                                    }
                                }
                                _ => {
                                    // Responses?
                                    // For simple server, we might ignore unless we are acting as client too (loopback).
                                }
                            }
                            writer.flush().await.map_err(FastMCPError::StdIo)?;
                        }
                        Err(e) => {
                            error!("Failed to parse JSON: {}", e);
                            let err_resp =
                                JsonRpcError::new(RequestId::Int(0), -32700, "Parse error", None);
                            let resp_str = serde_json::to_string(&JsonRpcMessage::Error(err_resp))
                                .map_err(FastMCPError::Json)?;
                            writer
                                .write_all(resp_str.as_bytes())
                                .await
                                .map_err(FastMCPError::StdIo)?;
                            writer.write_all(b"\n").await.map_err(FastMCPError::StdIo)?;
                            writer.flush().await.map_err(FastMCPError::StdIo)?;
                        }
                    }
                }
                Err(e) => {
                    error!("Error reading input: {}", e);
                    return Err(FastMCPError::StdIo(e));
                }
            }
        }
        Ok(())
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn start(
        &self,
        handler: Arc<dyn RequestHandler>,
        outbound_rx: Option<tokio::sync::broadcast::Receiver<JsonRpcMessage>>,
    ) -> Result<(), FastMCPError> {
        info!("Starting Stdio transport");
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();

        if let Some(rx) = outbound_rx {
            Self::run_loop_with_notifications(tokio::io::BufReader::new(stdin), stdout, handler, rx)
                .await
        } else {
            self.run_loop(stdin, stdout, handler).await
        }
    }
}

impl StdioTransport {
    async fn run_loop_with_notifications<R, W>(
        reader: R,
        mut writer: W,
        handler: Arc<dyn RequestHandler>,
        mut rx: tokio::sync::broadcast::Receiver<JsonRpcMessage>,
    ) -> Result<(), FastMCPError>
    where
        R: tokio::io::AsyncRead + Unpin + Send,
        W: tokio::io::AsyncWrite + Unpin + Send,
    {
        let mut reader = tokio::io::BufReader::new(reader); // Double buffering if R is already BufReader, but explicitly locally usually implies raw R.
        // Wait, typical pattern: BufReader::new(reader).
        // If passed R is already BufReader, it wraps again.
        // To allow both, let's assume R is `AsyncRead` and we wrap it.
        // In start() above I did `BufReader::new(stdin)`.
        // Better to match `run_loop` signature where `reader` is R.

        let mut line = String::new();

        loop {
            tokio::select! {
                res = reader.read_line(&mut line) => {
                    match res {
                         Ok(0) => {
                            info!("EOF on input, shutting down");
                            break;
                        }
                        Ok(_) => {
                             match serde_json::from_str::<JsonRpcMessage>(&line) {
                                Ok(message) => {
                                      match message {
                                        JsonRpcMessage::Request(req) => {
                                            let id = req.id.clone();
                                            match handler.handle_request(req).await {
                                                Ok(resp) => {
                                                    let resp_str = serde_json::to_string(&JsonRpcMessage::Response(resp)).map_err(FastMCPError::Json)?;
                                                    writer.write_all(resp_str.as_bytes()).await.map_err(FastMCPError::StdIo)?;
                                                    writer.write_all(b"\n").await.map_err(FastMCPError::StdIo)?;
                                                },
                                                Err(e) => {
                                                     let err_resp = JsonRpcError::new(id, -32603, &format!("Internal error: {}", e), None);
                                                     let resp_str = serde_json::to_string(&JsonRpcMessage::Error(err_resp)).map_err(FastMCPError::Json)?;
                                                     writer.write_all(resp_str.as_bytes()).await.map_err(FastMCPError::StdIo)?;
                                                     writer.write_all(b"\n").await.map_err(FastMCPError::StdIo)?;
                                                }
                                            }
                                        },
                                        JsonRpcMessage::Notification(notif) => {
                                            let _ = handler.handle_notification(notif).await;
                                        },
                                        _ => {}
                                    }
                                    writer.flush().await.map_err(FastMCPError::StdIo)?;
                                }
                                Err(e) => {
                                     error!("Failed to parse message: {}", e);
                                }
                             }
                             line.clear();
                        }
                        Err(e) => {
                            error!("Error reading line: {}", e);
                            break;
                        }
                    }
                }
                msg = rx.recv() => {
                    match msg {
                        Ok(outgoing_msg) => {
                            let resp_str = serde_json::to_string(&outgoing_msg).map_err(FastMCPError::Json)?;
                            writer.write_all(resp_str.as_bytes()).await.map_err(FastMCPError::StdIo)?;
                            writer.write_all(b"\n").await.map_err(FastMCPError::StdIo)?;
                            writer.flush().await.map_err(FastMCPError::StdIo)?;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            break;
                        }
                         Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                             tracing::warn!("Skipped {} lagged messages", n);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::transport::{JsonRpcRequest, JsonRpcResponse, RequestHandler};
    use serde_json::{Value, json};
    use std::io::Cursor;

    struct MockHandler;
    #[async_trait]
    impl RequestHandler for MockHandler {
        async fn handle_request(
            &self,
            request: JsonRpcRequest,
        ) -> Result<JsonRpcResponse, FastMCPError> {
            if request.method == "ping" {
                Ok(JsonRpcResponse::new(request.id, Value::Null))
            } else {
                Err(FastMCPError::InvalidRequest("Method not found".to_string()))
            }
        }

        async fn handle_notification(
            &self,
            _notification: crate::mcp::types::JsonRpcNotification,
        ) -> Result<(), FastMCPError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_stdio_ping() {
        let transport = StdioTransport::new();

        // Input: valid ping request
        let input_data = r#"{"jsonrpc": "2.0", "method": "ping", "id": 1}"#;
        // Need newline
        let input = Cursor::new(format!("{}\n", input_data));

        let mut output = Vec::new();
        let handler = Arc::new(MockHandler);

        transport
            .run_loop(input, &mut output, handler)
            .await
            .unwrap();

        let output_str = String::from_utf8(output).unwrap();
        let resp: JsonRpcMessage = serde_json::from_str(&output_str).unwrap();

        if let JsonRpcMessage::Response(r) = resp {
            assert_eq!(r.id, RequestId::Int(1));
        } else {
            panic!("Expected Response");
        }
    }

    #[tokio::test]
    async fn test_stdio_notifications() {
        let (tx, rx) = tokio::sync::broadcast::channel(10);
        let handler = Arc::new(MockHandler);

        // Uses a duplex stream to simulate Stdin that stays open
        let (client_read, client_write) = tokio::io::duplex(1024);
        let mut output = Vec::new();

        // Spawn transport loop
        let handle = tokio::spawn(async move {
            StdioTransport::run_loop_with_notifications(client_read, &mut output, handler, rx)
                .await
                .unwrap();
            output
        });

        // Send notification
        let notif_msg = JsonRpcMessage::Notification(crate::mcp::types::JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "test/method".to_string(),
            params: Some(json!({"foo": "bar"})),
        });
        tx.send(notif_msg).unwrap();

        // Allow some time for processing
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Drop client_write to signal EOF to reader
        drop(client_write);

        // Drop sender to signal closed (though loop likely breaks on EOF first)
        drop(tx);

        let output = handle.await.unwrap();
        let output_str = String::from_utf8(output).unwrap();

        // Verify output contains the notification
        assert!(output_str.contains("test/method"));
        assert!(output_str.contains("jsonrpc"));
    }
}
