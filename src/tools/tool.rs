use crate::error::FastMCPError;
use crate::mcp::types::ContentBlock;
use crate::server::context::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::json;
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// The value returned by a tool handler after execution.
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolResult {
    /// Content blocks (text, images, etc.) returned to the caller.
    pub content: Vec<ContentBlock>,
    /// Optional structured JSON output for programmatic consumption.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,
}

/// An async function that executes a tool given a [`Context`] and JSON arguments.
pub type ToolHandler = Box<
    dyn Fn(Context, Value) -> Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
        + Send
        + Sync,
>;

fn default_handler() -> Arc<ToolHandler> {
    Arc::new(Box::new(|_, _| {
        Box::pin(async {
            Err(FastMCPError::new(
                "Tool handler not set (deserialized tool has no handler)".to_string(),
            ))
        })
    }))
}

/// Description of a callable tool backed by an async handler.
#[derive(Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    /// Tool name (must be unique within the server).
    pub name: String,
    /// Human-readable description shown to clients.
    pub description: Option<String>,
    /// JSON Schema describing the expected input arguments.
    pub input_schema: Value,
    /// Optional JSON Schema describing the output.
    pub output_schema: Option<Value>,
    /// The async handler closure.
    #[serde(skip, default = "default_handler")]
    pub fn_handler: Arc<ToolHandler>,
    /// Pre-compiled schema for validation (populated at registration).
    #[serde(skip)]
    pub compiled_schema: Option<Arc<Value>>,
}

/// Discriminated kind of tool.
#[derive(Clone, Serialize, Deserialize)]
pub enum ToolKind {
    /// A standard function-based tool.
    Function(ToolFunction),
    /// A tool whose input/output has been transformed.
    Transformed { original: ToolFunction },
}

/// A registered tool component.
///
/// Alias for [`Component<ToolKind>`](crate::util::component::Component).
pub type Tool = crate::util::component::Component<ToolKind>;

impl Tool {
    /// Creates a new function tool with the given name and description.
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            title: Some(name.to_string()), // Default title to name
            description: Some(description.to_string()),
            enabled: true,
            key: None,
            tags: HashSet::new(),
            meta: None,
            data: ToolKind::Function(ToolFunction {
                name: name.to_string(),
                description: Some(description.to_string()),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
                output_schema: None,
                fn_handler: default_handler(),
                compiled_schema: None,
            }),
        }
    }

    /// Adds a parameter to the tool's input JSON Schema.
    pub fn add_parameter(mut self, name: &str, type_: &str, description: &str) -> Self {
        if let ToolKind::Function(ref mut func) = self.data {
            // We need to modify input_schema. It's a Value.
            if let Some(props) = func
                .input_schema
                .as_object_mut()
                .and_then(|obj| obj.get_mut("properties"))
                .and_then(|v| v.as_object_mut())
            {
                props.insert(
                    name.to_string(),
                    json!({
                        "type": type_,
                        "description": description
                    }),
                );
            }
        }
        self
    }

    /// Sets the async handler closure for this tool.
    pub fn with_handler(mut self, handler: ToolHandler) -> Self {
        if let ToolKind::Function(ref mut func) = self.data {
            func.fn_handler = Arc::new(handler);
        }
        self
    }
}
