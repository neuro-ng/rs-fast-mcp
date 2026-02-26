use crate::error::FastMCPError;
use crate::mcp::types::{
    BaseMetadata, Implementation, InitializeRequestParams, InitializeResult, JSONRPC_VERSION,
    JsonRpcRequest, JsonRpcResponse, Prompt as McpPrompt, Resource, ResourceTemplate,
    ServerCapabilities, Tool as McpTool,
};
use crate::prompts::manager::PromptManager;
use crate::prompts::prompt::Prompt;
use crate::resources::manager::{ResourceManager, ResourceReadHandler};
use crate::server::context::Context;
use crate::server::middleware::{Middleware, Next};
use crate::server::strategy::DuplicateStrategy;
use crate::tools::manager::ToolManager;
use crate::tools::tool::Tool;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

/// An async callback invoked during server startup or shutdown.
pub type LifespanHook =
    Box<dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), FastMCPError>> + Send>> + Send + Sync>;

/// High-level MCP server engine.
///
/// `FastMCP` owns the tool, resource, and prompt managers, a middleware
/// pipeline, lifecycle hooks, and tag-based filtering. It is cheap to
/// [`Clone`] because every field is behind an [`Arc`].

#[derive(Clone)]
pub struct FastMCP {
    name: String,
    version: String,
    instructions: Option<String>,
    tools: Arc<ToolManager>,
    resources: Arc<ResourceManager>,
    prompts: Arc<PromptManager>,
    on_startup: Arc<Mutex<Vec<LifespanHook>>>,
    on_shutdown: Arc<Mutex<Vec<LifespanHook>>>,
    middlewares: Arc<Mutex<Vec<Arc<dyn Middleware>>>>,
    include_tags: Arc<Mutex<Vec<String>>>,
    exclude_tags: Arc<Mutex<Vec<String>>>,
    notification_sender: tokio::sync::broadcast::Sender<crate::mcp::types::JsonRpcMessage>,
}

impl std::fmt::Debug for FastMCP {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FastMCP")
            .field("name", &self.name)
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

impl FastMCP {
    /// Creates a new `FastMCP` instance with the given server name and version.
    pub fn new(name: &str, version: &str) -> Self {
        let (tx, _rx) = tokio::sync::broadcast::channel(100);
        Self {
            name: name.to_string(),
            version: version.to_string(),
            instructions: None,
            tools: Arc::new(ToolManager::new()),
            resources: Arc::new(ResourceManager::new()),
            prompts: Arc::new(PromptManager::new()),
            on_startup: Arc::new(Mutex::new(Vec::new())),
            on_shutdown: Arc::new(Mutex::new(Vec::new())),
            middlewares: Arc::new(Mutex::new(Vec::new())),
            include_tags: Arc::new(Mutex::new(Vec::new())),
            exclude_tags: Arc::new(Mutex::new(Vec::new())),
            notification_sender: tx,
        }
    }

    /// Returns a new broadcast receiver for server-sent notifications.
    pub fn subscribe_notifications(
        &self,
    ) -> tokio::sync::broadcast::Receiver<crate::mcp::types::JsonRpcMessage> {
        self.notification_sender.subscribe()
    }

    /// Sends a JSON-RPC notification to all connected transports.
    pub fn send_notification(&self, method: &str, params: Value) -> Result<(), FastMCPError> {
        let msg = crate::mcp::types::JsonRpcMessage::Notification(
            crate::mcp::types::JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method: method.to_string(),
                params: Some(params),
            },
        );
        // Ignore error if no receivers
        let _ = self.notification_sender.send(msg);
        Ok(())
    }

    /// Sets human-readable server instructions returned during initialisation.
    pub fn set_instructions(&mut self, instructions: &str) {
        self.instructions = Some(instructions.to_string());
    }

    /// Configures tag-based filtering for tools, resources, and prompts.
    pub fn set_filtering(&self, include: Vec<String>, exclude: Vec<String>) {
        {
            let mut guard = self.include_tags.lock().unwrap();
            *guard = include;
        }
        {
            let mut guard = self.exclude_tags.lock().unwrap();
            *guard = exclude;
        }
    }

    fn should_include(&self, tags: &[String]) -> bool {
        // Exclude precedence
        {
            let exclude_guard = self.exclude_tags.lock().unwrap();
            for tag in tags {
                if exclude_guard.contains(tag) {
                    return false;
                }
            }
        }

        // Include logic
        {
            let include_guard = self.include_tags.lock().unwrap();
            if !include_guard.is_empty() {
                for tag in tags {
                    if include_guard.contains(tag) {
                        return true;
                    }
                }
                return false;
            }
        }
        // Default include if include list is empty (and not excluded)
        true
    }

    // --- Core Protocol Handling ---

    /// Routes an incoming JSON-RPC request through the middleware pipeline.
    pub async fn handle_request(
        &self,
        request: JsonRpcRequest,
    ) -> Result<JsonRpcResponse, FastMCPError> {
        self.handle_request_with_middleware(request).await
    }

    /// Core request dispatcher (called after middleware).
    async fn handle_request_core(
        &self,
        request: JsonRpcRequest,
    ) -> Result<JsonRpcResponse, FastMCPError> {
        let id = request.id.clone();
        match request.method.as_str() {
            "initialize" => {
                let _params: InitializeRequestParams =
                    serde_json::from_value(request.params.unwrap_or(Value::Null))
                        .map_err(FastMCPError::Json)?;

                let result = InitializeResult {
                    protocol_version: JSONRPC_VERSION.to_string(),
                    capabilities: ServerCapabilities {
                        tools: Some(crate::mcp::types::ToolsCapability {
                            list_changed: Some(true),
                        }),
                        resources: Some(crate::mcp::types::ResourcesCapability {
                            list_changed: Some(true),
                            subscribe: Some(true),
                        }),
                        prompts: Some(crate::mcp::types::PromptsCapability {
                            list_changed: Some(true),
                        }),
                        logging: None,
                        experimental: None,
                    },
                    server_info: Implementation {
                        version: self.version.clone(),
                        website_url: None,
                        icons: None,
                        base_metadata: BaseMetadata {
                            name: self.name.clone(),
                            title: None,
                        },
                    },
                    instructions: self.instructions.clone(),
                };

                Ok(JsonRpcResponse::new(
                    id,
                    serde_json::to_value(result).map_err(FastMCPError::Json)?,
                ))
            }
            "notifications/initialized" => Ok(JsonRpcResponse::new(id, Value::Null)),
            "ping" => Ok(JsonRpcResponse::new(id, Value::String("pong".to_string()))),
            "tools/list" => {
                let tools = self.list_tools();
                let mcp_tools: Vec<McpTool> = tools
                    .into_iter()
                    .filter(|t| {
                        let tags: Vec<String> = t.tags.iter().cloned().collect();
                        self.should_include(&tags)
                    })
                    .map(|t| McpTool {
                        base_metadata: BaseMetadata {
                            name: t.name,
                            title: t.title,
                        },
                        description: t.description,
                        input_schema: match &t.data {
                            crate::tools::tool::ToolKind::Function(f) => f.input_schema.clone(),
                            _ => serde_json::json!({}),
                        },
                        output_schema: match &t.data {
                            crate::tools::tool::ToolKind::Function(f) => f.output_schema.clone(),
                            _ => None,
                        },
                        icons: None,
                    })
                    .collect();

                let result = serde_json::json!({
                    "tools": mcp_tools
                });
                Ok(JsonRpcResponse::new(id, result))
            }
            "tools/call" => {
                let params = request.params.unwrap_or(Value::Null);
                let name = params.get("name").and_then(|v| v.as_str()).ok_or(
                    FastMCPError::InvalidRequest("Missing tool name".to_string()),
                )?;
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                let context = Context::default();
                let result = self.tools.call_tool(name, arguments, context).await?;

                let response_val = serde_json::to_value(result).map_err(FastMCPError::Json)?;
                Ok(JsonRpcResponse::new(id, response_val))
            }
            "resources/list" => {
                let resources = self.list_resources();
                let filtered_resources: Vec<Resource> = resources
                    .into_iter()
                    .filter(|r| self.should_include(r.tags.as_deref().unwrap_or(&[])))
                    .collect();
                let result = serde_json::json!({
                    "resources": filtered_resources
                });
                Ok(JsonRpcResponse::new(id, result))
            }
            "resources/templates/list" => {
                let templates = self.resources.list_templates();
                let result = serde_json::json!({
                    "resourceTemplates": templates
                });
                Ok(JsonRpcResponse::new(id, result))
            }
            "resources/subscribe" => {
                let params = request.params.unwrap_or(Value::Null);
                let uri = params.get("uri").and_then(|v| v.as_str()).ok_or(
                    FastMCPError::InvalidRequest("Missing resource uri".to_string()),
                )?;
                // Session ID is not yet carried through the request; use a placeholder.
                let session_id = Some("default_session".to_string());
                self.resources.subscribe(uri.to_string(), session_id);
                Ok(JsonRpcResponse::new(id, Value::Null))
            }
            "resources/unsubscribe" => {
                let params = request.params.unwrap_or(Value::Null);
                let uri = params.get("uri").and_then(|v| v.as_str()).ok_or(
                    FastMCPError::InvalidRequest("Missing resource uri".to_string()),
                )?;
                let session_id = Some("default_session".to_string());
                self.resources.unsubscribe(uri.to_string(), session_id);
                Ok(JsonRpcResponse::new(id, Value::Null))
            }
            "resources/read" => {
                let params = request.params.unwrap_or(Value::Null);
                let uri = params.get("uri").and_then(|v| v.as_str()).ok_or(
                    FastMCPError::InvalidRequest("Missing resource uri".to_string()),
                )?;

                let context = Context::default();
                let contents = self.resources.read_resource(uri, context).await?;

                let result = serde_json::json!({
                    "contents": contents
                });
                Ok(JsonRpcResponse::new(id, result))
            }
            "prompts/list" => {
                let prompts = self.list_prompts();
                let mcp_prompts: Vec<McpPrompt> = prompts
                    .into_iter()
                    .filter(|p| {
                        let tags: Vec<String> = p.tags.iter().cloned().collect();
                        self.should_include(&tags)
                    })
                    .map(|p| McpPrompt {
                        description: p.description,
                        arguments: p.data.arguments.clone().map(|args| {
                            args.into_iter()
                                .map(|a| crate::mcp::types::PromptArgument {
                                    name: a.name,
                                    description: a.description,
                                    required: a.required,
                                })
                                .collect()
                        }),
                        icons: None,
                        tags: if p.tags.is_empty() {
                            None
                        } else {
                            Some(p.tags.into_iter().collect())
                        },
                        base_metadata: BaseMetadata {
                            name: p.name,
                            title: p.title,
                        },
                    })
                    .collect();

                let result = serde_json::json!({
                    "prompts": mcp_prompts
                });
                Ok(JsonRpcResponse::new(id, result))
            }
            "prompts/get" => {
                let params = request.params.unwrap_or(Value::Null);
                let name = params.get("name").and_then(|v| v.as_str()).ok_or(
                    FastMCPError::InvalidRequest("Missing prompt name".to_string()),
                )?;
                let arguments_val = params.get("arguments").cloned();

                let arguments: Option<HashMap<String, Value>> = if let Some(v) = arguments_val {
                    serde_json::from_value(v).map_err(FastMCPError::Json)?
                } else {
                    None
                };

                let (description, messages) =
                    self.prompts.get_prompt_execution(name, arguments).await?;

                let result = serde_json::json!({
                    "description": description,
                    "messages": messages
                });
                Ok(JsonRpcResponse::new(id, result))
            }
            _ => Err(FastMCPError::InvalidRequest("Method not found".to_string())),
        }
    }

    // --- Tools ---

    /// Registers a tool with the server.
    ///
    /// Sends a `notifications/tools/list_changed` notification to connected clients.
    pub fn add_tool(&self, tool: Tool) -> Result<(), FastMCPError> {
        self.tools.register(tool)?;
        let _ = self.send_notification("notifications/tools/list_changed", serde_json::json!({}));
        Ok(())
    }

    /// Sets the duplicate-tool registration strategy.
    pub fn set_tool_strategy(&self, strategy: DuplicateStrategy) {
        self.tools.set_strategy(strategy);
    }

    /// Removes a registered tool by name.
    pub fn remove_tool(&self, name: &str) {
        self.tools.remove_tool(name);
        let _ = self.send_notification("notifications/tools/list_changed", serde_json::json!({}));
    }

    /// Returns all registered tools.
    pub fn list_tools(&self) -> Vec<Tool> {
        self.tools.list_tools()
    }

    /// Looks up a tool by name.
    pub fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tools.get_tool(name)
    }

    /// Returns the number of times a tool has been called.
    pub fn get_tool_usage(&self, name: &str) -> Option<usize> {
        self.tools.get_usage(name)
    }

    // --- Resources ---

    /// Registers a resource with an optional read handler.
    ///
    /// Sends a `notifications/resources/list_changed` notification to connected clients.
    pub fn add_resource(
        &self,
        resource: Resource,
        handler: Option<Arc<ResourceReadHandler>>,
    ) -> Result<(), FastMCPError> {
        self.resources.register(resource, handler)?;
        let _ = self.send_notification(
            "notifications/resources/list_changed",
            serde_json::json!({}),
        );
        Ok(())
    }

    /// Registers a URI template with a read handler for dynamic resources.
    pub fn add_resource_template(
        &self,
        template: ResourceTemplate,
        handler: Arc<ResourceReadHandler>,
    ) -> Result<(), FastMCPError> {
        self.resources.register_template(template, handler)?;
        Ok(())
    }

    /// Sets the duplicate-resource registration strategy.
    pub fn set_resource_strategy(&self, strategy: DuplicateStrategy) {
        self.resources.set_strategy(strategy);
    }

    /// Removes a registered resource by URI.
    pub fn remove_resource(&self, uri: &str) {
        self.resources.remove_resource(uri);
        let _ = self.send_notification(
            "notifications/resources/list_changed",
            serde_json::json!({}),
        );
    }

    /// Returns all registered resources.
    pub fn list_resources(&self) -> Vec<Resource> {
        self.resources.list_resources()
    }

    /// Looks up a resource by URI.
    pub fn get_resource(&self, uri: &str) -> Option<Resource> {
        self.resources.get_resource(uri)
    }

    /// Returns the number of times a resource has been read.
    pub fn get_resource_usage(&self, uri: &str) -> Option<usize> {
        self.resources.get_usage(uri)
    }

    // --- Prompts ---

    /// Registers a prompt template with the server.
    pub fn add_prompt(&self, prompt: Prompt) -> Result<(), FastMCPError> {
        self.prompts.register(prompt)
    }

    /// Sets the duplicate-prompt registration strategy.
    pub fn set_prompt_strategy(&self, strategy: DuplicateStrategy) {
        self.prompts.set_strategy(strategy);
    }

    /// Removes a registered prompt by name.
    pub fn remove_prompt(&self, name: &str) {
        self.prompts.remove_prompt(name);
    }

    /// Returns all registered prompts.
    pub fn list_prompts(&self) -> Vec<Prompt> {
        self.prompts.list_prompts()
    }

    /// Looks up a prompt by name.
    pub fn get_prompt(&self, name: &str) -> Option<Prompt> {
        self.prompts.get_prompt(name)
    }

    // --- Middleware ---

    // Compose middleware execution
    async fn handle_request_with_middleware(
        &self,
        request: JsonRpcRequest,
    ) -> Result<JsonRpcResponse, FastMCPError> {
        let middlewares = {
            let guard = self.middlewares.lock().unwrap();
            guard.clone()
        };

        let this = self.clone();
        let mut next: Next =
            Box::new(move |req| Box::pin(async move { this.handle_request_core(req).await }));

        for middleware in middlewares.iter().rev() {
            let m = middleware.clone();
            let n = next;
            next = Box::new(move |req| {
                let m = m.clone();
                Box::pin(async move { m.handle(req, n).await })
            });
        }

        next(request).await
    }

    /// Adds a middleware to the request processing pipeline.
    pub fn add_middleware<M: Middleware>(&self, middleware: M) {
        let mut guard = self.middlewares.lock().unwrap();
        guard.push(Arc::new(middleware));
    }

    /// Adds a pre-wrapped `Arc<dyn Middleware>` to the pipeline.
    pub fn add_middleware_arc(&self, middleware: Arc<dyn Middleware>) {
        let mut guard = self.middlewares.lock().unwrap();
        guard.push(middleware);
    }

    // --- Lifespan Hooks ---

    /// Registers a callback to run during server startup.
    pub fn add_startup_hook<F>(&self, hook: F)
    where
        F: Fn() -> Pin<Box<dyn Future<Output = Result<(), FastMCPError>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        let mut hooks = self.on_startup.lock().unwrap();
        hooks.push(Box::new(hook));
    }

    /// Registers a callback to run during server shutdown.
    pub fn add_shutdown_hook<F>(&self, hook: F)
    where
        F: Fn() -> Pin<Box<dyn Future<Output = Result<(), FastMCPError>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        let mut hooks = self.on_shutdown.lock().unwrap();
        hooks.push(Box::new(hook));
    }

    /// Runs all registered startup hooks in order.
    pub async fn run_startup(&self) -> Result<(), FastMCPError> {
        let futures = {
            let guard = self.on_startup.lock().unwrap();
            let mut futures = Vec::new();
            for hook in guard.iter() {
                futures.push(hook());
            }
            futures
        };

        for future in futures {
            future.await?;
        }
        Ok(())
    }

    /// Runs all registered shutdown hooks in order.
    pub async fn run_shutdown(&self) -> Result<(), FastMCPError> {
        let futures = {
            let guard = self.on_shutdown.lock().unwrap();
            let mut futures = Vec::new();
            for hook in guard.iter() {
                futures.push(hook());
            }
            futures
        };

        for future in futures {
            future.await?;
        }
        Ok(())
    }
}

/// Thread-safe, `Arc`-wrapped [`FastMCP`] server.
///
/// This is the primary handle used by transports. It implements
/// [`RequestHandler`].
#[derive(Clone, Debug)]
pub struct FastMCPServer(Arc<FastMCP>);

impl FastMCPServer {
    /// Creates a new server instance wrapping a [`FastMCP`] engine.
    pub fn new(name: &str, version: &str) -> Self {
        Self(Arc::new(FastMCP::new(name, version)))
    }

    /// Adds a middleware to the pipeline.
    pub fn add_middleware<M: Middleware>(&self, middleware: M) {
        self.0.add_middleware(middleware)
    }

    /// Adds a pre-wrapped `Arc<dyn Middleware>` to the pipeline.
    pub fn add_middleware_arc(&self, middleware: Arc<dyn Middleware>) {
        self.0.add_middleware_arc(middleware)
    }

    /// Returns a broadcast receiver for server-sent notifications.
    pub fn subscribe_notifications(
        &self,
    ) -> tokio::sync::broadcast::Receiver<crate::mcp::types::JsonRpcMessage> {
        self.0.subscribe_notifications()
    }

    /// Sends a notification to all connected transports.
    pub fn send_notification(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<(), FastMCPError> {
        self.0.send_notification(method, params)
    }

    /// Configures tag-based filtering.
    pub fn set_filtering(&self, include: Vec<String>, exclude: Vec<String>) {
        self.0.set_filtering(include, exclude);
    }

    // Delegate methods
    pub fn add_tool(&self, tool: Tool) -> Result<(), FastMCPError> {
        self.0.add_tool(tool)
    }

    /// Sets the duplicate-tool registration strategy.
    pub fn set_tool_strategy(&self, strategy: DuplicateStrategy) {
        self.0.set_tool_strategy(strategy);
    }

    /// Returns all registered tools.
    pub fn list_tools(&self) -> Vec<Tool> {
        self.0.list_tools()
    }

    /// Returns the number of times a tool has been called.
    pub fn get_tool_usage(&self, name: &str) -> Option<usize> {
        self.0.get_tool_usage(name)
    }

    /// Registers a resource with an optional read handler.
    pub fn add_resource(
        &self,
        resource: Resource,
        handler: Option<Arc<ResourceReadHandler>>,
    ) -> Result<(), FastMCPError> {
        self.0.add_resource(resource, handler)
    }

    /// Registers a URI template with a read handler for dynamic resources.
    pub fn add_resource_template(
        &self,
        template: ResourceTemplate,
        handler: Arc<ResourceReadHandler>,
    ) -> Result<(), FastMCPError> {
        self.0.add_resource_template(template, handler)
    }

    /// Sets the duplicate-resource registration strategy.
    pub fn set_resource_strategy(&self, strategy: DuplicateStrategy) {
        self.0.set_resource_strategy(strategy);
    }

    /// Returns the number of times a resource has been read.
    pub fn get_resource_usage(&self, uri: &str) -> Option<usize> {
        self.0.get_resource_usage(uri)
    }

    /// Registers a prompt template.
    pub fn add_prompt(&self, prompt: Prompt) -> Result<(), FastMCPError> {
        self.0.add_prompt(prompt)
    }

    /// Sets the duplicate-prompt registration strategy.
    pub fn set_prompt_strategy(&self, strategy: DuplicateStrategy) {
        self.0.set_prompt_strategy(strategy);
    }

    /// Registers a callback to run during server startup.
    pub fn add_startup_hook<F>(&self, hook: F)
    where
        F: Fn() -> Pin<Box<dyn Future<Output = Result<(), FastMCPError>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        self.0.add_startup_hook(hook)
    }

    /// Registers a callback to run during server shutdown.
    pub fn add_shutdown_hook<F>(&self, hook: F)
    where
        F: Fn() -> Pin<Box<dyn Future<Output = Result<(), FastMCPError>> + Send>>
            + Send
            + Sync
            + 'static,
    {
        self.0.add_shutdown_hook(hook)
    }

    /// Routes an incoming JSON-RPC request through the middleware pipeline.
    pub async fn handle_request(
        &self,
        request: JsonRpcRequest,
    ) -> Result<JsonRpcResponse, FastMCPError> {
        self.0.handle_request(request).await
    }

    /// Runs all registered startup hooks in order.
    pub async fn run_startup(&self) -> Result<(), FastMCPError> {
        self.0.run_startup().await
    }

    /// Runs all registered shutdown hooks in order.
    pub async fn run_shutdown(&self) -> Result<(), FastMCPError> {
        self.0.run_shutdown().await
    }
}

use crate::server::transport::RequestHandler;
use async_trait::async_trait;

#[async_trait]
impl RequestHandler for FastMCPServer {
    async fn handle_request(
        &self,
        request: JsonRpcRequest,
    ) -> Result<JsonRpcResponse, FastMCPError> {
        self.handle_request(request).await
    }

    async fn handle_notification(
        &self,
        notification: crate::mcp::types::JsonRpcNotification,
    ) -> Result<(), FastMCPError> {
        match notification.method.as_str() {
            "notifications/initialized" => {
                tracing::info!("Client initialized");
            }
            "notifications/resources/updated" => {
                tracing::debug!("Received resources/updated notification");
            }
            "notifications/tools/list_changed" => {
                tracing::debug!("Received tools/list_changed notification");
            }
            _ => {
                tracing::debug!("Received unknown notification: {}", notification.method);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::types::{BaseMetadata, RequestId};
    use serde_json::json;

    fn create_test_base_metadata(name: &str) -> BaseMetadata {
        BaseMetadata {
            name: name.to_string(),
            title: None,
        }
    }

    // Tests use internal Tool component
    fn create_test_tool(name: &str) -> Tool {
        use crate::tools::tool::{ToolFunction, ToolKind};
        Tool {
            name: name.to_string(),
            description: Some("Test tool".to_string()),
            data: ToolKind::Function(ToolFunction {
                name: name.to_string(),
                description: None,
                input_schema: json!({ "type": "object" }),
                output_schema: None,
                fn_handler: Arc::new(Box::new(|_, _| {
                    Box::pin(async { Err(crate::error::FastMCPError::new("not impl".to_string())) })
                        as std::pin::Pin<
                            Box<
                                dyn std::future::Future<
                                        Output = Result<
                                            crate::tools::tool::ToolResult,
                                            crate::error::FastMCPError,
                                        >,
                                    > + Send,
                            >,
                        >
                }) as crate::tools::tool::ToolHandler),
                compiled_schema: None,
            }),
            enabled: true,
            key: None,
            title: None,
            meta: None,
            tags: std::collections::HashSet::new(),
        }
    }

    #[test]
    fn test_tool_management() {
        let server = FastMCP::new("test", "1.0");

        server.add_tool(create_test_tool("tool1")).unwrap();
        assert_eq!(server.list_tools().len(), 1);

        let t = server.get_tool("tool1").unwrap();
        assert_eq!(t.name, "tool1");

        server.remove_tool("tool1");
        assert_eq!(server.list_tools().len(), 0);
    }

    #[test]
    fn test_resource_management() {
        let server = FastMCP::new("test", "1.0");
        let resource = Resource {
            uri: "file:///test".to_string(),
            base_metadata: create_test_base_metadata("test"),
            description: None,
            mime_type: Some("text/plain".to_string()),
            annotations: None,
            size: None,
            icons: None,
            tags: None,
        };

        server.add_resource(resource, None).unwrap();
        assert_eq!(server.list_resources().len(), 1);
        assert!(server.get_resource("file:///test").is_some());
    }

    #[test]
    fn test_prompt_management() {
        let server = FastMCP::new("test", "1.0");

        // Construct Prompt (Component) manually
        use crate::prompts::prompt::{Prompt as PromptComponent, PromptFunction};

        let prompt_func = PromptFunction {
            name: "test_prompt".to_string(),
            description: None,
            arguments: None,
            fn_handler: Arc::new(Box::new(|_| {
                Box::pin(async { Ok(vec![]) })
                    as std::pin::Pin<
                        Box<
                            dyn std::future::Future<
                                    Output = Result<
                                        Vec<crate::prompts::prompt::PromptMessage>,
                                        crate::error::FastMCPError,
                                    >,
                                > + Send,
                        >,
                    >
            }) as crate::prompts::prompt::PromptHandler),
        };

        let prompt = PromptComponent {
            name: "test_prompt".to_string(),
            description: None,
            data: prompt_func,
            title: None,
            enabled: true,
            key: None,
            meta: None,
            tags: std::collections::HashSet::new(),
        };

        server.add_prompt(prompt).unwrap();
        assert_eq!(server.list_prompts().len(), 1);
        assert!(server.get_prompt("test_prompt").is_some());
    }

    #[tokio::test]
    async fn test_handle_request_initialize() {
        let server = FastMCP::new("test-server", "1.0.0");
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            params: Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0"
                }
            })),
            id: RequestId::Int(1),
            transport_metadata: None,
        };

        let response = server
            .handle_request(request)
            .await
            .expect("Request failed");
        assert_eq!(response.id, RequestId::Int(1));

        let result: InitializeResult = serde_json::from_value(response.result).unwrap();
        assert_eq!(result.server_info.base_metadata.name, "test-server");
    }

    #[tokio::test]
    async fn test_tool_execution() {
        use crate::tools::tool::{ToolFunction, ToolKind};
        use std::future::Future;
        use std::pin::Pin;

        let server = FastMCP::new("test", "1.0");

        let handler = Arc::new(Box::new(|_ctx, _args| {
            Box::pin(async {
                Ok(crate::tools::tool::ToolResult {
                    content: vec![],
                    structured_content: Some(json!({ "result": "success" })),
                })
            })
                as Pin<
                    Box<
                        dyn Future<Output = Result<crate::tools::tool::ToolResult, FastMCPError>>
                            + Send,
                    >,
                >
        }) as crate::tools::tool::ToolHandler);

        let tool = Tool {
            name: "test_exec".to_string(),
            description: None,
            data: ToolKind::Function(ToolFunction {
                name: "test_exec".to_string(),
                description: None,
                input_schema: json!({}),
                output_schema: None,
                fn_handler: handler,
                compiled_schema: None,
            }),
            enabled: true,
            key: None,
            title: None,
            meta: None,
            tags: std::collections::HashSet::new(),
        };

        server.add_tool(tool).unwrap();

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "test_exec",
                "arguments": {}
            })),
            id: RequestId::Int(2),
            transport_metadata: None,
        };

        let response = server.handle_request(request).await.expect("Call failed");

        let result = response.result.as_object().unwrap();
        assert!(result.get("structured_content").is_some());
    }

    #[tokio::test]
    async fn test_resource_read() {
        use crate::mcp::types::ResourceContents;
        use std::future::Future;
        use std::pin::Pin;

        let server = FastMCP::new("test", "1.0");

        let handler = Arc::new(Box::new(|_uri, _ctx| {
            Box::pin(async {
                Ok(vec![ResourceContents {
                    uri: "file:///test".to_string(),
                    mime_type: Some("text/plain".to_string()),
                    text: Some("Hello World".to_string()),
                    blob: None,
                }])
            })
                as Pin<Box<dyn Future<Output = Result<Vec<ResourceContents>, FastMCPError>> + Send>>
        }) as crate::resources::manager::ResourceReadHandler);

        let resource = Resource {
            uri: "file:///test".to_string(),
            base_metadata: create_test_base_metadata("test_res"),
            description: None,
            mime_type: Some("text/plain".to_string()),
            annotations: None,
            size: None,
            icons: None,
            tags: None,
        };

        server.add_resource(resource, Some(handler)).unwrap();

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "resources/read".to_string(),
            params: Some(json!({
                "uri": "file:///test"
            })),
            id: RequestId::Int(3),
            transport_metadata: None,
        };

        let response = server.handle_request(request).await.expect("Read failed");

        let result = response.result.as_object().unwrap();
        let contents = result.get("contents").unwrap().as_array().unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(
            contents[0].get("text").unwrap().as_str().unwrap(),
            "Hello World"
        );
    }

    #[tokio::test]
    async fn test_prompt_execution() {
        use crate::prompts::prompt::{Prompt as PromptComponent, PromptFunction, PromptMessage};
        use std::future::Future;
        use std::pin::Pin;

        let server = FastMCP::new("test", "1.0");

        let handler = Arc::new(Box::new(|_args| {
            Box::pin(async {
                Ok(vec![PromptMessage {
                    role: "user".to_string(),
                    content: crate::mcp::types::ContentBlock::Text(
                        crate::mcp::types::TextContent {
                            text: "Hello Prompt".to_string(),
                            type_: "text".to_string(),
                            annotations: None,
                        },
                    ),
                }])
            })
                as Pin<Box<dyn Future<Output = Result<Vec<PromptMessage>, FastMCPError>> + Send>>
        }) as crate::prompts::prompt::PromptHandler);

        let prompt = PromptComponent {
            name: "test_prompt".to_string(),
            description: None,
            data: PromptFunction {
                name: "test_prompt".to_string(),
                description: None,
                arguments: None,
                fn_handler: handler,
            },
            title: None,
            enabled: true,
            key: None,
            meta: None,
            tags: std::collections::HashSet::new(),
        };

        server.add_prompt(prompt).unwrap();

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "prompts/get".to_string(),
            params: Some(json!({
                "name": "test_prompt"
            })),
            id: RequestId::Int(4),
            transport_metadata: None,
        };

        let response = server
            .handle_request(request)
            .await
            .expect("Execution failed");

        let result = response.result.as_object().unwrap();
        let messages = result.get("messages").unwrap().as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].get("role").unwrap().as_str().unwrap(), "user");
    }
}
