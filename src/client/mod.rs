use crate::error::FastMCPError;
use crate::mcp::types::{JsonRpcMessage, JsonRpcRequest, JsonRpcResponse, RequestId};

use dashmap::DashMap;
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::sync::oneshot;
use tracing::{debug, error, warn};

pub mod builder;

pub mod transport;
pub use crate::client::transport::ClientTransport;
pub mod auth;

type RequestHandler = Box<
    dyn Fn(
            JsonRpcRequest,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<Option<Value>, FastMCPError>> + Send>,
        > + Send
        + Sync,
>;

pub struct Client {
    transport: Arc<Box<dyn ClientTransport>>,
    request_id_counter: AtomicI64,
    pending_requests:
        Arc<DashMap<RequestId, oneshot::Sender<Result<JsonRpcResponse, FastMCPError>>>>,
    request_handlers: Arc<DashMap<String, RequestHandler>>,
    timeout: Option<std::time::Duration>,
}

impl Client {
    pub fn new(transport: Box<dyn ClientTransport>) -> Self {
        let transport = Arc::new(transport);
        let pending_requests = Arc::new(DashMap::new());
        let request_handlers: Arc<DashMap<String, RequestHandler>> = Arc::new(DashMap::new());

        let client = Self {
            transport: transport.clone(),
            request_id_counter: AtomicI64::new(1),
            pending_requests: pending_requests.clone(),
            request_handlers: request_handlers.clone(),
            timeout: None,
        };

        client.start_background_loop(transport, pending_requests, request_handlers);
        client
    }

    pub fn builder(transport: Box<dyn ClientTransport>) -> crate::client::builder::ClientBuilder {
        crate::client::builder::ClientBuilder::new(transport)
    }

    pub(crate) fn set_timeout(&mut self, timeout: Option<std::time::Duration>) {
        self.timeout = timeout;
    }

    fn start_background_loop(
        &self,
        transport: Arc<Box<dyn ClientTransport>>,
        pending_requests: Arc<
            DashMap<RequestId, oneshot::Sender<Result<JsonRpcResponse, FastMCPError>>>,
        >,
        request_handlers: Arc<DashMap<String, RequestHandler>>,
    ) {
        let transport_clone = transport.clone();
        tokio::spawn(async move {
            loop {
                match transport.receive().await {
                    Ok(message) => {
                        match message {
                            JsonRpcMessage::Response(response) => {
                                if let Some((_, sender)) = pending_requests.remove(&response.id) {
                                    let _ = sender.send(Ok(response));
                                } else {
                                    warn!(
                                        "Received response for unknown request ID: {:?}",
                                        response.id
                                    );
                                }
                            }
                            JsonRpcMessage::Error(err_msg) => {
                                // ErrorResponse also has ID
                                if let Some((_, sender)) = pending_requests.remove(&err_msg.id) {
                                    let err = FastMCPError::JsonRpcError {
                                        code: err_msg.error.code,
                                        message: err_msg.error.message,
                                        data: err_msg.error.data,
                                    };
                                    let _ = sender.send(Err(err));
                                } else {
                                    warn!(
                                        "Received error for unknown request ID: {:?}",
                                        err_msg.id
                                    );
                                }
                            }
                            JsonRpcMessage::Request(request) => {
                                // Handle incoming request (e.g. Sampling, Ping, Roots)
                                let method = request.method.clone();
                                let id = request.id.clone();
                                let handlers = request_handlers.clone();
                                let transport = transport_clone.clone();

                                tokio::spawn(async move {
                                    let result = if let Some(handler) = handlers.get(&method) {
                                        handler(request).await
                                    } else {
                                        // Default: Method Not Found?
                                        Err(FastMCPError::InvalidRequest(format!(
                                            "Method not found: {}",
                                            method
                                        )))
                                    };

                                    // Send Response back
                                    let response = match result {
                                        Ok(res_opt) => {
                                            // Result successful
                                            JsonRpcMessage::Response(JsonRpcResponse {
                                                jsonrpc: "2.0".to_string(),
                                                id,
                                                result: res_opt.unwrap_or(serde_json::Value::Null),
                                            })
                                        }
                                        Err(e) => {
                                            // Send Error
                                            use crate::mcp::types::ErrorData;
                                            use crate::mcp::types::JsonRpcError;
                                            JsonRpcMessage::Error(JsonRpcError {
                                                jsonrpc: "2.0".to_string(),
                                                id,
                                                error: ErrorData {
                                                    code: -32603, // Internal Error default?
                                                    message: e.to_string(),
                                                    data: None,
                                                },
                                            })
                                        }
                                    };

                                    if let Err(e) = transport.send(response).await {
                                        error!("Failed to send response: {}", e);
                                    }
                                });
                            }
                            JsonRpcMessage::Notification(notif) => {
                                debug!("Received notification: {}", notif.method);
                                // Currently just log, or dispatch to handlers?
                            }
                        }
                    }
                    Err(e) => {
                        error!("Transport receive error: {}. Stopping client loop.", e);
                        break;
                    }
                }
            }
        });
    }

    pub fn register_handler<F, Fut>(&self, method: &str, handler: F)
    where
        F: Fn(JsonRpcRequest) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<Option<Value>, FastMCPError>> + Send + 'static,
    {
        self.request_handlers.insert(
            method.to_string(),
            Box::new(move |req| Box::pin(handler(req))),
        );
    }

    pub fn register_sampling_handler<F, Fut>(&self, handler: F)
    where
        F: Fn(crate::mcp::types::CreateMessageRequestParams) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<
                Output = Result<crate::mcp::types::CreateMessageResult, FastMCPError>,
            > + Send
            + 'static,
    {
        let handler = Arc::new(handler);
        self.register_handler("sampling/createMessage", move |req| {
            let handler = handler.clone();
            Box::pin(async move {
                let params_val = req
                    .params
                    .ok_or(FastMCPError::InvalidRequest("Missing params".to_string()))?;
                let params: crate::mcp::types::CreateMessageRequestParams =
                    serde_json::from_value(params_val).map_err(FastMCPError::Json)?;

                let result = handler(params).await?;
                Ok(Some(
                    serde_json::to_value(result).map_err(FastMCPError::Json)?,
                ))
            })
        });
    }

    pub fn register_roots_list_handler<F, Fut>(&self, handler: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<crate::mcp::types::ListRootsResult, FastMCPError>>
            + Send
            + 'static,
    {
        let handler = Arc::new(handler);
        self.register_handler("roots/list", move |_req| {
            let handler = handler.clone();
            Box::pin(async move {
                let result = handler().await?;
                Ok(Some(
                    serde_json::to_value(result).map_err(FastMCPError::Json)?,
                ))
            })
        });
    }

    fn next_id(&self) -> RequestId {
        RequestId::Int(self.request_id_counter.fetch_add(1, Ordering::SeqCst))
    }

    pub async fn request(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, FastMCPError> {
        let id = self.next_id();
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: id.clone(),
            transport_metadata: None,
        };

        let (tx, rx) = oneshot::channel();
        self.pending_requests.insert(id, tx);

        self.transport.send(JsonRpcMessage::Request(req)).await?;

        // Wait for response with optional timeout
        let rx_outer_result = if let Some(duration) = self.timeout {
            match tokio::time::timeout(duration, rx).await {
                Ok(res) => {
                    res.map_err(|_| FastMCPError::InvalidRequest("Sender dropped".to_string()))
                }
                Err(_) => Err(FastMCPError::InvalidRequest(
                    "Request timed out".to_string(),
                )),
            }
        } else {
            rx.await
                .map_err(|_| FastMCPError::InvalidRequest("Sender dropped".to_string()))
        };

        match rx_outer_result {
            Ok(res_inner) => match res_inner {
                Ok(resp) => Ok(resp.result),
                Err(e) => Err(e),
            },
            Err(e) => Err(e),
        }
    }

    pub async fn list_tools(&self) -> Result<Vec<crate::mcp::types::Tool>, FastMCPError> {
        let result = self.request("tools/list", None).await?;
        let tools_val = result.get("tools").ok_or(FastMCPError::InvalidRequest(
            "Missing 'tools' field".to_string(),
        ))?;
        serde_json::from_value(tools_val.clone()).map_err(FastMCPError::Json)
    }

    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<crate::tools::tool::ToolResult, FastMCPError> {
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments
        });
        let result = self.request("tools/call", Some(params)).await?;
        serde_json::from_value(result).map_err(FastMCPError::Json)
    }

    pub async fn list_resources(&self) -> Result<Vec<crate::mcp::types::Resource>, FastMCPError> {
        let result = self.request("resources/list", None).await?;
        let res_val = result.get("resources").ok_or(FastMCPError::InvalidRequest(
            "Missing 'resources' field".to_string(),
        ))?;
        serde_json::from_value(res_val.clone()).map_err(FastMCPError::Json)
    }

    pub async fn read_resource(
        &self,
        uri: &str,
    ) -> Result<Vec<crate::mcp::types::ResourceContents>, FastMCPError> {
        let params = serde_json::json!({
            "uri": uri
        });
        let result = self.request("resources/read", Some(params)).await?;
        let contents_val = result.get("contents").ok_or(FastMCPError::InvalidRequest(
            "Missing 'contents' field".to_string(),
        ))?;
        serde_json::from_value(contents_val.clone()).map_err(FastMCPError::Json)
    }

    pub async fn list_prompts(&self) -> Result<Vec<crate::mcp::types::Prompt>, FastMCPError> {
        let result = self.request("prompts/list", None).await?;
        let prompts_val = result.get("prompts").ok_or(FastMCPError::InvalidRequest(
            "Missing 'prompts' field".to_string(),
        ))?;
        serde_json::from_value(prompts_val.clone()).map_err(FastMCPError::Json)
    }

    // Using Value for PromptResult if GetPromptResult is missing
    pub async fn get_prompt(
        &self,
        name: &str,
        arguments: Option<Value>,
    ) -> Result<Value, FastMCPError> {
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments.unwrap_or(serde_json::json!({}))
        });
        self.request("prompts/get", Some(params)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::core::FastMCPServer;

    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::Arc;

    use tokio::sync::Mutex;
    use tokio::sync::mpsc;

    #[derive(Debug)]
    struct LoopbackTransport {
        server: FastMCPServer,
        tx: mpsc::Sender<JsonRpcMessage>,
        rx: Mutex<mpsc::Receiver<JsonRpcMessage>>,
    }

    impl LoopbackTransport {
        fn new(server: FastMCPServer) -> Self {
            let (tx, rx) = mpsc::channel(100);
            Self {
                server,
                tx,
                rx: Mutex::new(rx),
            }
        }
    }

    #[async_trait]
    impl ClientTransport for LoopbackTransport {
        async fn send(&self, message: JsonRpcMessage) -> Result<(), FastMCPError> {
            if let JsonRpcMessage::Request(req) = message {
                // Logic: server.handle_request returns JsonRpcResponse.
                // Loopback needs to convert response back to JsonRpcMessage::Response and put in channel.
                match self.server.handle_request(req).await {
                    Ok(resp) => {
                        let _ = self.tx.send(JsonRpcMessage::Response(resp)).await;
                    }
                    Err(_e) => {
                        // If internal server error, send Error response?
                        // Or just log? Client expects a response if it sent a request.
                        // Construct error response.
                        // For test simplicity, assume success or simple error.
                        // But request ID is needed for error response.
                        // If handle_request fails at top level, we might not have ID?
                        // handle_request usually returns JsonRpcResponse even for errors if possible.
                        // FastMCPServer::handle_request returns Result<JsonRpcResponse, FastMCPError>.
                        // If Err, it means catastrophic failure or bad request structure where we can't reply?
                    }
                }
            }
            Ok(())
        }

        async fn receive(&self) -> Result<JsonRpcMessage, FastMCPError> {
            let mut rx = self.rx.lock().await;
            rx.recv().await.ok_or(FastMCPError::new("Channel closed"))
        }
    }

    #[tokio::test]
    async fn test_client_server_integration() {
        // Setup Server
        let server = FastMCPServer::new("test-server", "1.0");
        let tool = crate::tools::tool::Tool {
            name: "echo".to_string(),
            description: None,
            enabled: true,
            key: None,
            title: None,
            meta: None,
            tags: std::collections::HashSet::new(),
            data: crate::tools::tool::ToolKind::Function(crate::tools::tool::ToolFunction {
                name: "echo".to_string(),
                description: None,
                input_schema: json!({ "type": "object" }),
                output_schema: None,
                compiled_schema: None,
                fn_handler: Arc::new(Box::new(|_, args| {
                    Box::pin(async move {
                        Ok(crate::tools::tool::ToolResult {
                            content: vec![],
                            structured_content: Some(args),
                        })
                    })
                        as std::pin::Pin<
                            Box<
                                dyn std::future::Future<
                                        Output = Result<
                                            crate::tools::tool::ToolResult,
                                            FastMCPError,
                                        >,
                                    > + Send,
                            >,
                        >
                }) as crate::tools::tool::ToolHandler),
            }),
        };
        server.add_tool(tool).unwrap();

        // Setup Client
        let transport = Box::new(LoopbackTransport::new(server));
        let client = Client::new(transport);

        // Test list_tools
        let tools = client.list_tools().await.expect("Failed to list tools");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].base_metadata.name, "echo");

        // Test call_tool
        let result = client
            .call_tool("echo", json!({ "msg": "hello" }))
            .await
            .expect("Failed to call tool");
        let output = result.structured_content.unwrap();
        assert_eq!(output["msg"], "hello");
    }
}
