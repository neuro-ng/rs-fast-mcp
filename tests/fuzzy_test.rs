use rs_fast_mcp::mcp::types::Resource;
use rs_fast_mcp::server::core::FastMCP;
use rs_fast_mcp::tools::tool::Tool;
use serde_json::json;
use std::sync::Arc;

fn create_dummy_tool(name: &str) -> Tool {
    use rs_fast_mcp::tools::tool::{ToolFunction, ToolKind};
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
    use rs_fast_mcp::mcp::types::BaseMetadata;
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
async fn test_fuzzy_matching() {
    let server = FastMCP::new("test", "1.0");

    // Tools
    server.add_tool(create_dummy_tool("calculate_sum")).unwrap();

    // Call with typo
    let req = rs_fast_mcp::mcp::types::JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tools/call".to_string(),
        params: Some(json!({ "name": "calculate_sam", "arguments": {} })),
        id: rs_fast_mcp::mcp::types::RequestId::Int(1),
        transport_metadata: None,
    };

    let resp = server.handle_request(req).await;
    assert!(resp.is_err(), "Expected error for invalid tool name");
    let err = resp.err().unwrap();
    // Error is FastMCPError.
    // We need to convert it to string to check message?
    // FastMCPError display implementation should show the message.
    let err_msg = format!("{}", err);
    println!("Tool Error: {}", err_msg);
    assert!(err_msg.contains("Did you mean 'calculate_sum'?"));

    // Resources
    server
        .add_resource(create_dummy_resource("file:///config/main.json"), None)
        .unwrap();

    let req2 = rs_fast_mcp::mcp::types::JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "resources/read".to_string(),
        params: Some(json!({ "uri": "file:///config/main.gson" })),
        id: rs_fast_mcp::mcp::types::RequestId::Int(2),
        transport_metadata: None,
    };

    let resp2 = server.handle_request(req2).await;
    assert!(resp2.is_err(), "Expected error for invalid resource uri");
    let err2 = resp2.err().unwrap();
    let err_msg2 = format!("{}", err2);
    println!("Resource Error: {}", err_msg2);
    assert!(err_msg2.contains("Did you mean 'file:///config/main.json'?"));
}
