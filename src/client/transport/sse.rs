use crate::client::transport::ClientTransport;
use crate::error::FastMCPError;
use crate::mcp::types::JsonRpcMessage;
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::stream::StreamExt;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::sync::mpsc;

use crate::client::auth::AuthHandler;

use std::fmt;

#[derive(Clone)]
pub struct SseClientTransport {
    url: String,
    http_client: Client,
    endpoint: Arc<RwLock<Option<String>>>,
    read_rx: Arc<Mutex<mpsc::Receiver<JsonRpcMessage>>>,
    auth_handler: Option<Arc<dyn AuthHandler>>,
}

impl fmt::Debug for SseClientTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SseClientTransport")
            .field("url", &self.url)
            .field("http_client", &self.http_client)
            .field("endpoint", &self.endpoint)
            .field("read_rx", &self.read_rx)
            .field(
                "auth_handler",
                &if self.auth_handler.is_some() {
                    "Some(AuthHandler)"
                } else {
                    "None"
                },
            )
            .finish()
    }
}

impl SseClientTransport {
    pub fn new(url: String, auth_handler: Option<Arc<dyn AuthHandler>>) -> Self {
        let (tx, rx) = mpsc::channel(100);
        let transport = Self {
            url: url.clone(),
            http_client: Client::new(),
            endpoint: Arc::new(RwLock::new(None)),
            read_rx: Arc::new(Mutex::new(rx)),
            auth_handler,
        };

        transport.spawn_listener(tx);
        transport
    }

    fn spawn_listener(&self, tx: mpsc::Sender<JsonRpcMessage>) {
        let client = self.http_client.clone();
        let url = self.url.clone();
        let endpoint = self.endpoint.clone();

        let auth_handler = self.auth_handler.clone();

        tokio::spawn(async move {
            loop {
                // Reconnection logic could go here
                let mut req_builder = client.get(&url);

                if let Some(auth) = &auth_handler
                    && let Ok(Some(token)) = auth.get_auth_header().await
                {
                    req_builder = req_builder.header("Authorization", token);
                }

                let response_res = req_builder.send().await;
                match response_res {
                    Ok(response) => {
                        if !response.status().is_success() {
                            eprintln!("SSE connection failed: {}", response.status());
                            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                            continue;
                        }

                        let mut stream = response.bytes_stream().eventsource();
                        while let Some(event_res) = stream.next().await {
                            match event_res {
                                Ok(event) => {
                                    if event.event == "endpoint" {
                                        let mut lock = endpoint.write().await;
                                        *lock = Some(event.data);
                                        continue;
                                    }
                                    if event.event == "message"
                                        && let Ok(msg) =
                                            serde_json::from_str::<JsonRpcMessage>(&event.data)
                                        && tx.send(msg).await.is_err()
                                    {
                                        return; // Receiver dropped, stop
                                    }
                                }
                                Err(_) => break, // Stream error, reconnect
                            }
                        }
                    }
                    Err(_) => {
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                }
            }
        });
    }
}

#[async_trait]
impl ClientTransport for SseClientTransport {
    async fn send(&self, message: JsonRpcMessage) -> Result<(), FastMCPError> {
        let endpoint_url = {
            let lock = self.endpoint.read().await;
            lock.clone()
        };

        if let Some(uri) = endpoint_url {
            // Resolve relative URI logic simplified
            let target = if uri.starts_with("http") {
                uri
            } else {
                // Naive join
                format!("{}{}", self.url.trim_end_matches("/sse"), uri)
            };

            let mut req_builder = self.http_client.post(&target);

            if let Some(auth) = &self.auth_handler
                && let Ok(Some(token)) = auth.get_auth_header().await
            {
                req_builder = req_builder.header("Authorization", token);
            }

            req_builder
                .json(&message)
                .send()
                .await
                .map_err(|e| FastMCPError::new(format!("Failed to send message: {}", e)))?;
            Ok(())
        } else {
            Err(FastMCPError::new(
                "No endpoint available for sending".to_string(),
            ))
        }
    }

    async fn receive(&self) -> Result<JsonRpcMessage, FastMCPError> {
        let mut rx = self.read_rx.lock().await;
        rx.recv()
            .await
            .ok_or(FastMCPError::new("Channel closed".to_string()))
    }
}
