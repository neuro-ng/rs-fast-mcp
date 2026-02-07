use rs_fast_mcp::server::auth::{AuthMiddleware, SimpleAuthProvider};
use rs_fast_mcp::server::core::FastMCPServer;
use rs_fast_mcp::server::transport::Transport;
use rs_fast_mcp::server::transport::http::HttpTransport;
use rs_fast_mcp::tools::tool::{Tool, ToolFunction, ToolKind, ToolResult};

use serde_json::json;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[tokio::test]
async fn test_auth_propagation() {
    let server = FastMCPServer::new("auth-test", "1.0.0");

    // Add Auth Middleware
    let auth_provider = Arc::new(SimpleAuthProvider::new("secret-token"));
    server.add_middleware(AuthMiddleware::new(auth_provider));

    // Add a protected tool
    let tool = Tool {
        name: "secret_tool".to_string(),
        title: None,
        description: Some("Protected tool".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "secret_tool".to_string(),
            description: None,
            input_schema: json!({ "type": "object" }),
            output_schema: None,
            fn_handler: Arc::new(Box::new(|_ctx, _args| {
                Box::pin(async move {
                    Ok(ToolResult {
                        content: vec![],
                        structured_content: Some(json!({ "secret": "data" })),
                    })
                })
                    as Pin<
                        Box<
                            dyn Future<
                                    Output = Result<ToolResult, rs_fast_mcp::error::FastMCPError>,
                                > + Send,
                        >,
                    >
            })),
            compiled_schema: None,
        }),
    };
    server.add_tool(tool).unwrap();

    // Start Server
    let port = 9090;
    let transport = HttpTransport::new("127.0.0.1", port);
    let handler = Arc::new(server);

    // Run server in background
    let _handle = tokio::spawn(async move {
        transport.start(handler, None).await.unwrap();
    });

    // Give it a moment to start
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let client = reqwest::Client::new();

    // Test 1: No Auth -> Should Fail
    let resp = client
        .post(format!("http://127.0.0.1:{}/message", port))
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "secret_tool",
                "arguments": {}
            },
            "id": 1
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("error").is_some());

    // Test 2: With Header -> Should Succeed
    let resp = client
        .post(format!("http://127.0.0.1:{}/message", port))
        .header("Authorization", "Bearer secret-token")
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "secret_tool",
                "arguments": {}
            },
            "id": 2
        }))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    println!("Response Body: {:?}", body);
    assert!(
        body.get("result").is_some(),
        "Expected result, got error: {:?}",
        body
    );
    assert_eq!(body["result"]["structured_content"]["secret"], "data");
}
