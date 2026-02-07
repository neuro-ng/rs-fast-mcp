use rs_fast_mcp::mcp::types::JsonRpcRequest;
use rs_fast_mcp::mcp::types::{RequestId, Tool as McpTool};
use rs_fast_mcp::server::core::FastMCPServer;
use rs_fast_mcp::tools::tool::{Tool, ToolFunction, ToolKind, ToolResult};

use rs_fast_mcp::error::FastMCPError;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

async fn list_tools(server: &FastMCPServer) -> Vec<McpTool> {
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tools/list".to_string(),
        params: None,
        id: RequestId::Int(1),
        transport_metadata: None,
    };
    let resp = server.handle_request(req).await.unwrap();
    let result: serde_json::Value = resp.result;
    let tools_val = result.get("tools").unwrap();
    serde_json::from_value(tools_val.clone()).unwrap()
}

fn create_tool(name: &str, tags: Vec<&str>) -> Tool {
    Tool {
        name: name.to_string(),
        description: Some(format!("Description for {}", name)),
        enabled: true,
        key: None,
        title: None,
        meta: None,
        tags: tags.into_iter().map(|s| s.to_string()).collect(),
        data: ToolKind::Function(ToolFunction {
            name: name.to_string(),
            description: None,
            input_schema: serde_json::json!({}),
            output_schema: None,
            fn_handler: Arc::new(Box::new(|_ctx, _args| {
                Box::pin(async move {
                    Ok(ToolResult {
                        content: vec![],
                        structured_content: None,
                    })
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
            compiled_schema: None,
        }),
    }
}

use rs_fast_mcp::server::builder::ServerBuilder;

#[tokio::test]
async fn test_filtering_include() {
    let server_app = ServerBuilder::new("test", "1.0")
        .include_tags(vec!["public".to_string()])
        .build();
    let server = server_app.core;

    // Tools
    let tool1 = create_tool("tool1", vec!["public"]);
    let tool2 = create_tool("tool2", vec!["internal"]);
    let tool3 = create_tool("tool3", vec!["public", "internal"]);

    server.add_tool(tool1).unwrap();
    server.add_tool(tool2).unwrap();
    server.add_tool(tool3).unwrap();

    let tools = list_tools(&server).await;
    let names: Vec<String> = tools.iter().map(|t| t.base_metadata.name.clone()).collect();

    // tool1: public -> keep
    // tool2: internal -> drop
    // tool3: public, internal -> keep (has public)

    assert!(names.contains(&"tool1".to_string()));
    assert!(!names.contains(&"tool2".to_string()));
    assert!(names.contains(&"tool3".to_string()));
    assert_eq!(names.len(), 2);
}

#[tokio::test]
async fn test_filtering_exclude() {
    let server_app = ServerBuilder::new("test", "1.0")
        .exclude_tags(vec!["secret".to_string()])
        .build();
    let server = server_app.core;

    let tool1 = create_tool("tool1", vec!["secret"]);
    let tool2 = create_tool("tool2", vec![]);
    let tool3 = create_tool("tool3", vec!["secret", "other"]);

    server.add_tool(tool1).unwrap();
    server.add_tool(tool2).unwrap();
    server.add_tool(tool3).unwrap();

    let tools = list_tools(&server).await;
    let names: Vec<String> = tools.iter().map(|t| t.base_metadata.name.clone()).collect();

    // tool1: secret -> drop
    // tool2: no tags -> keep (exclude logic: drop if contains secret)
    // tool3: secret, other -> drop

    assert!(!names.contains(&"tool1".to_string()));
    assert!(names.contains(&"tool2".to_string()));
    assert!(!names.contains(&"tool3".to_string()));
    assert_eq!(names.len(), 1);
}
