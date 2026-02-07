use crate::error::FastMCPError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc; // Added Serialize

// Placeholder types to simulate mcp_sdk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    pub role: String,
    pub content: crate::mcp::types::ContentBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    pub name: String,
    pub description: Option<String>,
    pub required: Option<bool>,
}

pub type PromptHandler = Box<
    dyn Fn(
            HashMap<String, Value>,
        )
            -> Pin<Box<dyn Future<Output = Result<Vec<PromptMessage>, FastMCPError>> + Send>>
        + Send
        + Sync,
>;

#[derive(Clone)]
pub struct PromptFunction {
    pub name: String,
    pub description: Option<String>,
    pub arguments: Option<Vec<PromptArgument>>,
    pub fn_handler: Arc<PromptHandler>,
}

pub type Prompt = crate::util::component::Component<PromptFunction>;
