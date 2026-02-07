use rs_fast_mcp::server::core::FastMCP;
use serde_json::json;

#[tokio::test]
async fn test_resource_subscription() {
    let server = FastMCP::new("test", "1.0");
    
    // Subscribe
    let req = rs_fast_mcp::mcp::types::JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "resources/subscribe".to_string(),
        params: Some(json!({ "uri": "file:///test" })),
        id: rs_fast_mcp::mcp::types::RequestId::Int(1),
        transport_metadata: None,
    };

    let resp = server.handle_request(req).await.unwrap();
    assert!(resp.result.is_null()); // Should return null on success

    // Unsubscribe
    let req2 = rs_fast_mcp::mcp::types::JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "resources/unsubscribe".to_string(),
        params: Some(json!({ "uri": "file:///test" })),
        id: rs_fast_mcp::mcp::types::RequestId::Int(2),
        transport_metadata: None,
    };

    let resp2 = server.handle_request(req2).await.unwrap();
    assert!(resp2.result.is_null());
}
