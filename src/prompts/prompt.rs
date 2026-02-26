use crate::error::FastMCPError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// A single message in a prompt response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    /// The role of the message author (e.g. `"user"`, `"assistant"`).
    pub role: String,
    /// The message content block.
    pub content: crate::mcp::types::ContentBlock,
}

/// Describes a single argument accepted by a prompt template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    /// Argument name.
    pub name: String,
    /// Human-readable description of the argument.
    pub description: Option<String>,
    /// Whether this argument must be provided.
    pub required: Option<bool>,
}

/// An async function that renders a prompt given a map of string-keyed arguments.
pub type PromptHandler = Box<
    dyn Fn(
            HashMap<String, Value>,
        )
            -> Pin<Box<dyn Future<Output = Result<Vec<PromptMessage>, FastMCPError>> + Send>>
        + Send
        + Sync,
>;

/// Description of a prompt template backed by an async handler.
#[derive(Clone)]
pub struct PromptFunction {
    /// Prompt name (must be unique within the server).
    pub name: String,
    /// Human-readable description shown to clients.
    pub description: Option<String>,
    /// Formal arguments accepted by this prompt.
    pub arguments: Option<Vec<PromptArgument>>,
    /// The async handler closure that produces messages.
    pub fn_handler: Arc<PromptHandler>,
}

/// A registered prompt component.
///
/// Alias for [`Component<PromptFunction>`](crate::util::component::Component).

pub type Prompt = crate::util::component::Component<PromptFunction>;
