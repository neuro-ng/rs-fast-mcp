use rs_fast_mcp::server::core::FastMCPServer;


use rs_fast_mcp::error::FastMCPError;
use rs_fast_mcp::mcp::types::{JsonRpcRequest, JsonRpcResponse, RequestId};
use rs_fast_mcp::server::middleware::{Middleware, Next, BoxFuture};

use std::sync::{Arc, Mutex};


use serde_json::json;



// Logging Middleware that counts requests
struct CountingMiddleware {
    count: Arc<Mutex<i32>>,
}

impl Middleware for CountingMiddleware {
    fn handle<'a, 'b>(
        &'a self,
        req: JsonRpcRequest,
        next: Next<'b>,
    ) -> BoxFuture<'a, Result<JsonRpcResponse, FastMCPError>> 
    where 'b: 'a
    {
        let count = self.count.clone();
        Box::pin(async move {
            {
                let mut c = count.lock().unwrap();
                *c += 1;
            }
            next(req).await
        })
    }
}



#[tokio::test]
async fn test_middleware_chain() {
    let server = FastMCPServer::new("mw-test", "1.0");
    
    let counter = Arc::new(Mutex::new(0));
    server.add_middleware(CountingMiddleware { count: counter.clone() });
    
    // Send a ping request
    let req = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "ping".to_string(),
        params: None,
        id: RequestId::Int(1),
        transport_metadata: None,
    };
    
    let resp = server.handle_request(req).await.expect("Request failed");
    assert_eq!(resp.result, json!("pong"));
    
    assert_eq!(*counter.lock().unwrap(), 1);
}
