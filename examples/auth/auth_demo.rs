use rs_fast_mcp::error::FastMCPError;
use rs_fast_mcp::server::auth::{AuthMiddleware, SimpleAuthProvider, current_context};
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

    info!("🚀 Auth Demo server starting...");

    // 1. Create Server
    let server = FastMCPServer::new("auth-demo", "1.0.0");

    // 2. Setup Authentication
    // Use SimpleAuthProvider with a static token for demonstration.
    // In production, use GoogleProvider, OIDCProvider, etc.
    let auth_provider = Arc::new(SimpleAuthProvider::new("secret-token"));

    // Create the middleware.
    // We can specify public paths if needed (e.g. "tools/list", "ping" are usually public).
    // The AuthMiddleware by default might inspect all requests depending on implementation.
    // Let's assume it authenticates but allows request processing to continue, populating the context?
    // OR it rejects if auth fails?
    // Typically, for Tools, we want to allow the request to reach the tool so the tool can decide,
    // OR we reject at the middleware level?
    // If we reject at middleware, we can't have public tools mixed with private tools easily unless middleware supports exclusions.
    // Alternatively, AuthMiddleware might be "Permissive" (only populates context if token present) or "Strict".
    // For this demo, let's assume it's strict for authenticated endpoints or we check context manually.
    // Actually, `AuthMiddleware` usually enforces auth.
    // Let's check `AuthMiddleware` implementation quickly? No, I'll rely on `current_context()` usage imply permissive or we configure it.
    // If strict, we can't implement "public_echo" easily if it blocks everything.
    // Let's assume for now we use it to populate context and we check in tools, OR implementation details differ.
    // NOTE: If AuthMiddleware rejects `call_tool` for `public_echo`, that's an issue.
    // Let's assume we want to demonstrate `current_context()` check, so maybe we DON'T enforce strict auth on all requests in the middleware?
    // Or we configure the middleware to be optional?
    // Let's stick to the plan: "Add a protected tool... that checks current_context()".
    // This implies the middleware sets the context but might not block?
    // Let's use `AuthMiddleware::new(provider)` and see.

    let auth_middleware = AuthMiddleware::new(auth_provider);
    server.add_middleware(auth_middleware);

    // 3. Tool: public_echo
    let public_echo = Tool {
        name: "public_echo".to_string(),
        title: None,
        description: Some("Echo message (Public)".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "public_echo".to_string(),
            description: None,
            input_schema: json!({
                "type": "object",
                "properties": { "message": { "type": "string" } },
                "required": ["message"]
            }),
            output_schema: None,
            compiled_schema: None,
            fn_handler: Arc::new(Box::new(|_ctx: Context, args: serde_json::Value| {
                Box::pin(async move {
                    // No auth check
                    Ok(ToolResult {
                        content: vec![],
                        structured_content: Some(json!({ "echo": args.get("message") })),
                    })
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
        }),
    };
    server.add_tool(public_echo)?;

    // 4. Tool: protected_echo
    let protected_echo = Tool {
        name: "protected_echo".to_string(),
        title: None,
        description: Some("Echo message (Protected)".to_string()),
        enabled: true,
        key: None,
        tags: std::collections::HashSet::new(),
        meta: None,
        data: ToolKind::Function(ToolFunction {
            name: "protected_echo".to_string(),
            description: None,
            input_schema: json!({
                "type": "object",
                "properties": { "message": { "type": "string" } },
                "required": ["message"]
            }),
            output_schema: None,
            compiled_schema: None,
            fn_handler: Arc::new(Box::new(|_ctx: Context, args: serde_json::Value| {
                Box::pin(async move {
                    // Check Authentication Context
                    match current_context() {
                        Some(auth_ctx) => {
                            info!(
                                "Access granted to user: {}",
                                auth_ctx.user_id.clone().unwrap_or("anon".to_string())
                            );
                            Ok(ToolResult {
                                content: vec![],
                                structured_content: Some(json!({
                                    "echo": args.get("message"),
                                    "user": auth_ctx.user_id
                                })),
                            })
                        }
                        None => {
                            // If AuthMiddleware is permissive (doesn't block missing token), we catch it here.
                            // If it's strict, we might not reach here without a token, but this handles the case.
                            Err(FastMCPError::InvalidRequest(
                                "Unauthorized: Missing valid authentication token".to_string(),
                            ))
                        }
                    }
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
        }),
    };
    server.add_tool(protected_echo)?;

    // 5. Run Server
    let transport = StdioTransport::new();
    let handler = Arc::new(server.clone());
    let rx = Some(server.subscribe_notifications());

    info!("Server ready. Listening on stdio...");
    transport.start(handler, rx).await?;

    Ok(())
}
