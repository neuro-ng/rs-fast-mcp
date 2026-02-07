use clap::Parser;
use rs_fast_mcp::error::FastMCPError;
use rs_fast_mcp::server::auth::AuthMiddleware;
use rs_fast_mcp::server::auth::providers::workos::WorkOSProvider;
use rs_fast_mcp::server::context::Context;
use rs_fast_mcp::server::core::FastMCPServer;
use rs_fast_mcp::server::transport::Transport;
use rs_fast_mcp::server::transport::stdio::StdioTransport;
use rs_fast_mcp::tools::tool::{Tool, ToolFunction, ToolKind, ToolResult};
use serde_json::json;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::info;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, env = "WORKOS_CLIENT_ID")]
    client_id: String,

    #[arg(long, env = "WORKOS_AUTHKIT_DOMAIN")]
    authkit_domain: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    rs_fast_mcp::server::logging::init_logging("debug").expect("Failed to initialize logging");

    info!("🚀 Starting WorkOS OAuth Demo Server (AuthKit)...");

    // Initialize WorkOS Provider
    let provider = WorkOSProvider::new(&args.client_id, &args.authkit_domain)
        .await
        .expect("Failed to initialize WorkOS provider");
    let provider = Arc::new(provider);

    let server = FastMCPServer::new("workos_oauth_demo", "1.0.0");
    server.add_middleware_arc(Arc::new(AuthMiddleware::new(provider)));

    let tool = Tool {
        name: "admin_only".to_string(),
        title: None,
        description: Some("Action restricted to AuthKit users".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "admin_only".to_string(),
            description: None,
            input_schema: json!({ "type": "object" }),
            output_schema: None,
            fn_handler: Arc::new(Box::new(|_ctx: Context, _args: serde_json::Value| {
                Box::pin(async move {
                    Ok(ToolResult {
                        content: vec![],
                        structured_content: Some(json!({ "status": "Authorized by WorkOS" })),
                    })
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
            compiled_schema: None,
        }),
    };
    server.add_tool(tool).expect("Failed to add tool");

    let transport = StdioTransport::new();
    let handler = Arc::new(server.clone());
    let rx = Some(server.subscribe_notifications());

    info!("Listening on Stdio...");
    transport.start(handler, rx).await?;

    Ok(())
}
