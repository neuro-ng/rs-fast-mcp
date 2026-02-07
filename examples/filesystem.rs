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
use tokio::fs;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    rs_fast_mcp::server::logging::init_logging("info").expect("Failed to initialize logging");

    info!("Starting filesystem example...");

    let server = FastMCPServer::new("filesystem-server", "1.0.0");

    // Tool: read_file
    let read_file = Tool {
        name: "read_file".to_string(),
        title: None,
        description: Some("Reads a file from the filesystem".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "read_file".to_string(),
            description: None,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }),
            output_schema: None,
            fn_handler: Arc::new(Box::new(|_ctx: Context, args: serde_json::Value| {
                Box::pin(async move {
                    let path = args
                        .get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| FastMCPError::InvalidRequest("Missing path".to_string()))?;
                    info!("Reading file: {}", path);
                    match fs::read_to_string(path).await {
                        Ok(content) => Ok(ToolResult {
                            content: vec![],
                            structured_content: Some(json!({ "content": content })),
                        }),
                        Err(e) => {
                            error!("Failed to read file: {}", e);
                            Err(FastMCPError::new(e.to_string()))
                        }
                    }
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
            compiled_schema: None,
        }),
    };
    server
        .add_tool(read_file)
        .expect("Failed to add read_file tool");

    // Tool: write_file
    let write_file = Tool {
        name: "write_file".to_string(),
        title: None,
        description: Some("Writes content to a file".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "write_file".to_string(),
            description: None,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }),
            output_schema: None,
            fn_handler: Arc::new(Box::new(|_ctx: Context, args: serde_json::Value| {
                Box::pin(async move {
                    let path = args
                        .get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| FastMCPError::InvalidRequest("Missing path".to_string()))?;
                    let content =
                        args.get("content")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                FastMCPError::InvalidRequest("Missing content".to_string())
                            })?;
                    info!("Writing to file: {}", path);
                    match fs::write(path, content).await {
                        Ok(_) => Ok(ToolResult {
                            content: vec![],
                            structured_content: Some(json!({ "status": "success" })),
                        }),
                        Err(e) => {
                            error!("Failed to write file: {}", e);
                            Err(FastMCPError::new(e.to_string()))
                        }
                    }
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
            compiled_schema: None,
        }),
    };
    server
        .add_tool(write_file)
        .expect("Failed to add write_file tool");

    // Tool: list_directory
    let list_directory = Tool {
        name: "list_directory".to_string(),
        title: None,
        description: Some("Lists contents of a directory".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "list_directory".to_string(),
            description: None,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }),
            output_schema: None,
            fn_handler: Arc::new(Box::new(|_ctx: Context, args: serde_json::Value| {
                Box::pin(async move {
                    let path = args
                        .get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| FastMCPError::InvalidRequest("Missing path".to_string()))?;
                    info!("Listing directory: {}", path);
                    match fs::read_dir(path).await {
                        Ok(mut entries) => {
                            let mut files = Vec::new();
                            while let Ok(Some(entry)) = entries.next_entry().await {
                                if let Ok(name) = entry.file_name().into_string() {
                                    files.push(name);
                                }
                            }
                            Ok(ToolResult {
                                content: vec![],
                                structured_content: Some(json!({ "files": files })),
                            })
                        }
                        Err(e) => {
                            error!("Failed to list directory: {}", e);
                            Err(FastMCPError::new(e.to_string()))
                        }
                    }
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
            compiled_schema: None,
        }),
    };
    server
        .add_tool(list_directory)
        .expect("Failed to add list_directory tool");

    // Run Server
    let transport = StdioTransport::new();
    let handler = Arc::new(server.clone());
    let rx = Some(server.subscribe_notifications());

    info!("Filesystem server ready.");
    transport.start(handler, rx).await?;

    Ok(())
}
