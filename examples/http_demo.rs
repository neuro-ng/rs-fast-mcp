use rs_fast_mcp::server::context::Context;
use rs_fast_mcp::server::core::FastMCPServer;
use rs_fast_mcp::server::transport::Transport;
use rs_fast_mcp::server::transport::http::HttpTransport;
use rs_fast_mcp::tools::tool::{Tool, ToolFunction, ToolKind, ToolResult};
use serde_json::json;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    rs_fast_mcp::server::logging::init_logging("info").expect("Failed to initialize logging");

    info!("Starting HTTP Demo Server on port 8080...");

    let server = FastMCPServer::new("http-demo", "1.0.0");

    let hello_tool = Tool {
        name: "hello".to_string(),
        title: None,
        description: Some("Say hello".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "hello".to_string(),
            description: None,
            input_schema: json!({ "type": "object" }),
            output_schema: None,
            fn_handler: Arc::new(Box::new(|_ctx: Context, _args: serde_json::Value| {
                Box::pin(async move {
                    Ok(ToolResult {
                        content: vec![],
                        structured_content: Some(json!({ "message": "Hello via HTTP!" })),
                    })
                })
                    as Pin<
                        Box<
                            dyn Future<
                                    Output = Result<
                                        rs_fast_mcp::tools::tool::ToolResult,
                                        rs_fast_mcp::error::FastMCPError,
                                    >,
                                > + Send,
                        >,
                    >
            })),
            compiled_schema: None,
        }),
    };
    server.add_tool(hello_tool).unwrap();

    let transport = HttpTransport::new("127.0.0.1", 8080);
    transport.start(Arc::new(server), None).await?;

    Ok(())
}
