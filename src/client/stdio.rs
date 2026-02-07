use crate::error::FastMCPError;
use crate::client::{ClientTransport, JsonRpcMessage};
use async_trait::async_trait;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Command, Child};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct StdioClientTransport {
    _child: Arc<Mutex<Child>>,
    reader: Arc<Mutex<BufReader<tokio::process::ChildStdout>>>,
    writer: Arc<Mutex<tokio::process::ChildStdin>>,
    // We need a way to correlate responses.
    // Basic implementation: send request, wait for next line? 
    // No, JSON-RPC is asynchronous. We need a background listener loop that dispatches responses.
    // BUT the `ClientTransport` trait currently assumes `send_request` returns `JsonRpcMessage` (response).
    // This implies a synchronous request-response flow for `send_request`.
    // For Stdio, if we only do 1 request at a time, we can wait for the response.
    // Limit: No concurrent requests.
}

impl StdioClientTransport {
    pub fn new(command: &str, args: &[&str]) -> Result<Self, FastMCPError> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(FastMCPError::StdIo)?;

        let stdin = child.stdin.take().ok_or_else(|| FastMCPError::StdIo(std::io::Error::other("Failed to open stdin")))?;
        let stdout = child.stdout.take().ok_or_else(|| FastMCPError::StdIo(std::io::Error::other("Failed to open stdout")))?;

        Ok(Self {
            _child: Arc::new(Mutex::new(child)),
            reader: Arc::new(Mutex::new(BufReader::new(stdout))),
            writer: Arc::new(Mutex::new(stdin)),
        })
    }
}

#[async_trait]
impl ClientTransport for StdioClientTransport {
    async fn send(&self, message: JsonRpcMessage) -> Result<(), FastMCPError> {
        let req_str = serde_json::to_string(&message).map_err(FastMCPError::Json)?;
        
        let mut writer = self.writer.lock().await;
        writer.write_all(req_str.as_bytes()).await.map_err(FastMCPError::StdIo)?;
        writer.write_all(b"\n").await.map_err(FastMCPError::StdIo)?;
        writer.flush().await.map_err(FastMCPError::StdIo)?;
        Ok(())
    }

    async fn receive(&self) -> Result<JsonRpcMessage, FastMCPError> {
        let mut reader = self.reader.lock().await;
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).await.map_err(FastMCPError::StdIo)?;
        if bytes_read == 0 {
            return Err(FastMCPError::StdIo(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Server closed connection")));
        }

        // Parse json (generic Value first to identify type?)
        // Or directly deserialize JsonRpcMessage? 
        // JsonRpcMessage is enum. Serde can handle it if tagged appropriately.
        // `src/mcp/types.rs` definition:
        // #[derive(Serialize, Deserialize)]
        // #[serde(untagged)]
        // pub enum JsonRpcMessage { Request, Response, Notification, Error }
        // If it's untagged, serde tries to match variants.
        
        let val: JsonRpcMessage = serde_json::from_str(&line).map_err(FastMCPError::Json)?;
        Ok(val)
    }
}
