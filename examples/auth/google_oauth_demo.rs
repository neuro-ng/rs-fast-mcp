use clap::Parser;
use rs_fast_mcp::error::FastMCPError;
use rs_fast_mcp::server::auth::AuthMiddleware;
use rs_fast_mcp::server::auth::providers::google::GoogleProvider;
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
    /// Google Client ID for token validation
    #[arg(long, env = "GOOGLE_CLIENT_ID")]
    client_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // 1. Initialize Logging
    rs_fast_mcp::server::logging::init_logging("debug").expect("Failed to initialize logging");

    info!("🚀 Starting Google OAuth Demo Server...");

    // 2. Initialize Google Provider
    let google_provider = Arc::new(GoogleProvider::new(&args.client_id));

    // 3. Create Server
    let server = FastMCPServer::new("google_oauth_demo", "1.0.0");

    // 4. Add Auth Middleware
    server.add_middleware_arc(Arc::new(AuthMiddleware::new(google_provider)));

    // 5. Add Tool: user_info
    let user_info_tool = Tool {
        name: "user_info".to_string(),
        title: None,
        description: Some("Returns the authenticated Google user info".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "user_info".to_string(),
            description: None,
            input_schema: json!({ "type": "object" }),
            output_schema: None,
            fn_handler: Arc::new(Box::new(|_ctx: Context, _args: serde_json::Value| {
                Box::pin(async move {
                    let auth_info = rs_fast_mcp::server::auth::current_context()
                        .map(|ctx| {
                            format!(
                                "Google User ID: {:?} (Client ID: {:?})",
                                ctx.user_id, ctx.client_id
                            )
                        })
                        .unwrap_or_else(|| "Not authenticated".to_string());

                    Ok(ToolResult {
                        content: vec![],
                        structured_content: Some(json!({ "info": auth_info })),
                    })
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
            compiled_schema: None,
        }),
    };
    server.add_tool(user_info_tool).expect("Failed to add tool");

    // 6. Run Server
    let transport = StdioTransport::new();
    let handler = Arc::new(server.clone());
    let rx = Some(server.subscribe_notifications());

    info!("Listening on Stdio...");
    transport.start(handler, rx).await?;

    Ok(())
}
