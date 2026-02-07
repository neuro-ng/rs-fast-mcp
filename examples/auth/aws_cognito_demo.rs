use clap::Parser;
use rs_fast_mcp::error::FastMCPError;
use rs_fast_mcp::server::auth::AuthMiddleware;
use rs_fast_mcp::server::auth::providers::aws::AwsCognito;
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
    #[arg(long, env = "AWS_COGNITO_USER_POOL_ID")]
    user_pool_id: String,

    #[arg(long, env = "AWS_COGNITO_REGION")]
    region: String,

    #[arg(long, env = "AWS_COGNITO_CLIENT_ID")]
    client_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    rs_fast_mcp::server::logging::init_logging("debug").expect("Failed to initialize logging");

    info!("🚀 Starting AWS Cognito Demo Server...");

    // Initialize AWS Cognito Provider
    // Note: This makes a network call to fetch OIDC config
    let provider = AwsCognito::create(&args.user_pool_id, &args.region, &args.client_id)
        .await
        .expect("Failed to initialize AWS Cognito provider");
    let provider = Arc::new(provider);

    let server = FastMCPServer::new("aws_cognito_demo", "1.0.0");

    server.add_middleware_arc(Arc::new(AuthMiddleware::new(provider)));

    let tool = Tool {
        name: "secure_task".to_string(),
        title: None,
        description: Some("A protected task".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "secure_task".to_string(),
            description: None,
            input_schema: json!({ "type": "object" }),
            output_schema: None,
            fn_handler: Arc::new(Box::new(|_ctx: Context, _args: serde_json::Value| {
                Box::pin(async move {
                    let ctx = rs_fast_mcp::server::auth::current_context();
                    let msg = if let Some(c) = ctx {
                        format!("Hello AWS User: {:?}!", c.user_id)
                    } else {
                        "Access Denied".to_string()
                    };

                    Ok(ToolResult {
                        content: vec![],
                        structured_content: Some(json!({ "status": msg })),
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
