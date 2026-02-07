use rs_fast_mcp::client::transport::sse::SseClientTransport;
use rs_fast_mcp::client::transport::stdio::StdioClientTransport;

#[tokio::test]
async fn test_transport_instantiation() {
    let _sse = SseClientTransport::new("http://localhost:8080/sse".to_string(), None);
    let _stdio = StdioClientTransport::new();
}
