use rs_fast_mcp::mcp::types::ResourceTemplate;
use rs_fast_mcp::server::core::FastMCP;
use serde_json::json;
use std::sync::Arc;

fn create_dummy_template(uri_template: &str, name: &str) -> ResourceTemplate {
    ResourceTemplate {
        uri_template: uri_template.to_string(),
        name: name.to_string(),
        description: None,
        mime_type: None,
        annotations: None,
    }
}

#[tokio::test]
async fn test_resource_template() {
    let server = FastMCP::new("test", "1.0");

    // Register template: file:///{path}
    let template = create_dummy_template("file:///{path}", "file_template");

    // Handler that echoes the 'path' argument
    let handler = Arc::new(
        Box::new(|_, context: rs_fast_mcp::server::context::Context| {
            Box::pin(async move {
                let path = context.arguments.get("path").cloned().unwrap_or_default();
                Ok(rs_fast_mcp::resources::types::ResourceResult::from_text(
                    format!("Content of {}", path),
                    Some("text/plain".to_string()),
                ))
            })
                as std::pin::Pin<
                    Box<
                        dyn std::future::Future<
                                Output = Result<
                                    rs_fast_mcp::resources::types::ResourceResult,
                                    rs_fast_mcp::error::FastMCPError,
                                >,
                            > + Send,
                    >,
                >
        }) as rs_fast_mcp::resources::manager::ResourceReadHandler,
    );

    server.add_resource_template(template, handler).unwrap();

    // Request a matching URI: file:///foo/bar.txt
    let req = rs_fast_mcp::mcp::types::JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "resources/read".to_string(),
        params: Some(json!({ "uri": "file:///foo/bar.txt" })),
        id: rs_fast_mcp::mcp::types::RequestId::Int(1),
        transport_metadata: None,
    };

    let resp = server.handle_request(req).await.unwrap();
    let result = resp.result;

    println!("Result: {:?}", result);

    assert_eq!(result["contents"][0]["text"], "Content of foo/bar.txt");
    // URI is now injected from the request URI rather than constructed by the handler
    assert_eq!(result["contents"][0]["uri"], "file:///foo/bar.txt");

    // Test templates/list
    let list_req = rs_fast_mcp::mcp::types::JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "resources/templates/list".to_string(),
        params: None,
        id: rs_fast_mcp::mcp::types::RequestId::Int(2),
        transport_metadata: None,
    };
    let list_resp = server.handle_request(list_req).await.unwrap();
    let list_result = list_resp.result;
    assert!(
        !list_result["resourceTemplates"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(list_result["resourceTemplates"][0]["name"], "file_template");
}
