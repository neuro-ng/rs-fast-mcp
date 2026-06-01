use crate::error::FastMCPError;
use crate::mcp::types::{Resource, ResourceTemplate};
use crate::prompts::prompt::Prompt;
use crate::server::core::FastMCP;
use crate::server::providers::Provider;
use crate::tools::tool::Tool;
use async_trait::async_trait;
use std::sync::Arc;

/// A `Provider` that wraps a local nested [`FastMCP`] server and delegates
/// all discovery and execution calls to it.
#[derive(Debug, Clone)]
pub struct FastMCPProvider {
    server: Arc<FastMCP>,
}

impl FastMCPProvider {
    pub fn new(server: Arc<FastMCP>) -> Self {
        Self { server }
    }

    pub fn server(&self) -> &Arc<FastMCP> {
        &self.server
    }
}

#[async_trait]
impl Provider for FastMCPProvider {
    async fn list_tools(&self) -> Result<Vec<Tool>, FastMCPError> {
        Ok(self.server.list_tools())
    }

    async fn get_tool(&self, name: &str) -> Result<Option<Tool>, FastMCPError> {
        Ok(self.server.get_tool(name))
    }

    async fn list_resources(&self) -> Result<Vec<Resource>, FastMCPError> {
        Ok(self.server.list_resources())
    }

    async fn get_resource(&self, uri: &str) -> Result<Option<Resource>, FastMCPError> {
        Ok(self.server.get_resource(uri))
    }

    async fn list_resource_templates(&self) -> Result<Vec<ResourceTemplate>, FastMCPError> {
        Ok(self.server.list_resource_templates())
    }

    async fn list_prompts(&self) -> Result<Vec<Prompt>, FastMCPError> {
        Ok(self.server.list_prompts())
    }

    async fn get_prompt(&self, name: &str) -> Result<Option<Prompt>, FastMCPError> {
        Ok(self.server.get_prompt(name))
    }

    async fn lifespan(&self) -> Result<(), FastMCPError> {
        self.server.run_startup().await
    }
}
