use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use async_trait::async_trait;
use rs_fast_mcp::client::ClientTransport;
use rs_fast_mcp::client::auth::AuthHandler;
use rs_fast_mcp::client::transport::sse::SseClientTransport;
use rs_fast_mcp::error::FastMCPError;
use rs_fast_mcp::mcp::types::JsonRpcMessage;
use std::sync::Arc;
use std::time::Duration;

struct MockAuthHandler {
    token: String,
}

#[async_trait]
impl AuthHandler for MockAuthHandler {
    async fn get_auth_header(&self) -> Result<Option<String>, FastMCPError> {
        Ok(Some(format!("Bearer {}", self.token)))
    }
}

async fn mock_sse_endpoint(req: actix_web::HttpRequest) -> impl Responder {
    if let Some(auth) = req.headers().get("Authorization")
        && auth.to_str().unwrap() == "Bearer test-token"
    {
        // Return stream
        return HttpResponse::Ok()
            .insert_header(("Content-Type", "text/event-stream"))
            .body("event: endpoint\ndata: /message\n\n");
    }
    HttpResponse::Unauthorized().finish()
}

async fn mock_message_endpoint(
    req: actix_web::HttpRequest,
    _body: web::Json<JsonRpcMessage>,
) -> impl Responder {
    if let Some(auth) = req.headers().get("Authorization")
        && auth.to_str().unwrap() == "Bearer test-token"
    {
        return HttpResponse::Ok().finish();
    }
    HttpResponse::Unauthorized().finish()
}

#[tokio::test]
async fn test_sse_client_auth_injection() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("Failed to bind");
    let port = listener.local_addr().expect("Failed to get port").port();

    let server = HttpServer::new(|| {
        App::new()
            .route("/sse", web::get().to(mock_sse_endpoint))
            .route("/message", web::post().to(mock_message_endpoint))
    })
    .listen(listener)
    .expect("Failed to listen")
    .run();

    let server_handle = server.handle();
    tokio::spawn(server);

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Test Client with Auth
    let url = format!("http://127.0.0.1:{}/sse", port);
    let auth = Arc::new(MockAuthHandler {
        token: "test-token".to_string(),
    });

    let transport = SseClientTransport::new(url.clone(), Some(auth));

    // Test Connection (GET) implicitly happens on spawn
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Send a message (POST)
    let msg = JsonRpcMessage::Request(rs_fast_mcp::mcp::types::JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "ping".to_string(),
        params: None,
        id: rs_fast_mcp::mcp::types::RequestId::Int(1),
        transport_metadata: None,
    });

    let result = transport.send(msg).await;
    assert!(result.is_ok(), "Send failed: {:?}", result.err());

    // Clean up
    server_handle.stop(false).await;
}
