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
use tracing::info;

mod atproto;
use atproto::client::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    rs_fast_mcp::server::logging::init_logging("info").expect("Failed to initialize logging");

    info!("Starting ATProto Server...");

    let server = FastMCPServer::new("atproto-server", "1.0.0");
    let client = Arc::new(Client::new());

    // Tool: Login
    let client_clone = client.clone();
    let login_tool = Tool {
        name: "login".to_string(),
        title: None,
        description: Some("Login to Bluesky".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "login".to_string(),
            description: None,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "identifier": { "type": "string" },
                    "password": { "type": "string" }
                },
                "required": ["identifier", "password"]
            }),
            output_schema: None,
            fn_handler: Arc::new(Box::new(move |_ctx: Context, args: serde_json::Value| {
                let client = client_clone.clone();
                Box::pin(async move {
                    let id = args
                        .get("identifier")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let pass = args.get("password").and_then(|v| v.as_str()).unwrap_or("");

                    match client.login(id, pass).await {
                        Ok(handle) => Ok(ToolResult {
                            content: vec![],
                            structured_content: Some(
                                json!({ "status": "logged_in", "handle": handle }),
                            ),
                        }),
                        Err(e) => Err(FastMCPError::new(e)),
                    }
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
            compiled_schema: None,
        }),
    };
    server
        .add_tool(login_tool)
        .expect("Failed to add login tool");

    // Tool: Post
    let client_clone = client.clone();
    let post_tool = Tool {
        name: "post".to_string(),
        title: None,
        description: Some("Create a post".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "post".to_string(),
            description: None,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string" }
                },
                "required": ["text"]
            }),
            output_schema: None,
            fn_handler: Arc::new(Box::new(move |_ctx: Context, args: serde_json::Value| {
                let client = client_clone.clone();
                Box::pin(async move {
                    let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
                    let record = atproto::posts::create_post_record(text);

                    match client.create_record("app.bsky.feed.post", record).await {
                        Ok(status) => Ok(ToolResult {
                            content: vec![],
                            structured_content: Some(json!({ "status": status })),
                        }),
                        Err(e) => Err(FastMCPError::new(e)),
                    }
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
            compiled_schema: None,
        }),
    };
    server.add_tool(post_tool).expect("Failed to add post tool");

    // Tool: Get Timeline
    let client_clone = client.clone();
    let timeline_tool = Tool {
        name: "get_timeline".to_string(),
        title: None,
        description: Some("Get home timeline".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "get_timeline".to_string(),
            description: None,
            input_schema: json!({ "type": "object" }),
            output_schema: None,
            fn_handler: Arc::new(Box::new(move |_ctx: Context, _args: serde_json::Value| {
                let client = client_clone.clone();
                Box::pin(async move {
                    match client.get_timeline().await {
                        Ok(feed) => Ok(ToolResult {
                            content: vec![],
                            structured_content: Some(json!({ "feed": feed })),
                        }),
                        Err(e) => Err(FastMCPError::new(e)),
                    }
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
            compiled_schema: None,
        }),
    };
    server
        .add_tool(timeline_tool)
        .expect("Failed to add get_timeline tool");

    // Run Server
    let transport = StdioTransport::new();
    let handler = Arc::new(server.clone());
    let rx = Some(server.subscribe_notifications()); // No notifications for now

    info!("ATProto Server ready.");
    transport.start(handler, rx).await?;

    Ok(())
}
