use rs_fast_mcp::client::transport::stdio::StdioClientTransport;
use rs_fast_mcp::mcp::types::{JsonRpcRequest, JsonRpcResponse, RequestId};
use rs_fast_mcp::server::core::FastMCP;
use rs_fast_mcp::server::proxy::MountedServer;

#[tokio::test]
async fn test_server_mounting() {
    // 1. Path to simple_server binary
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let server_path =
        std::path::Path::new(&manifest_dir).join("target/debug/examples/simple_server");

    if !server_path.exists() {
        eprintln!(
            "simple_server binary not found at {:?}. Run `cargo build --examples` first.",
            server_path
        );
    }

    // 2. Create StdioTransport
    let transport = StdioClientTransport::new_process(server_path.to_str().unwrap(), &[])
        .expect("Failed to create transport");

    // 3. Create Host Server
    let host = FastMCP::new("host-server", "1.0");

    // 4. Create MountedServer and Mount
    // Prefix "remote"
    let mounted = MountedServer::new(Box::new(transport), "remote");
    mounted.mount(&host).await.expect("Failed to mount server");

    // 5. Verify Tools
    let tools = host.list_tools();
    println!(
        "Mounted tools: {:?}",
        tools.iter().map(|t| &t.name).collect::<Vec<_>>()
    );

    // Check for prefixed names
    let echo_tool = host.get_tool("remote_echo");
    assert!(
        echo_tool.is_some(),
        "Tool remote_echo not found. Available: {:?}",
        tools.iter().map(|t| &t.name).collect::<Vec<_>>()
    );

    // 6. Call Tool via Request
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: RequestId::String("1".to_string()),
        method: "tools/call".to_string(),
        params: Some(serde_json::json!({
            "name": "remote_echo",
            "arguments": {
                "message": "Hello Mounted World"
            }
        })),
        transport_metadata: None,
    };

    let resp: JsonRpcResponse = host.handle_request(req).await.expect("Request failed");

    // Parse result
    let result = resp.result;
    // Verify structure matches ToolResult
    println!(
        "Response: {}",
        serde_json::to_string_pretty(&result).unwrap()
    );

    let structured = result
        .get("structured_content")
        .expect("Missing structured_content");
    let echo = structured.get("echo").unwrap().as_str().unwrap();

    assert!(echo.contains("Hello Mounted World"));

    // 7. Verify Resources
    let resources = host.list_resources();
    println!(
        "Mounted resources: {:?}",
        resources.iter().map(|r| &r.uri).collect::<Vec<_>>()
    );

    assert!(!resources.is_empty(), "No resources found");
    assert!(
        resources.iter().any(|r| r.uri == "file:///hello"),
        "Resource file:///hello not found"
    );

    // 8. Read Resource
    let read_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: RequestId::String("2".to_string()),
        method: "resources/read".to_string(),
        params: Some(serde_json::json!({
            "uri": "file:///hello"
        })),
        transport_metadata: None,
    };

    let read_resp: JsonRpcResponse = host
        .handle_request(read_req)
        .await
        .expect("Read Request failed");
    if let Some(_err) = read_resp.result.get("error") { // Wait, JsonRpcResponse doesn't have error field in result. Logic error.
        // JsonRpcResponse is { jsonrpc, id, result } (successful) or we might have handled it via Result?
        // My handle_request returns Result<JsonRpcResponse, FastMCPError>.
        // If Protocol Error (like MethodNotFound), it might be inside Result.
        // But if User Error (ResourceNotFound), it might be returned as successful JSON-RPC response with error object?
        // FastMCP::handle_request returns JsonRpcResponse for "success" (even if logic error?) or Err for protocol error?
        // Looking at `server/core.rs`, errors are returned as `Err(FastMCPError)`.
        // Wait, JSON-RPC errors are usually 200 OK with error field?
        // `handle_request` returns `Result<JsonRpcResponse, FastMCPError>`.
        // `JsonRpcResponse` struct has `result: Value`.
        // Does `JsonRpcResponse` support error? `mcp/types.rs` defines `JsonRpcError` separately.
        // `FastMCP::handle_request` returns `Result<JsonRpcResponse, ...>`.
        // If it returns Ok(resp), resp.result is the result.
        // If it returns Err(e), the caller (transport) converts it to JsonRpcError.
        // So here `read_resp` implies Success.
    }

    let read_res_content = read_resp.result;
    println!(
        "Read Resource: {}",
        serde_json::to_string_pretty(&read_res_content).unwrap()
    );

    let contents = read_res_content
        .get("contents")
        .expect("Missing contents")
        .as_array()
        .expect("Contents not array");
    let text = contents[0].get("text").unwrap().as_str().unwrap();
    assert!(text.contains("Hello from simple server"));

    // 9. Verify Prompts
    let prompts = host.list_prompts();
    println!(
        "Mounted prompts: {:?}",
        prompts.iter().map(|p| &p.name).collect::<Vec<_>>()
    );

    // Expect "remote_greet"
    assert!(
        prompts.iter().any(|p| p.name == "remote_greet"),
        "Prompt remote_greet not found"
    );

    // 10. Get Prompt
    let prompt = host
        .get_prompt("remote_greet")
        .expect("Failed to get prompt");
    assert_eq!(prompt.name, "remote_greet");

    // 11. Execute Prompt
    let prompt_req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: RequestId::String("3".to_string()),
        method: "prompts/get".to_string(),
        params: Some(serde_json::json!({
            "name": "remote_greet",
            "arguments": {
                "name": "Mounted User"
            }
        })),
        transport_metadata: None,
    };

    let prompt_resp: JsonRpcResponse = host
        .handle_request(prompt_req)
        .await
        .expect("Prompt Request failed");
    println!(
        "Prompt Exec Response: {}",
        serde_json::to_string_pretty(&prompt_resp.result).unwrap()
    );

    let messages = prompt_resp
        .result
        .get("messages")
        .expect("Missing messages")
        .as_array()
        .expect("Messages not array");
    let content = messages[0].get("content").expect("Missing content");
    // content is a ContentBlock (tagged union in types, but here JSON Value).
    // In simple_server, it sends ContentBlock::Text.
    // { "type": "text", "text": "..." }
    let text = content
        .get("text")
        .expect("Missing text")
        .as_str()
        .expect("Text not string");

    assert_eq!(text, "Hello, Mounted User!");
}
