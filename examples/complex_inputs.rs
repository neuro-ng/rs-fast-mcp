use rs_fast_mcp::error::FastMCPError;
use rs_fast_mcp::server::core::FastMCPServer;
use rs_fast_mcp::server::transport::Transport;
use rs_fast_mcp::server::transport::stdio::StdioTransport;
use rs_fast_mcp::tools::tool::{Tool, ToolFunction, ToolKind, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

// --- Domain Types ---

#[derive(Debug, Deserialize, Serialize)]
struct Shrimp {
    name: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct ShrimpTank {
    shrimp: Vec<Shrimp>,
}

#[derive(Debug, Deserialize, Serialize)]
struct NameShrimpArgs {
    tank: ShrimpTank,
    extra_names: Vec<String>,
}

// --- Tool Implementation ---

fn create_shrimp_tool() -> Tool {
    Tool {
        name: "name_shrimp".to_string(),
        description: Some("List all shrimp names in the tank".to_string()),
        enabled: true,
        key: None,
        title: None,
        meta: None,
        tags: Default::default(),
        data: ToolKind::Function(ToolFunction {
            name: "name_shrimp".to_string(),
            description: None,
            // Strict Schema
            input_schema: json!({
                "type": "object",
                "properties": {
                    "tank": {
                        "type": "object",
                        "properties": {
                            "shrimp": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "name": { "type": "string", "maxLength": 10 }
                                    },
                                    "required": ["name"]
                                }
                            }
                        },
                        "required": ["shrimp"]
                    },
                    "extra_names": {
                        "type": "array",
                        "items": { "type": "string", "maxLength": 10 }
                    }
                },
                "required": ["tank", "extra_names"]
            }),
            output_schema: None,
            fn_handler: Arc::new(Box::new(|_ctx, args| {
                Box::pin(async move {
                    // Manual deserialization to validate structure
                    let parsed: NameShrimpArgs = serde_json::from_value(args).map_err(|e| {
                        FastMCPError::InvalidRequest(format!("Invalid arguments: {}", e))
                    })?;

                    let mut names: Vec<String> =
                        parsed.tank.shrimp.into_iter().map(|s| s.name).collect();
                    names.extend(parsed.extra_names);

                    Ok(ToolResult {
                        content: vec![],
                        structured_content: Some(json!(names)),
                    })
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
            compiled_schema: None,
        }),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = FastMCPServer::new("complex_inputs", "1.0");

    server.add_tool(create_shrimp_tool()).unwrap();

    let transport = StdioTransport::new();
    let handler = Arc::new(server.clone());
    let rx = Some(server.subscribe_notifications());
    transport.start(handler, rx).await?;

    Ok(())
}
