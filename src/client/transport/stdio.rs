use crate::client::transport::ClientTransport;
use crate::error::FastMCPError;
use crate::mcp::types::JsonRpcMessage;
use async_trait::async_trait;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tracing::error;

#[derive(Debug)]
pub struct StdioClientTransport {
    read_rx: Mutex<mpsc::Receiver<JsonRpcMessage>>,
    write_tx: mpsc::Sender<JsonRpcMessage>,
    // Keep handle to child process if spawned?
    // For now, we spawn tokio tasks that manage I/O.
    // If we want to kill child on drop, we need to store Child handle.
    // Simplifying for now.
}

impl StdioClientTransport {
    /// Creates a transport that uses the current process's Stdin/Stdout.
    pub fn new() -> Self {
        let (read_tx, read_rx) = mpsc::channel(100);
        let (write_tx, mut write_rx) = mpsc::channel::<JsonRpcMessage>(100);

        // Spawn Reader (Stdin)
        tokio::spawn(async move {
            let stdin = tokio::io::stdin();
            let mut reader = BufReader::new(stdin);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        if let Ok(msg) = serde_json::from_str::<JsonRpcMessage>(&line) {
                            if read_tx.send(msg).await.is_err() {
                                break;
                            }
                        } else {
                            error!("Failed to parse line from stdin: {}", line);
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Spawn Writer (Stdout)
        tokio::spawn(async move {
            let mut stdout = tokio::io::stdout();
            while let Some(msg) = write_rx.recv().await {
                if let Ok(json) = serde_json::to_string(&msg) {
                    let mut data = json.into_bytes();
                    data.push(b'\n');
                    if stdout.write_all(&data).await.is_err() {
                        break;
                    }
                    if stdout.flush().await.is_err() {
                        break;
                    }
                }
            }
        });

        Self {
            read_rx: Mutex::new(read_rx),
            write_tx,
        }
    }
}

impl Default for StdioClientTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl StdioClientTransport {
    /// Creates a transport that spawns a command and communicating with it via Stdio.
    pub fn new_process(command: &str, args: &[String]) -> Result<Self, FastMCPError> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // Log stderr to parent stderr
            .spawn()
            .map_err(FastMCPError::StdIo)?;

        let stdin = child
            .stdin
            .take()
            .ok_or(FastMCPError::StdIo(std::io::Error::other(
                "Failed to open stdin",
            )))?;
        let stdout = child
            .stdout
            .take()
            .ok_or(FastMCPError::StdIo(std::io::Error::other(
                "Failed to open stdout",
            )))?;

        let (read_tx, read_rx) = mpsc::channel(100);
        let (write_tx, mut write_rx) = mpsc::channel::<JsonRpcMessage>(100);

        // Reader (Child Stdout)
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        if let Ok(msg) = serde_json::from_str::<JsonRpcMessage>(&line) {
                            if read_tx.send(msg).await.is_err() {
                                break;
                            }
                        } else {
                            error!("Failed to parse line from child: {}", line);
                        }
                    }
                    Err(e) => {
                        error!("Error reading from child stdout: {}", e);
                        break;
                    }
                }
            }
        });

        // Writer (Child Stdin)
        tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(msg) = write_rx.recv().await {
                if let Ok(json) = serde_json::to_string(&msg) {
                    let mut data = json.into_bytes();
                    data.push(b'\n');
                    if stdin.write_all(&data).await.is_err() {
                        break;
                    }
                    if stdin.flush().await.is_err() {
                        break;
                    }
                }
            }
        });

        // Ensure child is cleaned up?
        // We detached the IO. Child runs until EOF on stdin or we kill it.
        // We drop child handle, so it might become zombie if not awaited?
        // Tokio `Command` child doesn't detach on drop by default but also doesn't wait.
        // We should spawn a waiter.
        tokio::spawn(async move {
            let _ = child.wait().await;
        });

        Ok(Self {
            read_rx: Mutex::new(read_rx),
            write_tx,
        })
    }
}

#[async_trait]
impl ClientTransport for StdioClientTransport {
    async fn send(&self, message: JsonRpcMessage) -> Result<(), FastMCPError> {
        self.write_tx
            .send(message)
            .await
            .map_err(|_| FastMCPError::new("Write channel closed".to_string()))
    }

    async fn receive(&self) -> Result<JsonRpcMessage, FastMCPError> {
        let mut rx = self.read_rx.lock().await;
        rx.recv()
            .await
            .ok_or(FastMCPError::new("Read channel closed".to_string()))
    }
}
