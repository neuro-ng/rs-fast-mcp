use chrono::Local;
use rs_fast_mcp::error::FastMCPError;
use rs_fast_mcp::mcp::types::{BaseMetadata, Resource, ResourceContents};
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    rs_fast_mcp::server::logging::init_logging("info").expect("Failed to initialize logging");

    info!("🚀 Demo server starting...");

    // Create Server
    let server = FastMCPServer::new("demo-server", "1.0.0");

    // --- Tools ---

    // 1. Tool: greet
    let greet_tool = Tool {
        name: "greet".to_string(),
        title: None,
        description: Some("Greet a person by name".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "greet".to_string(),
            description: None,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" }
                }
            }),
            output_schema: None,
            compiled_schema: None,
            fn_handler: Arc::new(Box::new(|_ctx: Context, args: serde_json::Value| {
                Box::pin(async move {
                    let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("World");
                    Ok(ToolResult {
                        content: vec![],
                        structured_content: Some(json!({ "message": format!("Hello, {}!", name) })),
                    })
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
        }),
    };
    server.add_tool(greet_tool)?;

    // 2. Tool: echo
    let echo_tool = Tool {
        name: "echo".to_string(),
        title: None,
        description: Some("Echo back the input".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "echo".to_string(),
            description: None,
            input_schema: json!({
                "type": "object"
            }),
            output_schema: None,
            compiled_schema: None,
            fn_handler: Arc::new(Box::new(|_ctx: Context, args: serde_json::Value| {
                Box::pin(async move {
                    Ok(ToolResult {
                        content: vec![],
                        structured_content: Some(json!({ "echoed": args })),
                    })
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
        }),
    };
    server.add_tool(echo_tool)?;

    // 3. Tool: get_time
    let time_tool = Tool {
        name: "get_time".to_string(),
        title: None,
        description: Some("Get current server time".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "get_time".to_string(),
            description: None,
            input_schema: json!({ "type": "object" }),
            output_schema: None,
            compiled_schema: None,
            fn_handler: Arc::new(Box::new(|_, _| {
                Box::pin(async move {
                    let now = Local::now().to_rfc3339();
                    Ok(ToolResult {
                        content: vec![],
                        structured_content: Some(json!({ "time": now })),
                    })
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
        }),
    };
    server.add_tool(time_tool)?;

    info!("Tools: greet, echo, get_time");

    // --- Resources ---

    // Resource: demo://info
    let resource = Resource {
        uri: "demo://info".to_string(),
        base_metadata: BaseMetadata {
            name: "Server Info".to_string(),
            title: None,
        },
        description: Some("Information about this demo server".to_string()),
        mime_type: Some("text/plain".to_string()),
        annotations: None,
        size: None,
        icons: None,
        tags: None,
    };

    // We need to capture server ID or tool count, but for simplicity let's just return static info + dynamic version
    // Accessing 'server' inside the handler would require Arc/Mutex which implies cloning the server earlier.
    // For this demo, we'll keep it simple.

    let resource_handler = Arc::new(Box::new(|_uri: String, _ctx: Context| {
        Box::pin(async {
            let info = "OxFastMCP Demo Server (Rust Port)\nVersion: 1.0.0".to_string();
            Ok(vec![ResourceContents {
                uri: "demo://info".to_string(),
                mime_type: Some("text/plain".to_string()),
                text: Some(info),
                blob: None,
            }])
        })
            as Pin<Box<dyn Future<Output = Result<Vec<ResourceContents>, FastMCPError>> + Send>>
    })
        as rs_fast_mcp::resources::manager::ResourceReadHandler);

    server.add_resource(resource, Some(resource_handler))?;

    info!("Resources: demo://info");

    // Run Server
    let transport = StdioTransport::new();
    let handler = Arc::new(server.clone());
    let rx = Some(server.subscribe_notifications());

    transport.start(handler, rx).await?;

    Ok(())
}
