use crate::client::Client;
use crate::client::transport::sse::SseClientTransport;
use crate::client::transport::stdio::StdioClientTransport;
use crate::server::core::FastMCPServer;
use crate::server::transport::Transport;
use crate::server::transport::http::HttpTransport;
use crate::server::transport::stdio::StdioTransport;
use std::process::exit;
use tracing::{error, info};

pub async fn run(
    transport_type: &str,
    port: u16,
    config_path: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let server = FastMCPServer::new("rs-fast-mcp", "0.1.0");

    // Determine config path
    let config_path = config_path
        .map(std::path::Path::new)
        .unwrap_or_else(|| std::path::Path::new("fastmcp.json"));

    if config_path.exists() {
        info!("Loading configuration from {}", config_path.display());
        match crate::mcp::config::ServerConfig::load_from_file(config_path) {
            Ok(config) => {
                if let Some(_name) = config.name {
                    // Re-create server with new name if needed, or we just rely on the default since name is immutable in FastMCPServer struct without setters (except creating new).
                    // Since FastMCPServer wraps Arc<FastMCP>, we can't mutate name easily.
                    // For now, let's just log it or if we really wanted to change it we'd need to reconstruct.
                    // But we already created it.
                    // Optimization: Move creation *after* loading config?
                }

                // Register Tools
                for (name, tool_config) in config.tools {
                    match tool_config {
                        crate::mcp::config::ToolConfig::Command {
                            command,
                            args,
                            env,
                            description,
                        } => {
                            let _tool_name = name.clone();
                            let tool_cmd = command.clone();
                            let tool_args = args.clone();
                            let tool_env = env.clone();

                            let tool = crate::tools::tool::Tool {
                                name: name.clone(),
                                title: None,
                                description: description.or(Some(format!("Run {}", command))),
                                enabled: true,
                                key: None,
                                tags: std::collections::HashSet::new(),
                                meta: None,
                                data: crate::tools::tool::ToolKind::Function(
                                    crate::tools::tool::ToolFunction {
                                        name: name.clone(),
                                        description: None,
                                        input_schema: serde_json::json!({
                                            "type": "object",
                                            "additionalProperties": true
                                        }), // Allow any args for now, passed as JSON string maybe?
                                        output_schema: None,
                                        compiled_schema: None,
                                        fn_handler: std::sync::Arc::new(Box::new(
                                            move |_ctx, args| {
                                                let cmd = tool_cmd.clone();
                                                let t_args = tool_args.clone();
                                                let t_env = tool_env.clone();
                                                Box::pin(async move {
                                            use tokio::process::Command;
                                            let mut child = Command::new(&cmd);
                                            child.args(&t_args);
                                            child.envs(&t_env);
                                            // Pass arguments as JSON to stdin? Or env vars?
                                            // For simple command tools, maybe just arguments.
                                            // Let's pass arguments as a JSON string in FASTMCP_ARGS
                                            child.env("FASTMCP_ARGS", serde_json::to_string(&args).unwrap_or_default());                                            
                                            match child.output().await {
                                                Ok(output) => {
                                                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                                                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                                                    if output.status.success() {
                                                        Ok(crate::tools::tool::ToolResult {
                                                            content: vec![crate::mcp::types::ContentBlock::Text(crate::mcp::types::TextContent {
                                                                type_: "text".to_string(),
                                                                text: stdout,
                                                                annotations: None,
                                                            })],
                                                            structured_content: None,
                                                        })
                                                    } else {
                                                        Err(crate::error::FastMCPError::Tool(crate::error::ErrorData {
                                                            code: Some(1),
                                                            message: format!("Command failed: {}", stderr),
                                                            data: None,
                                                        }))
                                                    }
                                                }
                                                Err(e) => Err(crate::error::FastMCPError::Tool(crate::error::ErrorData {
                                                    code: Some(1),
                                                    message: e.to_string(),
                                                    data: None,
                                                })),
                                            }
                                        }) as std::pin::Pin<Box<dyn std::future::Future<Output = Result<crate::tools::tool::ToolResult, crate::error::FastMCPError>> + Send>>
                                            },
                                        )),
                                    },
                                ),
                            };
                            server
                                .add_tool(tool)
                                .unwrap_or_else(|e| error!("Failed to add tool {}: {}", name, e));
                        }
                    }
                }

                // Register Resources
                for (name, resource_config) in config.resources {
                    match resource_config {
                        crate::mcp::config::ResourceConfig::File { path, mime_type } => {
                            let resource = crate::mcp::types::Resource {
                                uri: name.clone(),
                                description: Some(format!("File: {}", path)),
                                mime_type,
                                tags: None,
                                base_metadata: crate::mcp::types::BaseMetadata {
                                    name: name.clone(),
                                    title: None,
                                },
                                size: None,
                                annotations: None,
                                icons: None,
                            };
                            let file_path = path.clone();
                            let handler = Box::new(
                                move |_uri: String, _ctx: crate::server::context::Context| {
                                    let p = file_path.clone();
                                    Box::pin(async move {
                                        match tokio::fs::read_to_string(&p).await {
                                            Ok(content) => Ok(
                                                crate::resources::types::ResourceResult::from_text(
                                                    content, None,
                                                ),
                                            ),
                                            Err(e) => Err(crate::error::FastMCPError::Resource(
                                                crate::error::ErrorData {
                                                    code: Some(1),
                                                    message: e.to_string(),
                                                    data: None,
                                                },
                                            )),
                                        }
                                    })
                                        as std::pin::Pin<
                                            Box<
                                                dyn std::future::Future<
                                                        Output = Result<
                                                            crate::resources::types::ResourceResult,
                                                            crate::error::FastMCPError,
                                                        >,
                                                    > + Send,
                                            >,
                                        >
                                },
                            );
                            server
                                .add_resource(resource, Some(std::sync::Arc::new(handler)))
                                .unwrap_or_else(|e| {
                                    error!("Failed to add resource {}: {}", name, e)
                                });
                        }
                        crate::mcp::config::ResourceConfig::Text { content, mime_type } => {
                            let resource = crate::mcp::types::Resource {
                                uri: name.clone(),
                                description: Some("Static text".to_string()),
                                mime_type: mime_type.clone(),
                                tags: None,
                                base_metadata: crate::mcp::types::BaseMetadata {
                                    name: name.clone(),
                                    title: None,
                                },
                                size: None,
                                annotations: None,
                                icons: None,
                            };
                            let text_content = content.clone();
                            let handler = Box::new(move |_uri: String, _: crate::server::context::Context| {
                                let t = text_content.clone();
                                let m = mime_type.clone();
                                Box::pin(async move {
                                    Ok(crate::resources::types::ResourceResult::from_text(t, m))
                                })
                                    as std::pin::Pin<
                                        Box<
                                            dyn std::future::Future<
                                                    Output = Result<
                                                        crate::resources::types::ResourceResult,
                                                        crate::error::FastMCPError,
                                                    >,
                                                > + Send,
                                        >,
                                    >
                            });
                            server
                                .add_resource(resource, Some(std::sync::Arc::new(handler)))
                                .unwrap_or_else(|e| {
                                    error!("Failed to add resource {}: {}", name, e)
                                });
                        }
                    }
                }
            }
            Err(e) => error!("Failed to load fastmcp.json: {}", e),
        }
    }

    // Prepare handler and notification receiver
    let handler = std::sync::Arc::new(server.clone());
    let rx = Some(server.subscribe_notifications());

    match transport_type {
        "stdio" => {
            let transport = StdioTransport::new();
            transport.start(handler, rx).await?;
        }
        "sse" | "http" => {
            let transport = HttpTransport::new("127.0.0.1", port);
            transport.start(handler, rx).await?;
        }
        _ => {
            error!("Unknown transport type: {}", transport_type);
            exit(1);
        }
    }
    Ok(())
}

pub async fn client(
    server_url: Option<&str>,
    command: Option<&str>,
    _args: &[String], // Args might be used later if we spawn subprocess
) -> Result<(), Box<dyn std::error::Error>> {
    let transport: Box<dyn crate::client::ClientTransport> = if let Some(url) = server_url {
        if url.starts_with("http") {
            Box::new(SseClientTransport::new(url.to_string(), None))
        } else {
            error!("Invalid server URL for SSE: {}", url);
            exit(1);
        }
    } else if let Some(cmd) = command {
        // Spawn the command
        Box::new(StdioClientTransport::new_process(cmd, _args).map_err(|e| e.to_string())?)
    } else {
        error!("Either --server-url or --command must be provided");
        exit(1);
    };

    let client = Client::new(transport);

    info!("Client connected. Listing tools...");
    match client.list_tools().await {
        Ok(tools) => {
            println!("Tools available:");
            for tool in tools {
                println!("- {}", tool.base_metadata.name);
            }
        }
        Err(e) => error!("Failed to list tools: {}", e),
    }

    Ok(())
}

pub async fn inspect(
    server_url: Option<&str>,
    command: Option<&str>,
    args: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    // Reuse client logic but more inspection
    client(server_url, command, args).await
}

pub async fn dev(
    config_path: Option<&str>,
    npx_path: String,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Determine the command to run the server (ourselves)
    // We want to run this binary with `run` subcommand.
    // If we were launched with `cargo run`, we assume we can just use `cargo run`.
    // But getting `cargo` invocation is tricky.
    // Safest bet is `current_exe()` which points to the binary.

    let current_exe = std::env::current_exe()?;
    let server_cmd = current_exe.to_string_lossy().to_string();

    let mut server_args = vec![
        "run".to_string(),
        "--transport".to_string(),
        "stdio".to_string(),
    ];
    if let Some(cfg) = config_path {
        server_args.push("--config".to_string());
        server_args.push(cfg.to_string());
    }

    info!("Starting MCP Inspector via {}...", npx_path);
    // The inspector expects: npx @modelcontextprotocol/inspector <command> <args...>

    use tokio::process::Command;

    let mut child = Command::new(&npx_path)
        .arg("@modelcontextprotocol/inspector")
        .arg(server_cmd)
        .args(server_args)
        .spawn()
        .map_err(|e| format!("Failed to spawn inspector ({}): {}", npx_path, e))?;

    let status = child.wait().await?;

    if !status.success() {
        return Err("Inspector exited with error".into());
    }

    Ok(())
}

pub async fn version() -> Result<(), Box<dyn std::error::Error>> {
    println!("rs-fast-mcp v{}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
