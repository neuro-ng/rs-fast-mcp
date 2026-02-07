use rs_fast_mcp::server::core::FastMCPServer;
use rs_fast_mcp::server::app::Server;
use rs_fast_mcp::server::transport::Transport;
use rs_fast_mcp::error::FastMCPError;

use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use rs_fast_mcp::server::transport::RequestHandler;

// Mock Transport that finishes immediately
struct MockTransport;

#[async_trait]
impl Transport for MockTransport {
    async fn start(&self, _handler: Arc<dyn RequestHandler>, _notification_rx: Option<tokio::sync::broadcast::Receiver<rs_fast_mcp::mcp::types::JsonRpcMessage>>) -> Result<(), FastMCPError> {
        // Simulate brief work then exit
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(())
    }
}

#[tokio::test]
async fn test_lifespan_hooks() {
    let server_core = FastMCPServer::new("lifespan-test", "1.0");
    
    let startup_called = Arc::new(Mutex::new(false));
    let shutdown_called = Arc::new(Mutex::new(false));
    
    let startup_flag = startup_called.clone();
    server_core.add_startup_hook(move || {
        let flag = startup_flag.clone();
        Box::pin(async move {
            *flag.lock().unwrap() = true;
            Ok(())
        })
    });

    let shutdown_flag = shutdown_called.clone();
    server_core.add_shutdown_hook(move || {
        let flag = shutdown_flag.clone();
        Box::pin(async move {
            *flag.lock().unwrap() = true;
            Ok(())
        })
    });

    let app = Server::new(server_core, vec![Box::new(MockTransport)]);
    
    // Run server
    let result = app.run().await;
    
    // We can't easily test signal handling in unit tests without spawning a process.
    // However, we can assert that if run returns, shutdown hooks were called.
    // The previous test verifies hooks mechanism.
    // This file primarily tests that hooks *exist* and *run* when `run_startup` / `run_shutdown` are invoked manually in tests,
    // which `FastMCP` exposes.
    
    // To properly test the signal handling integrated into `Server::run`, we'd need an integration test
    // that sends SIGINT to the test process, which kills the test runner.
    // So we rely on manual verification or trust the logic.
    assert!(result.is_ok());
    assert!(*startup_called.lock().unwrap(), "Startup hook should be called");
    assert!(*shutdown_called.lock().unwrap(), "Shutdown hook should be called");
}

#[tokio::test]
async fn test_shutdown_hook_execution() {
    let server = FastMCPServer::new("shutdown_test", "1.0");
    let shutdown_called = Arc::new(Mutex::new(false));
    let s_clone = shutdown_called.clone();
    
    server.add_shutdown_hook(move || {
        let s = s_clone.clone();
        Box::pin(async move {
            let mut guard = s.lock().unwrap();
            *guard = true;
            Ok(())
        })
    });

    server.run_shutdown().await.unwrap();
    assert!(*shutdown_called.lock().unwrap());
}
