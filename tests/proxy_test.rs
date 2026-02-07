use async_trait::async_trait;
use rs_fast_mcp::client::ClientTransport;
use rs_fast_mcp::mcp::types::{JsonRpcMessage, JsonRpcRequest, RequestId}; // Import from types
use rs_fast_mcp::server::core::FastMCPServer;
use rs_fast_mcp::server::proxy::MountedServer;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

#[derive(Debug)]
struct MockTransport {
    server: FastMCPServer,
    tx: mpsc::Sender<JsonRpcMessage>,
    rx: Mutex<mpsc::Receiver<JsonRpcMessage>>,
}

impl MockTransport {
    fn new(server: FastMCPServer) -> (Self, mpsc::Sender<JsonRpcMessage>) {
        let (tx, rx) = mpsc::channel(100);
        (
            Self {
                server,
                tx: tx.clone(),
                rx: Mutex::new(rx),
            },
            tx,
        )
    }
}

#[async_trait]
impl ClientTransport for MockTransport {
    async fn send(&self, message: JsonRpcMessage) -> Result<(), rs_fast_mcp::error::FastMCPError> {
        match message {
            JsonRpcMessage::Request(req) => {
                match self.server.handle_request(req).await {
                    Ok(resp) => {
                        let _ = self.tx.send(JsonRpcMessage::Response(resp)).await;
                    }
                    Err(_) => {
                        // ignore
                    }
                }
            }
            JsonRpcMessage::Response(resp) => {
                // Client sending response to server (e.g. Sampling Result)
                // We don't have a "Server" loop here handling responses.
                // But for test verification, we want to capture this response!
                // Wait, this transport connects Client -> MockServer.
                // Ideally MockServer should handle it.
                // But `FastMCPServer` logic is mainly request handling.
                // It doesn't have a "pending requests" map for *outgoing* server requests necessarily (unless we use Client inside server?).
                // For now, we can perhaps just log it or ignore?
                // Testing logic:
                // 1. Inject Request (Sampling).
                // 2. Client processes.
                // 3. Client sends Response.
                // 4. `send` is called with Response.
                // We should capture this response to verify it!
                // Let's repurpose `tx`? No `tx` feeds into Client's input.
                // If we push response into `tx`, Client receives its own response as if it was a message FROM server?
                // No. Client handles `Response` by looking up ID in pending_requests.
                // If Client *sent* a Response (to a Sampling request), it expects Server to receive it.
                // Server is `self.server`. `FastMCPServer` doesn't expose a method to "complete" a sampling request.
                // But we can capture it in a side channel?
                // Or just print it?
                // For verification, I'll print it.
                // Or better: MockTransport should have an `outgoing_rx` for test to inspect?
                println!("Client sent response: {:?}", resp);
            }
            _ => {}
        }
        Ok(())
    }

    async fn receive(&self) -> Result<JsonRpcMessage, rs_fast_mcp::error::FastMCPError> {
        let mut rx = self.rx.lock().await;
        rx.recv()
            .await
            .ok_or(rs_fast_mcp::error::FastMCPError::new("Channel closed"))
    }
}

#[tokio::test]
async fn test_proxy_tool_call() {
    // 1. Create "Remote" Server
    let remote_server = FastMCPServer::new("remote", "1.0");
    use rs_fast_mcp::tools::tool::{Tool, ToolFunction, ToolKind};

    let echo_tool = Tool {
        name: "echo".to_string(),
        description: None,
        enabled: true,
        key: None,
        title: None,
        meta: None,
        tags: std::collections::HashSet::new(),
        data: ToolKind::Function(ToolFunction {
            name: "echo".to_string(), // Must match tool name usually
            description: None,
            input_schema: json!({ "type": "object" }),
            output_schema: None,
            compiled_schema: None,
            fn_handler: Arc::new(Box::new(|_, args| {
                Box::pin(async move {
                    Ok(rs_fast_mcp::tools::tool::ToolResult {
                        content: vec![],
                        structured_content: Some(args),
                    })
                })
                    as std::pin::Pin<
                        Box<
                            dyn std::future::Future<
                                    Output = Result<
                                        rs_fast_mcp::tools::tool::ToolResult,
                                        rs_fast_mcp::error::FastMCPError,
                                    >,
                                > + Send,
                        >,
                    >
            }) as rs_fast_mcp::tools::tool::ToolHandler),
        }),
    };
    remote_server.add_tool(echo_tool).unwrap();

    // 2. Create "Host" Server
    let host_server = rs_fast_mcp::server::core::FastMCP::new("host", "1.0");

    // 3. Mount Remote -> Host via MockTransport
    let (transport_impl, _tx) = MockTransport::new(remote_server);
    let transport = Box::new(transport_impl);
    let mounted = MountedServer::new(transport, "remote");

    // Mount (syncs tools)
    mounted.mount(&host_server).await.expect("Failed to mount");

    // 4. Verify Tool was synced
    let tools = host_server.list_tools();
    assert_eq!(tools.len(), 1);
    let proxy_tool = &tools[0];
    assert_eq!(proxy_tool.name, "remote_echo");

    // 5. Call Proxy Tool using handle_request
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "remote_echo",
            "arguments": { "msg": "proxy_works" }
        })),
        id: RequestId::Int(1),
        transport_metadata: None,
    };

    let resp = host_server
        .handle_request(req)
        .await
        .expect("Request failed");
    let result = resp.result;
    // Verify results
    let content = result
        .get("structured_content")
        .expect("Expected structured_content");
    assert_eq!(content["msg"], "proxy_works");
}

#[tokio::test]
async fn test_client_sampling_handler() {
    use rs_fast_mcp::client::Client;
    use rs_fast_mcp::mcp::types::{
        CreateMessageResult, JsonRpcMessage, JsonRpcRequest, RequestId, Role,
        SamplingMessageContent, TextContent,
    };
    use rs_fast_mcp::server::core::FastMCPServer;
    use std::sync::Arc;
    use tokio::sync::Notify;

    // 1. Setup Mock Server (dummy)
    let remote_server = FastMCPServer::new("remote", "1.0");

    // 2. Setup Client
    let (transport_impl, injector) = MockTransport::new(remote_server);
    let client = Client::new(Box::new(transport_impl));

    // Side channel to verify execution
    let processed = Arc::new(Notify::new());
    let processed_clone = processed.clone();

    // 3. Register Sampling Handler
    client.register_sampling_handler(move |_params| {
        let processed = processed_clone.clone();
        async move {
            // Echo back "Approved"
            processed.notify_one();
            Ok(CreateMessageResult {
                role: Role::Assistant,
                content: SamplingMessageContent::Text(TextContent {
                    type_: "text".to_string(),
                    text: "Approved".to_string(),
                    annotations: None,
                }),
                model: "test-model".to_string(),
                stop_reason: None,
            })
        }
    });

    // 4. Inject Sampling Request (Server -> Client)
    let req_id = RequestId::String("sample-1".to_string());
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: req_id.clone(),
        method: "sampling/createMessage".to_string(),
        params: Some(serde_json::json!({
            "messages": [],
            "maxTokens": 100
        })),
        transport_metadata: None,
    };

    injector
        .send(JsonRpcMessage::Request(req))
        .await
        .expect("Failed to inject");

    // 5. Verify Execution
    // Wait for notification
    let result =
        tokio::time::timeout(std::time::Duration::from_secs(2), processed.notified()).await;
    assert!(result.is_ok(), "Sampling handler was not called");
}
