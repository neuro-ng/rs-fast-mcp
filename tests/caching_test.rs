use rs_fast_mcp::server::middleware::caching::CacheMiddleware;
use rs_fast_mcp::server::middleware::Middleware;
use rs_fast_mcp::mcp::types::{JsonRpcRequest, JsonRpcResponse, RequestId};
use std::sync::{Arc, Mutex};
use std::time::Duration;


#[tokio::test]
async fn test_caching_middleware() {
    let middleware = CacheMiddleware::new(100, 1); // 1 sec TTL
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "tools/call".to_string(),
        params: Some(serde_json::json!({"name": "test_tool"})),
        id: RequestId::Int(1),
        transport_metadata: None,
    };

    let counter = Arc::new(Mutex::new(0));
    let counter_clone = counter.clone();

    let next_fn = Arc::new(move |_: JsonRpcRequest| {
        let mut c = counter_clone.lock().unwrap();
        *c += 1;
        let count = *c;
        Box::pin(async move {
            Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: RequestId::Int(1),
                result: serde_json::json!({"count": count}),
            })
        }) as std::pin::Pin<Box<dyn std::future::Future<Output = _> + Send>>
    });

    // 1st Request - Miss
    let res = middleware.handle(req.clone(), Box::new({
        let n = next_fn.clone(); 
        move |r| n(r)
    })).await.unwrap();
    assert_eq!(res.result["count"], 1);

    // 2nd Request - Hit
    let res = middleware.handle(req.clone(), Box::new({
        let n = next_fn.clone(); 
        move |r| n(r)
    })).await.unwrap();
    assert_eq!(res.result["count"], 1); // Should still be 1 (Cached)
    
    // Wait for Expiration (1.1s)
    tokio::time::sleep(Duration::from_millis(1100)).await;
    
    // 3rd Request - Miss (Expired)
    let res = middleware.handle(req.clone(), Box::new({
        let n = next_fn.clone(); 
        move |r| n(r)
    })).await.unwrap();
    assert_eq!(res.result["count"], 2); // Should be 2 (New Execution)
}
