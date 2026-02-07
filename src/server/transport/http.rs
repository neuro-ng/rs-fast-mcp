use crate::error::FastMCPError;
use crate::mcp::types::JsonRpcMessage;
use crate::server::transport::Transport;
use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use actix_web_lab::sse::{self, Sse};
use async_trait::async_trait;
use futures::StreamExt;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{error, info};

use crate::server::transport::RequestHandler;

// Application state to hold the broadcast channel
pub(crate) struct AppState {
    tx: broadcast::Sender<Arc<String>>, // Broadcast JSON strings (SSE events)
}

struct TransportState {
    app_state: Arc<AppState>,
    handler: Arc<dyn RequestHandler>,
}

pub type AppConfigCallback = Arc<dyn Fn(&mut web::ServiceConfig) + Send + Sync>;

pub struct HttpTransport {
    host: String,
    port: u16,
    state: Arc<AppState>,
    app_configs: Vec<AppConfigCallback>,
}

impl HttpTransport {
    pub fn new(host: &str, port: u16) -> Self {
        let (tx, _rx) = broadcast::channel(100);
        Self {
            host: host.to_string(),
            port,
            state: Arc::new(AppState { tx }),
            app_configs: Vec::new(),
        }
    }

    pub fn with_app_config<F>(mut self, config: F) -> Self
    where
        F: Fn(&mut web::ServiceConfig) + Send + Sync + 'static,
    {
        self.app_configs.push(Arc::new(config));
        self
    }

    // Explicitly expose app factory for testing (Actix style testing uses App factory)
    // However, Actix's test::init_service takes a factory.
    // We can just keep the logic inside start or expose a configure method.
    // For simplicity and matching previous structure, we'll keep logic in start/handlers here.

    #[cfg(test)]
    pub(crate) fn get_state(&self) -> Arc<AppState> {
        self.state.clone()
    }

    fn build_server(
        &self,
        handler: Arc<dyn RequestHandler>,
    ) -> std::io::Result<actix_web::dev::Server> {
        let transport_state = web::Data::new(TransportState {
            app_state: self.state.clone(),
            handler,
        });
        let port = self.port;

        let host = self.host.clone();
        let configs = self.app_configs.clone();

        let server = HttpServer::new(move || {
            let mut app = App::new()
                .app_data(transport_state.clone())
                .route("/sse", web::get().to(sse_handler))
                .route("/message", web::post().to(message_handler));

            for config in &configs {
                app = app.configure(|cfg| config(cfg));
            }
            app
        })
        .bind((host, port))?
        .run();

        Ok(server)
    }
}

// Handlers

async fn sse_handler(state: web::Data<TransportState>) -> impl Responder {
    let rx = state.app_state.tx.subscribe();

    // Map broadcast receiver to SSE events
    // Broadcast errors (Lagged) should be handled or ignored (skipped)
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|msg| async move {
        match msg {
            Ok(json_str) => Some(Ok::<_, actix_web::Error>(sse::Event::Data(sse::Data::new(
                json_str.as_str(),
            )))),
            Err(_) => None, // Skip lagged messages
        }
    });

    Sse::from_stream(stream).with_keep_alive(std::time::Duration::from_secs(10))
}

async fn message_handler(
    req: actix_web::HttpRequest,
    state: web::Data<TransportState>,
    body: web::Json<JsonRpcMessage>,
) -> impl Responder {
    info!("Received message via HTTP: {:?}", body);

    let mut message = body.0;

    // Extract headers if it's a request
    if let JsonRpcMessage::Request(ref mut rpc_req) = message {
        let mut metadata = std::collections::HashMap::new();
        for (key, value) in req.headers() {
            if let Ok(v) = value.to_str() {
                metadata.insert(key.as_str().to_string(), v.to_string());
            }
        }
        rpc_req.transport_metadata = Some(metadata);
    }

    match message {
        JsonRpcMessage::Request(req) => {
            // Dispatch to handler
            match state.handler.handle_request(req).await {
                Ok(resp) => {
                    // Also broadcast to SSE clients? Usually notifications or specific events.
                    // For now, just return response.
                    HttpResponse::Ok().json(JsonRpcMessage::Response(resp))
                }
                Err(e) => {
                    error!("Handler error: {}", e);
                    HttpResponse::InternalServerError().json(json!({
                        "jsonrpc": "2.0",
                        "error": {
                            "code": -32603,
                            "message": format!("Internal error: {}", e)
                        },
                        "id": null
                    }))
                }
            }
        }
        _ => {
            // Notifications etc.
            // Broadcast?
            match serde_json::to_string(&message) {
                Ok(json_str) => {
                    let _ = state.app_state.tx.send(Arc::new(json_str));
                    HttpResponse::Ok().json(json!({"status": "ok"}))
                }
                Err(e) => {
                    error!("Failed to serialize message: {}", e);
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn start(
        &self,
        handler: Arc<dyn RequestHandler>,
        outbound_rx: Option<tokio::sync::broadcast::Receiver<JsonRpcMessage>>,
    ) -> Result<(), FastMCPError> {
        info!("Starting HTTP/SSE server on {}:{}", self.host, self.port);

        // Bridge outbound core notifications to SSE broadcast channel
        if let Some(mut rx) = outbound_rx {
            let tx = self.state.tx.clone();
            tokio::spawn(async move {
                while let Ok(msg) = rx.recv().await {
                    match serde_json::to_string(&msg) {
                        Ok(json_str) => {
                            let _ = tx.send(Arc::new(json_str));
                        }
                        Err(e) => {
                            error!("Failed to serialize outbound notification for SSE: {}", e);
                        }
                    }
                }
            });
        }

        let server = self.build_server(handler).map_err(FastMCPError::StdIo)?;
        server.await.map_err(FastMCPError::StdIo)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::types::{JsonRpcRequest, JsonRpcResponse};
    use actix_web::{App, test};
    use serde_json::Value;

    struct MockHandler;
    #[async_trait]
    impl RequestHandler for MockHandler {
        async fn handle_request(
            &self,
            request: JsonRpcRequest,
        ) -> Result<JsonRpcResponse, FastMCPError> {
            Ok(JsonRpcResponse::new(request.id, Value::Null))
        }

        async fn handle_notification(
            &self,
            _notification: crate::mcp::types::JsonRpcNotification,
        ) -> Result<(), FastMCPError> {
            Ok(())
        }
    }

    #[actix_web::test]
    async fn test_message_handler() {
        let transport = HttpTransport::new("127.0.0.1", 0);
        let handler = Arc::new(MockHandler);
        let transport_state = web::Data::new(TransportState {
            app_state: transport.get_state(),
            handler,
        });

        let app = test::init_service(
            App::new()
                .app_data(transport_state.clone())
                .route("/message", web::post().to(message_handler)),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/message")
            .set_json(json!({
                "jsonrpc": "2.0",
                "method": "ping",
                "id": 1
            }))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }
}
