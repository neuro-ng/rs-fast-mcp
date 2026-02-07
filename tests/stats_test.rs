use rs_fast_mcp::mcp::types::{BaseMetadata, Resource};
use rs_fast_mcp::server::core::FastMCP;
use rs_fast_mcp::tools::tool::{Tool, ToolFunction, ToolKind};
use serde_json::json;
use std::sync::Arc;

fn create_dummy_tool(name: &str) -> Tool {
    Tool {
        name: name.to_string(),
        description: None,
        data: ToolKind::Function(ToolFunction {
            name: name.to_string(),
            description: None,
            input_schema: json!({}),
            output_schema: None,
            fn_handler: Arc::new(Box::new(|_, _| {
                Box::pin(async {
                    Ok(rs_fast_mcp::tools::tool::ToolResult {
                        content: vec![],
                        structured_content: None,
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

fn create_dummy_resource(uri: &str) -> Resource {
    Resource {
        uri: uri.to_string(),
        base_metadata: BaseMetadata {
            name: "test".to_string(),
            title: None,
        },
        description: None,
        mime_type: None,
        annotations: None,
        size: None,
        icons: None,
        tags: None,
    }
}

#[tokio::test]
async fn test_tool_stats() {
    let server = FastMCP::new("test", "1.0");
    server.add_tool(create_dummy_tool("tool1")).unwrap();

    assert_eq!(server.get_tool_usage("tool1"), Some(0));

    let req = rs_fast_mcp::mcp::types::JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tools/call".to_string(),
        params: Some(json!({ "name": "tool1", "arguments": {} })),
        id: rs_fast_mcp::mcp::types::RequestId::Int(1),
        transport_metadata: None,
    };

    server.handle_request(req).await.unwrap();

    assert_eq!(server.get_tool_usage("tool1"), Some(1));
}

#[tokio::test]
async fn test_resource_stats() {
    let server = FastMCP::new("test", "1.0");
    let uri = "file:///test";

    // We need a handler for read to succeed
    let handler = Arc::new(Box::new(|_, _| {
        Box::pin(async { Ok(vec![]) })
            as std::pin::Pin<
                Box<
                    dyn std::future::Future<
                            Output = Result<
                                Vec<rs_fast_mcp::mcp::types::ResourceContents>,
                                rs_fast_mcp::error::FastMCPError,
                            >,
                        > + Send,
                >,
            >
    })
        as rs_fast_mcp::resources::manager::ResourceReadHandler);

    server
        .add_resource(create_dummy_resource(uri), Some(handler))
        .unwrap();

    assert_eq!(server.get_resource_usage(uri), Some(0));

    let req = rs_fast_mcp::mcp::types::JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "resources/read".to_string(),
        params: Some(json!({ "uri": uri })),
        id: rs_fast_mcp::mcp::types::RequestId::Int(1),
        transport_metadata: None,
    };

    server.handle_request(req).await.unwrap();

    assert_eq!(server.get_resource_usage(uri), Some(1));
}
