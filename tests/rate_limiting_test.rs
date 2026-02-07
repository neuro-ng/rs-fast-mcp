use rs_fast_mcp::mcp::types::{JsonRpcRequest, JsonRpcResponse, RequestId};
use rs_fast_mcp::server::middleware::Middleware;
use rs_fast_mcp::server::middleware::rate_limiting::RateLimitMiddleware;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_rate_limiting_bucket() {
    // Capacity 2.0, Refill rate 1.0/sec
    let middleware = RateLimitMiddleware::new(2.0, 1.0);
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "test".to_string(),
        params: None,
        id: RequestId::Int(1),
        transport_metadata: None,
    };

    // Simple mock "next" handler
    let next_fn = Arc::new(|_: JsonRpcRequest| {
        Box::pin(async {
            Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: RequestId::Int(1),
                result: Value::Null,
            })
        }) as std::pin::Pin<Box<dyn std::future::Future<Output = _> + Send>>
    });

    // 1st Request (Allowed)
    let res = middleware
        .handle(req.clone(), Box::new(move |r| next_fn(r)))
        .await;
    assert!(res.is_ok(), "First request should be allowed");

    // 2nd Request (Allowed)
    let next_fn = Arc::new(|_: JsonRpcRequest| {
        Box::pin(async { Ok(JsonRpcResponse::new(RequestId::Int(1), Value::Null)) })
    });
    let res = middleware
        .handle(req.clone(), Box::new(move |r| next_fn(r)))
        .await;
    assert!(res.is_ok(), "Second request should be allowed");

    // 3rd Request (Denied)
    let next_fn = Arc::new(|_: JsonRpcRequest| {
        Box::pin(async { Ok(JsonRpcResponse::new(RequestId::Int(1), Value::Null)) })
    });
    let res = middleware
        .handle(req.clone(), Box::new(move |r| next_fn(r)))
        .await;
    assert!(res.is_err(), "Third request should be denied");

    // Wait 1.1s
    tokio::time::sleep(Duration::from_millis(1100)).await;

    // 4th Request (Allowed)
    let next_fn = Arc::new(|_: JsonRpcRequest| {
        Box::pin(async { Ok(JsonRpcResponse::new(RequestId::Int(1), Value::Null)) })
    });
    let res = middleware
        .handle(req.clone(), Box::new(move |r| next_fn(r)))
        .await;
    assert!(res.is_ok(), "Fourth request after sleep should be allowed");
}
