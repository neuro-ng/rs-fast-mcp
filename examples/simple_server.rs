use rs_fast_mcp::error::FastMCPError;
use rs_fast_mcp::mcp::types::{
    BaseMetadata, ContentBlock, Resource, ResourceContents, TextContent,
};
use rs_fast_mcp::prompts::prompt::{Prompt, PromptFunction, PromptMessage};
use rs_fast_mcp::server::core::FastMCPServer;
use rs_fast_mcp::server::transport::Transport;
use rs_fast_mcp::server::transport::stdio::StdioTransport;
use rs_fast_mcp::tools::tool::{Tool, ToolFunction, ToolKind, ToolResult};
use serde_json::json;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::info;

use rs_fast_mcp::server::context::Context; // Added import

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    rs_fast_mcp::server::logging::init_logging("info").expect("Failed to initialize logging");

    info!("Starting simple-server example...");

    // 1. Create Server
    let server = FastMCPServer::new("simple-mcp-server", "1.0.0");

    // 2. Add Tool: echo
    let echo_tool = Tool {
        name: "echo".to_string(),
        title: None,
        description: Some("Echoes back the input".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "echo".to_string(),
            description: None,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
                "required": ["message"]
            }),
            output_schema: None,
            fn_handler: Arc::new(Box::new(|_ctx: Context, args: serde_json::Value| {
                Box::pin(async move {
                    let msg = args
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    info!("Executing echo tool with message: {}", msg);
                    Ok(ToolResult {
                        content: vec![],
                        structured_content: Some(json!({ "echo": msg })),
                    })
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
            compiled_schema: None,
        }),
    };
    server.add_tool(echo_tool).expect("Failed to add echo tool");

    // 3. Add Resource: static text
    let resource = Resource {
        uri: "file:///hello".to_string(),
        base_metadata: BaseMetadata {
            name: "hello".to_string(),
            title: None,
        },
        description: Some("A static hello resource".to_string()),
        mime_type: Some("text/plain".to_string()),
        annotations: None,
        size: None,
        icons: None,
        tags: None,
    };

    let resource_handler = Arc::new(Box::new(|_uri: String, _ctx: Context| {
        Box::pin(async {
            Ok(vec![ResourceContents {
                uri: "file:///hello".to_string(),
                mime_type: Some("text/plain".to_string()),
                text: Some("Hello from simple server!".to_string()),
                blob: None,
            }])
        })
            as Pin<Box<dyn Future<Output = Result<Vec<ResourceContents>, FastMCPError>> + Send>>
    })
        as rs_fast_mcp::resources::manager::ResourceReadHandler);

    server
        .add_resource(resource, Some(resource_handler))
        .expect("Failed to add hello resource");

    // 4. Add Prompt: greet
    let prompt = Prompt {
        name: "greet".to_string(),
        title: None,
        description: Some("Generates a greeting".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: PromptFunction {
            name: "greet".to_string(),
            description: None,
            arguments: None,
            fn_handler: Arc::new(Box::new(|args| {
                Box::pin(async move {
                    let name = args
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Stranger")
                        .to_string();
                    Ok(vec![PromptMessage {
                        role: "user".to_string(),
                        content: ContentBlock::Text(TextContent {
                            text: format!("Hello, {}!", name),
                            type_: "text".to_string(),
                            annotations: None,
                        }),
                    }])
                })
                    as Pin<
                        Box<dyn Future<Output = Result<Vec<PromptMessage>, FastMCPError>> + Send>,
                    >
            })),
        },
    };
    server
        .add_prompt(prompt)
        .expect("Failed to add greet prompt");

    // 5. Run Server with Stdio Transport
    let transport = StdioTransport::new();
    let handler = Arc::new(server.clone());
    let rx = Some(server.subscribe_notifications());

    info!("Server ready. Listening on stdio...");
    transport.start(handler, rx).await?;

    Ok(())
}
