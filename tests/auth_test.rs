use rs_fast_mcp::mcp::types::{JsonRpcRequest, RequestId};
use rs_fast_mcp::server::builder::ServerBuilder;
use serde_json::json;

#[tokio::test]
async fn test_simple_auth_success() {
    let server_app = ServerBuilder::new("auth-test", "1.0")
        .with_simple_auth("secret123")
        .build(); // App is Server

    // Accessing core from Server is hard if fields are private on Server.
    // Server has `core` field which is `FastMCPServer`.
    // In `tests/filtering_test.rs` I made `core` public.
    // So I can use `server_app.core`.

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "ping".to_string(), // Assume ping exists or just any method
        params: Some(json!({
            "token": "secret123"
        })),
        id: RequestId::Int(1),
        transport_metadata: None,
    };

    let result = server_app.core.handle_request(req).await;
    assert!(result.is_ok(), "Request with correct token should succeed");
}

#[tokio::test]
async fn test_simple_auth_failure() {
    let server_app = ServerBuilder::new("auth-test-fail", "1.0")
        .with_simple_auth("secret123")
        .build();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "ping".to_string(),
        params: Some(json!({
            "token": "wrong"
        })),
        id: RequestId::Int(2),
        transport_metadata: None,
    };

    let result = server_app.core.handle_request(req).await;
    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("Unauthorized"),
                "Expected Unauthorized error, got: {}",
                msg
            );
        }
        Ok(_) => panic!("Request with wrong token should fail"),
    }
}

#[tokio::test]
async fn test_simple_auth_missing_token() {
    let server_app = ServerBuilder::new("auth-test-missing", "1.0")
        .with_simple_auth("secret123")
        .build();

    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "ping".to_string(),
        params: Some(json!({})),
        id: RequestId::Int(3),
        transport_metadata: None,
    };

    let result = server_app.core.handle_request(req).await;
    assert!(result.is_err(), "Request without token should fail");
}
