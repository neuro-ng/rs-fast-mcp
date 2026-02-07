use rs_fast_mcp::error::FastMCPError;
use rs_fast_mcp::server::context::Context;
use rs_fast_mcp::server::core::FastMCPServer;
use rs_fast_mcp::server::transport::Transport;
use rs_fast_mcp::server::transport::stdio::StdioTransport;
use rs_fast_mcp::tools::tool::{Tool, ToolFunction, ToolKind, ToolResult};
use serde_json::json;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    rs_fast_mcp::server::logging::init_logging("info").expect("Failed to initialize logging");

    let server = FastMCPServer::new("calculator", "1.0.0");

    // Add Tool
    let add_tool = Tool {
        name: "add".to_string(),
        title: Some("Add".to_string()),
        description: Some("Adds two numbers".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "add".to_string(),
            description: None,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "a": { "type": "number" },
                    "b": { "type": "number" }
                },
                "required": ["a", "b"]
            }),
            output_schema: None,
            fn_handler: Arc::new(Box::new(|_ctx: Context, args: serde_json::Value| {
                Box::pin(async move {
                    let a = args
                        .get("a")
                        .and_then(|v| v.as_f64())
                        .ok_or(FastMCPError::InvalidRequest("Missing a".into()))?;
                    let b = args
                        .get("b")
                        .and_then(|v| v.as_f64())
                        .ok_or(FastMCPError::InvalidRequest("Missing b".into()))?;
                    Ok(ToolResult {
                        content: vec![],
                        structured_content: Some(json!({ "result": a + b })),
                    })
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
            compiled_schema: None,
        }),
    };
    server.add_tool(add_tool).expect("Failed to add tool");

    let transport = StdioTransport::new();
    transport.start(Arc::new(server), None).await?;
    Ok(())
}
