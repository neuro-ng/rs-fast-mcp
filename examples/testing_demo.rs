use rs_fast_mcp::error::FastMCPError;
// use rs_fast_mcp::mcp::types::ResourceTemplate;
use rs_fast_mcp::mcp::types::{BaseMetadata, Resource};
use rs_fast_mcp::mcp::types::{ContentBlock, TextContent};
use rs_fast_mcp::prompts::prompt::{Prompt, PromptArgument, PromptFunction};
use rs_fast_mcp::prompts::types::{Message, PromptResult};
use rs_fast_mcp::resources::types::ResourceResult;
use rs_fast_mcp::server::core::FastMCPServer;
use rs_fast_mcp::server::transport::Transport;
use rs_fast_mcp::server::transport::stdio::StdioTransport;
use rs_fast_mcp::tools::tool::{Tool, ToolFunction, ToolKind, ToolResult};
use serde_json::json;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

fn create_server() -> FastMCPServer {
    let server = FastMCPServer::new("testing_demo", "1.0");

    // --- Tool: add ---
    let add_tool = Tool {
        name: "add".to_string(),
        title: None,
        description: Some("Add two numbers".to_string()),
        enabled: true,
        key: None,
        tags: Default::default(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "add".to_string(),
            description: None,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "a": { "type": "integer" },
                    "b": { "type": "integer" }
                },
                "required": ["a", "b"]
            }),
            output_schema: None,
            compiled_schema: None,
            fn_handler: Arc::new(Box::new(|_ctx, args| {
                Box::pin(async move {
                    let a = args.get("a").and_then(|v| v.as_i64()).unwrap_or(0);
                    let b = args.get("b").and_then(|v| v.as_i64()).unwrap_or(0);
                    let sum = a + b;
                    Ok(ToolResult {
                        content: vec![ContentBlock::Text(TextContent {
                            type_: "text".to_string(),
                            text: sum.to_string(),
                            annotations: None,
                        })],
                        structured_content: None,
                    })
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
        }),
    };
    server.add_tool(add_tool).unwrap();

    // --- Resource: demo://info ---
    let resource = Resource {
        uri: "demo://info".to_string(),
        base_metadata: BaseMetadata {
            name: "info".to_string(),
            title: None,
        },
        description: Some("Server info".to_string()),
        mime_type: Some("text/plain".to_string()),
        annotations: None,
        size: None,
        icons: None,
        tags: None,
    };
    server
        .add_resource(
            resource,
            Some(Arc::new(Box::new(|_uri, _ctx| {
                Box::pin(async move {
                    Ok(ResourceResult::from_text(
                        "This is the FastMCP Testing Demo server".to_string(),
                        Some("text/plain".to_string()),
                    ))
                })
                    as Pin<
                        Box<
                            dyn Future<Output = Result<ResourceResult, FastMCPError>> + Send,
                        >,
                    >
            }))),
        )
        .unwrap();

    // --- Prompt: greet ---
    let prompt = Prompt {
        name: "greet".to_string(),
        title: None,
        description: Some("Generate a greeting".to_string()),
        enabled: true,
        key: None,
        tags: Default::default(),
        meta: None,
        data: PromptFunction {
            name: "greet".to_string(),
            description: None,
            arguments: Some(vec![PromptArgument {
                name: "name".to_string(),
                description: Some("Name to greet".to_string()),
                required: Some(false),
            }]),
            fn_handler: Arc::new(Box::new(|args| {
                Box::pin(async move {
                    let name = args
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Friend")
                        .to_string();
                    Ok(PromptResult::new(vec![Message::user(format!(
                        "Hello, {}!",
                        name
                    ))]))
                })
                    as Pin<
                        Box<dyn Future<Output = Result<PromptResult, FastMCPError>> + Send>,
                    >
            })),
        },
    };
    server.add_prompt(prompt).unwrap();

    server
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = create_server();
    let transport = StdioTransport::new();
    let handler = Arc::new(server.clone());
    let rx = Some(server.subscribe_notifications());
    transport.start(handler, rx).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs_fast_mcp::server::context::Context;

    #[tokio::test]
    async fn test_tool_execution() {
        let server = create_server();
        // Direct invocation (bypassing transport)
        let context = Context::default();
        let args = json!({ "a": 10, "b": 32 });

        // We use the `tools` manager directly implicitly via `server.tools` if exposed,
        // OR we simulate a JSON-RPC request if we want to test that layer.
        // But `FastMCPServer` exposes `tools` capability via internal Arc?
        // `FastMCPServer` struct wraps `Arc<FastMCP>`.
        // We need `server.get_tool("add")`? No, we want to EXECUTE it.
        // `FastMCP` (internal) has `tools.call_tool`.
        // But `FastMCPServer` wrapper currently doesn't expose `call_tool`.
        // Let's check `src/server/core.rs`. `handle_request` calls `tools.call_tool`.
        // Ideally checking `examples/demo_server.ml` (OCaml), it calls `FastMCP.call_tool`.
        // We should ensure `FastMCPServer` exposes `call_tool` or effectively test via `handle_request`.
        // Using `handle_request` is "integration testing".
        // Let's use `handle_request` to verify the full stack except transport.

        let req = rs_fast_mcp::mcp::types::JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "add",
                "arguments": args
            })),
            id: rs_fast_mcp::mcp::types::RequestId::Int(1),
            transport_metadata: None,
        };

        // We can call `handle_request` directly!
        let handler = server; // It implements RequestHandler
        use rs_fast_mcp::server::transport::RequestHandler;

        let resp = handler.handle_request(req).await.expect("Request failed");

        // Check result
        let result_json = resp.result;
        // Expected: { "content": [{ "type": "text", "text": "42" }] }
        // Note: call_tool returns ToolResult, handle_request wraps it.
        // Wait, handle_request returns `JsonRpcResponse { result: Value }`.

        println!("Result: {}", result_json);
        assert!(result_json.to_string().contains("42"));
    }

    #[tokio::test]
    async fn test_resource_read() {
        let server = create_server();
        use rs_fast_mcp::server::transport::RequestHandler;

        let req = rs_fast_mcp::mcp::types::JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "resources/read".to_string(),
            params: Some(json!({ "uri": "demo://info" })),
            id: rs_fast_mcp::mcp::types::RequestId::Int(2),
            transport_metadata: None,
        };

        let resp = server.handle_request(req).await.expect("Request failed");
        let result_json = resp.result;
        assert!(result_json.to_string().contains("Testing Demo server"));
    }
}
