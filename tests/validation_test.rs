use rs_fast_mcp::mcp::types::{JsonRpcRequest, RequestId};
use rs_fast_mcp::server::core::FastMCP;
use rs_fast_mcp::tools::tool::{Tool, ToolFunction, ToolKind};
use serde_json::json;
use std::sync::Arc;

fn create_validated_tool(name: &str) -> Tool {
    Tool {
        name: name.to_string(),
        description: None,
        data: ToolKind::Function(ToolFunction {
            name: name.to_string(),
            description: None,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "age": { "type": "integer", "minimum": 0 },
                    "name": { "type": "string" }
                },
                "required": ["name", "age"]
            }),
            output_schema: None,
            fn_handler: Arc::new(Box::new(|_, _| {
                Box::pin(async {
                    Ok(rs_fast_mcp::tools::tool::ToolResult {
                        content: vec![],
                        structured_content: Some(json!({"status": "ok"})),
                    })
                })
            })),
            compiled_schema: None,
        }),
        enabled: true,
        key: None,
        title: None,
        meta: None,
        tags: std::collections::HashSet::new(),
    }
}

#[tokio::test]
async fn test_tool_validation_success() {
    let server = FastMCP::new("test", "1.0");
    server.add_tool(create_validated_tool("test_tool")).unwrap();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "test_tool",
            "arguments": { "name": "Alice", "age": 30 }
        })),
        id: RequestId::Int(1),
        transport_metadata: None,
    };

    let resp = server.handle_request(req).await.unwrap();
    assert!(resp.result.get("structured_content").is_some());
}

#[tokio::test]
async fn test_tool_validation_failure_type() {
    let server = FastMCP::new("test", "1.0");
    server.add_tool(create_validated_tool("test_tool")).unwrap();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "test_tool",
            "arguments": { "name": "Alice", "age": "thirty" } // Invalid type
        })),
        id: RequestId::Int(2),
        transport_metadata: None,
    };

    let resp = server.handle_request(req).await;
    assert!(resp.is_err());
    let err = resp.err().unwrap();
    let msg = format!("{}", err);
    println!("Error message: {}", msg);
    assert!(msg.contains("Invalid arguments"));
    assert!(msg.contains("age"));
}

#[tokio::test]
async fn test_tool_validation_failure_required() {
    let server = FastMCP::new("test", "1.0");
    server.add_tool(create_validated_tool("test_tool")).unwrap();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tools/call".to_string(),
        params: Some(json!({
            "name": "test_tool",
            "arguments": { "name": "Alice" } // Missing age
        })),
        id: RequestId::Int(3),
        transport_metadata: None,
    };

    let resp = server.handle_request(req).await;
    assert!(resp.is_err());
    let msg = format!("{}", resp.err().unwrap());
    assert!(msg.contains("Invalid arguments"));
    assert!(msg.contains("age"));
}

#[tokio::test]
async fn test_prompt_validation_missing_arg() {
    use rs_fast_mcp::prompts::prompt::{Prompt, PromptArgument, PromptFunction};

    let server = FastMCP::new("test", "1.0");

    let prompt = Prompt {
        name: "test_prompt".to_string(),
        description: None,
        data: PromptFunction {
            name: "test_prompt".to_string(),
            description: None,
            arguments: Some(vec![PromptArgument {
                name: "topic".to_string(),
                description: None,
                required: Some(true),
            }]),
            fn_handler: Arc::new(Box::new(|_| {
                Box::pin(async {
                    Ok(rs_fast_mcp::prompts::types::PromptResult::new(vec![]))
                })
            })),
        },
        enabled: true,
        key: None,
        title: None,
        meta: None,
        tags: std::collections::HashSet::new(),
    };
    server.add_tool(create_validated_tool("dummy")).unwrap(); // Just to keep server happy? No need.
    server.add_prompt(prompt).unwrap();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "prompts/get".to_string(),
        params: Some(json!({
            "name": "test_prompt",
            "arguments": {}
        })),
        id: RequestId::Int(4),
        transport_metadata: None,
    };

    let resp = server.handle_request(req).await;
    assert!(resp.is_err());
    let msg = format!("{}", resp.err().unwrap());
    assert!(msg.contains("Missing required argument"));
    assert!(msg.contains("topic"));
}

#[tokio::test]
async fn test_resource_validation_invalid_uri() {
    let server = FastMCP::new("test", "1.0");
    // No need to add resource, validation happens before lookup potentially?
    // Actually, checking logic: 1. Validate URI. 2. Lookup.
    // So even if resource doesn't exist, invalid URI should trigger InvalidRequest("Invalid URI..."), NOT "Resource not found".

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "resources/read".to_string(),
        params: Some(json!({
            "uri": "not_a_valid_uri"
        })),
        id: RequestId::Int(5),
        transport_metadata: None,
    };

    let resp = server.handle_request(req).await;
    assert!(resp.is_err());
    let msg = format!("{}", resp.err().unwrap());
    println!("Resource error: {}", msg);
    assert!(msg.contains("Invalid URI"));
}
