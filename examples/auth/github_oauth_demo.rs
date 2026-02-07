use clap::Parser;
use rs_fast_mcp::error::FastMCPError;
use rs_fast_mcp::server::auth::AuthMiddleware;
use rs_fast_mcp::server::auth::providers::github::GitHubProvider;
use rs_fast_mcp::server::context::Context;
use rs_fast_mcp::server::core::FastMCPServer;
use rs_fast_mcp::server::transport::Transport;
use rs_fast_mcp::server::transport::http::HttpTransport;
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
    /// Use HTTP transport instead of Stdio
    #[arg(long)]
    http: bool,

    /// Port for HTTP transport
    #[arg(long, default_value_t = 8000)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // 1. Initialize Logging
    rs_fast_mcp::server::logging::init_logging("debug").expect("Failed to initialize logging");

    info!("🚀 Starting GitHub OAuth Demo Server...");

    // 2. Initialize GitHub Provider
    let github_provider = Arc::new(GitHubProvider::new());

    // 3. Create Server
    let server = FastMCPServer::new("github_oauth_demo", "1.0.0");

    // 4. Add Auth Middleware
    server.add_middleware_arc(Arc::new(AuthMiddleware::new(github_provider)));

    // 5. Add Tool: whoami
    let whoami_tool = Tool {
        name: "whoami".to_string(),
        title: None,
        description: Some("Returns the authenticated GitHub user ID".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "whoami".to_string(),
            description: None,
            input_schema: json!({ "type": "object" }),
            output_schema: None,
            fn_handler: Arc::new(Box::new(|_ctx: Context, _args: serde_json::Value| {
                Box::pin(async move {
                    // In a real scenario, we'd access task-local auth context here to get the user ID.
                    // Accessing it via: rs_fast_mcp::server::auth::current_context()
                    let auth_info = rs_fast_mcp::server::auth::current_context()
                        .map(|ctx| {
                            format!(
                                "Authenticated as: {:?} (Client: {:?})",
                                ctx.user_id, ctx.client_id
                            )
                        })
                        .unwrap_or_else(|| "Not authenticated (Middleware failed?)".to_string());

                    Ok(ToolResult {
                        content: vec![],
                        structured_content: Some(json!({ "message": auth_info })),
                    })
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
            compiled_schema: None,
        }),
    };
    server.add_tool(whoami_tool).expect("Failed to add tool");

    // 6. Run Server
    let handler = Arc::new(server.clone());

    if args.http {
        info!("Listening on http://127.0.0.1:{}", args.port);
        info!(
            "Test with: curl -X POST http://127.0.0.1:{}/message -H 'Authorization: Bearer YOUR_GITHUB_TOKEN' -d '{{\"jsonrpc\":\"2.0\",\"method\":\"tools/call\",\"params\":{{\"name\":\"whoami\"}},\"id\":1}}'",
            args.port
        );
        let transport = HttpTransport::new("127.0.0.1", args.port);
        transport.start(handler, None).await?;
    } else {
        info!("Listening on Stdio...");
        let transport = StdioTransport::new();
        let rx = Some(server.subscribe_notifications());
        transport.start(handler, rx).await?;
    }

    Ok(())
}
