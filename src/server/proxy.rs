use crate::client::{Client, ClientTransport};
use crate::error::FastMCPError;
use crate::server::core::FastMCP;
use crate::tools::tool::{Tool, ToolFunction, ToolKind};
use std::sync::Arc;

/// Mounts a remote MCP server as a local tool/resource/prompt provider.
pub struct MountedServer {
    client: Arc<Client>,
    prefix: String,
}

impl MountedServer {
    /// Creates a new proxy backed by the given client transport.
    pub fn new(transport: Box<dyn ClientTransport>, prefix: &str) -> Self {
        let client = Arc::new(Client::new(transport));
        Self {
            client,
            prefix: prefix.to_string(),
        }
    }

    /// Connects to the remote server, runs the MCP handshake, then mirrors its
    /// tools, resources, and prompts onto `host` under the configured prefix.
    pub async fn mount(&self, host: &FastMCP) -> Result<(), FastMCPError> {
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

        match self.client.list_tools().await {
            Ok(tools) => {
                for tool in tools {
                    let proxy_tool = self.create_proxy_tool(tool);
                    host.add_tool(proxy_tool)?;
                }
            }
            Err(e) => tracing::warn!("Failed to sync tools from {}: {}", self.prefix, e),
        }

        match self.client.list_resources().await {
            Ok(resources) => {
                for resource in resources {
                    self.create_proxy_resource_and_register(resource, host)
                        .await?;
                }
            }
            Err(e) => tracing::warn!("Failed to sync resources from {}: {}", self.prefix, e),
        }

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
                let description = result
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);

                // Deserialise wire PromptMessages, then rewrap as typed PromptResult
                let wire_messages: Vec<crate::prompts::prompt::PromptMessage> =
                    serde_json::from_value(messages_val.clone()).map_err(|e| {
                        FastMCPError::InvalidRequest(format!(
                            "Failed to deserialize messages: {}. Value: {}",
                            e, messages_val
                        ))
                    })?;

                let messages: Vec<crate::prompts::types::Message> = wire_messages
                    .into_iter()
                    .map(|pm| {
                        let role = if pm.role == "assistant" {
                            crate::prompts::types::MessageRole::Assistant
                        } else {
                            crate::prompts::types::MessageRole::User
                        };
                        crate::prompts::types::Message {
                            role,
                            content: crate::prompts::types::MessageContent::ContentBlock(
                                pm.content,
                            ),
                        }
                    })
                    .collect();

                let mut prompt_result = crate::prompts::types::PromptResult::new(messages);
                prompt_result.description = description;
                Ok(prompt_result)
            })
                as std::pin::Pin<
                    Box<
                        dyn std::future::Future<
                                Output = Result<
                                    crate::prompts::types::PromptResult,
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

        let handler: Arc<crate::resources::manager::ResourceReadHandler> = Arc::new(Box::new(
            move |uri: String, _ctx: crate::server::context::Context| {
                let client = client.clone();
                Box::pin(async move {
                    let wire = client.read_resource(&uri).await?;
                    // Convert wire Vec<ResourceContents> → typed ResourceResult
                    let contents = wire
                        .into_iter()
                        .map(|rc| {
                            if let Some(blob) = rc.blob {
                                let data = base64::Engine::decode(
                                    &base64::engine::general_purpose::STANDARD,
                                    blob,
                                )
                                .unwrap_or_default();
                                crate::resources::types::ResourceContent::binary(
                                    data,
                                    rc.mime_type,
                                )
                            } else {
                                crate::resources::types::ResourceContent::text(
                                    rc.text.unwrap_or_default(),
                                    rc.mime_type,
                                )
                            }
                        })
                        .collect();
                    Ok(crate::resources::types::ResourceResult {
                        contents,
                        meta: None,
                    })
                })
                    as std::pin::Pin<
                        Box<
                            dyn std::future::Future<
                                    Output = Result<
                                        crate::resources::types::ResourceResult,
                                        FastMCPError,
                                    >,
                                > + Send,
                        >,
                    >
            },
        ));

        host.add_resource(remote_resource, Some(handler))
    }
}
