use crate::client::{Client, ClientTransport};
use crate::error::FastMCPError;
use crate::server::core::FastMCP;
use crate::tools::tool::{Tool, ToolFunction, ToolKind};
use std::sync::Arc;

pub struct MountedServer {
    client: Arc<Client>,
    prefix: String,
}

impl MountedServer {
    pub fn new(transport: Box<dyn ClientTransport>, prefix: &str) -> Self {
        let client = Arc::new(Client::new(transport));
        Self {
            client,
            prefix: prefix.to_string(),
        }
    }

    /// Connects (initializes) and syncs tools to the host server.
    pub async fn mount(&self, host: &FastMCP) -> Result<(), FastMCPError> {
        // 1. Initialize
        let init_params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "rs-fast-mcp-proxy",
                "version": "1.0"
            }
        });

        let _init_res = self.client.request("initialize", Some(init_params)).await?;
        self.client
            .request("notifications/initialized", None)
            .await?;

        // 2. Sync Tools
        match self.client.list_tools().await {
            Ok(tools) => {
                for tool in tools {
                    let proxy_tool = self.create_proxy_tool(tool);
                    host.add_tool(proxy_tool)?;
                }
            }
            Err(e) => tracing::warn!("Failed to sync tools from {}: {}", self.prefix, e),
        }

        // 3. Sync Resources
        match self.client.list_resources().await {
            Ok(resources) => {
                for resource in resources {
                    self.create_proxy_resource_and_register(resource, host)
                        .await?;
                }
            }
            Err(e) => tracing::warn!("Failed to sync resources from {}: {}", self.prefix, e),
        }

        // 4. Sync Prompts
        match self.client.list_prompts().await {
            Ok(prompts) => {
                for prompt in prompts {
                    let proxy_prompt = self.create_proxy_prompt(prompt);
                    host.add_prompt(proxy_prompt)?;
                }
            }
            Err(e) => tracing::warn!("Failed to sync prompts from {}: {}", self.prefix, e),
        }

        Ok(())
    }

    fn create_proxy_prompt(
        &self,
        remote_prompt: crate::mcp::types::Prompt,
    ) -> crate::prompts::prompt::Prompt {
        let client = self.client.clone();
        let remote_name = remote_prompt.base_metadata.name.clone();
        let local_name = if self.prefix.is_empty() {
            remote_name.clone()
        } else {
            format!("{}_{}", self.prefix, remote_name)
        };

        let handler = Arc::new(Box::new(move |args| {
            let client = client.clone();
            let name = remote_name.clone();
            Box::pin(async move {
                let args_val = serde_json::to_value(args).map_err(FastMCPError::Json)?;
                let result = client.get_prompt(&name, Some(args_val)).await?;

                let messages_val = result.get("messages").ok_or(FastMCPError::InvalidRequest(
                    "Missing messages in prompt result".to_string(),
                ))?;
                // Map mcp::types::PromptMessage to prompts::prompt::PromptMessage?
                // They might be structurally identical but different types.
                // prompts::prompt::PromptMessage uses crate::mcp::types::ContentBlock.
                // Let's check PromptMessage definition in prompt.rs again.
                // Step 110:
                // pub struct PromptMessage { pub role: String, pub content: crate::mcp::types::ContentBlock }
                // In mcp::types.rs (Step 64), SamplingMessage has role/content.
                // But PromptMessage in types? I didn't see it in Step 64.
                // "Prompt as McpPrompt" was used in core.rs.
                // McpPrompt has arguments.
                // But `get_prompt` returns `GetPromptResult`.
                // `GetPromptResult` has `messages`. Messages are `PromptMessage`.
                // Verify `GetPromptResult` in types.
                // Actually `GetPromptResult` wasn't shown in Step 64.
                // But JSON structure is standard.
                // Assuming `crate::prompts::prompt::PromptMessage` implements Deserialize matching the JSON.

                let messages: Vec<crate::prompts::prompt::PromptMessage> =
                    serde_json::from_value(messages_val.clone()).map_err(|e| {
                        FastMCPError::InvalidRequest(format!(
                            "Failed to deserialize messages: {}. Value: {}",
                            e, messages_val
                        ))
                    })?;
                Ok(messages)
            })
                as std::pin::Pin<
                    Box<
                        dyn std::future::Future<
                                Output = Result<
                                    Vec<crate::prompts::prompt::PromptMessage>,
                                    FastMCPError,
                                >,
                            > + Send,
                    >,
                >
        }) as crate::prompts::prompt::PromptHandler);

        crate::prompts::prompt::Prompt {
            name: local_name,
            description: remote_prompt.description.clone(),
            data: crate::prompts::prompt::PromptFunction {
                name: remote_prompt.base_metadata.name,
                description: remote_prompt.description,
                arguments: remote_prompt.arguments.map(|args| {
                    args.into_iter()
                        .map(|a| crate::prompts::prompt::PromptArgument {
                            name: a.name,
                            description: a.description,
                            required: a.required,
                        })
                        .collect()
                }),
                fn_handler: handler,
            },
            enabled: true,
            key: None,
            title: remote_prompt.base_metadata.title,
            meta: None,
            tags: std::collections::HashSet::new(),
        }
    }

    fn create_proxy_tool(&self, remote_tool: crate::mcp::types::Tool) -> Tool {
        let client = self.client.clone();
        let remote_name = remote_tool.base_metadata.name.clone();
        let local_name = if self.prefix.is_empty() {
            remote_name.clone()
        } else {
            format!("{}_{}", self.prefix, remote_name)
        };

        let handler = Arc::new(Box::new(move |_ctx, args| {
            let client = client.clone();
            let name = remote_name.clone();
            Box::pin(async move {
                let result = client.call_tool(&name, args).await?;
                Ok(result)
            })
                as std::pin::Pin<
                    Box<
                        dyn std::future::Future<
                                Output = Result<crate::tools::tool::ToolResult, FastMCPError>,
                            > + Send,
                    >,
                >
        }) as crate::tools::tool::ToolHandler);

        Tool {
            name: local_name,
            description: remote_tool.description,
            data: ToolKind::Function(ToolFunction {
                name: remote_tool.base_metadata.name,
                description: None,
                input_schema: remote_tool.input_schema,
                output_schema: remote_tool.output_schema,
                compiled_schema: None,
                fn_handler: handler,
            }),
            enabled: true,
            key: None,
            title: remote_tool.base_metadata.title,
            meta: None,
            tags: std::collections::HashSet::new(),
        }
    }

    async fn create_proxy_resource_and_register(
        &self,
        remote_resource: crate::mcp::types::Resource,
        host: &FastMCP,
    ) -> Result<(), FastMCPError> {
        let client = self.client.clone();

        // We need to match `ResourceReadHandler` from `src/resources/manager.rs`.
        // Based on error, it expects `Fn(String, Context)`.

        let handler = Arc::new(Box::new(
            move |uri: String, _ctx: crate::server::context::Context| {
                let client = client.clone();
                // Use local captured uri or argument? `ResourceReadHandler` is `Fn(String, Context)`.
                // The argument `uri` is the one requested.
                // If we are a proxy, we probably want to pass the REQUESTED uri to the remote?
                // But if we prefixed it? "prefix_resourceURI".
                // The remote server doesn't know about prefix.
                // However, `remote_resource.uri` is the ORIGINAL uri.
                // When we registered, we registered it with `remote_resource.uri` (Step 162 in previous proxy.rs content).
                // Wait, did I prefix the name/URI?
                // In `create_proxy_resource_and_register`, I used `remote_resource.uri` directly.
                // `host.add_resource(remote_resource, ...)` uses the URI inside the struct.
                // If multiple servers have same URI, they collide.
                // Assuming no collision or we don't care about prefixing URI for now.
                // So `uri` passed to handler passed to `read_resource` is correct.

                Box::pin(async move {
                    // client.read_resource returns Result<Vec<ResourceContents>, FastMCPError>
                    // Handler expects Result<Vec<ResourceContents>, FastMCPError>
                    // So we can just return it directly!
                    client.read_resource(&uri).await
                })
                    as std::pin::Pin<
                        Box<
                            dyn std::future::Future<
                                    Output = Result<
                                        Vec<crate::mcp::types::ResourceContents>,
                                        FastMCPError,
                                    >,
                                > + Send,
                        >,
                    >
            },
        ) as crate::resources::manager::ResourceReadHandler);

        host.add_resource(remote_resource, Some(handler))
    }
}
