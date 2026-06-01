//! Provider composition architecture.
//!
//! The `Provider` trait replaces the monolithic `MountedServer` with a
//! composable, stackable design:
//!
//! ```text
//! [Parent FastMCP]
//!      │
//!      └── TransformingProvider("api")  ← adds "api_" prefix
//!               └── FastMCPProvider    ← delegates to child FastMCP
//!                        └── [Child FastMCP]
//! ```
//!
//! Use [`FastMCP::mount`] to compose child servers under a namespace.

mod fmcp_provider;
mod transforming;

pub use fmcp_provider::FastMCPProvider;
pub use transforming::TransformingProvider;

use crate::error::FastMCPError;
use crate::mcp::types::{Resource, ResourceTemplate};
use crate::prompts::prompt::Prompt;
use crate::tools::tool::Tool;
use async_trait::async_trait;

/// A composable source of MCP components (tools, resources, prompts).
///
/// Implementations may delegate to a local [`FastMCP`] engine, a remote
/// server, a database, or any other dynamic source.
#[async_trait]
pub trait Provider: Send + Sync + std::fmt::Debug {
    async fn list_tools(&self) -> Result<Vec<Tool>, FastMCPError>;
    async fn get_tool(&self, name: &str) -> Result<Option<Tool>, FastMCPError>;

    async fn list_resources(&self) -> Result<Vec<Resource>, FastMCPError>;
    async fn get_resource(&self, uri: &str) -> Result<Option<Resource>, FastMCPError>;

    async fn list_resource_templates(&self) -> Result<Vec<ResourceTemplate>, FastMCPError>;

    async fn list_prompts(&self) -> Result<Vec<Prompt>, FastMCPError>;
    async fn get_prompt(&self, name: &str) -> Result<Option<Prompt>, FastMCPError>;

    /// Called when the parent server starts up; can perform initialisation.
    async fn lifespan(&self) -> Result<(), FastMCPError>;
}
