use actix_web::{HttpResponse, Responder, web};
use rs_fast_mcp::mcp::types::ContentBlock;
use rs_fast_mcp::server::{builder::ServerBuilder, transport::http::HttpTransport};
use rs_fast_mcp::tools::tool::{Tool, ToolResult};
use serde_json::json;
use std::env;

async fn dcr_metadata() -> impl Responder {
    let resource_url =
        env::var("RESOURCE_URL").unwrap_or_else(|_| "http://localhost:8000".to_string());
    let auth_server =
        env::var("AUTH_SERVER").unwrap_or_else(|_| "https://auth.workos.com".to_string());

    HttpResponse::Ok().json(json!({
        "resource": resource_url,
        "authorization_servers": [auth_server],
        "scopes_supported": ["email", "profile"],
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    rs_fast_mcp::server::logging::init_logging("info").expect("Failed to initialize logging");

    // 1. Create Transport with Custom Route
    let transport = HttpTransport::new("127.0.0.1", 8000).with_app_config(|cfg| {
        cfg.route(
            "/.well-known/oauth-protected-resource",
            web::get().to(dcr_metadata),
        );
    });

    // 2. Build Server
    let server = ServerBuilder::new("authkit-dcr", "1.0.0")
        .with_transport(Box::new(transport))
        .build();

    // 3. Define and Register Tool
    let echo_tool = Tool::new("echo", "Echo message")
        .add_parameter("message", "string", "Message to echo")
        .with_handler(Box::new(|_ctx, args| {
            Box::pin(async move {
                let msg = args
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("No message");
                Ok(ToolResult {
                    content: vec![ContentBlock::Text(rs_fast_mcp::mcp::types::TextContent {
                        type_: "text".to_string(),
                        text: format!("Echo: {}", msg),
                        annotations: None,
                    })],
                    structured_content: Some(json!({ "message": msg })),
                })
            })
        }));

    server.core.add_tool(echo_tool)?;

    println!("Starting AuthKit DCR Server on 127.0.0.1:8000");
    println!(
        "DCR Metadata available at: http://127.0.0.1:8000/.well-known/oauth-protected-resource"
    );

    // 4. Run
    server.run().await?;
    Ok(())
}
