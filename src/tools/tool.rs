use crate::error::FastMCPError;
use crate::mcp::types::ContentBlock;
use crate::server::context::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value; // For HashMap<String, Value> in get_prompt arg parsing
use serde_json::json;
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<ContentBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,
}

pub type ToolHandler = Box<
    dyn Fn(Context, Value) -> Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
        + Send
        + Sync,
>;

fn default_handler() -> Arc<ToolHandler> {
    Arc::new(Box::new(|_, _| {
        Box::pin(async {
            // Use FastMCPError::new
            Err(FastMCPError::new(
                "Tool handler not initialized via deserialization".to_string(),
            ))
        })
    }))
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
    pub output_schema: Option<Value>,
    #[serde(skip, default = "default_handler")]
    pub fn_handler: Arc<ToolHandler>,
    #[serde(skip)]
    pub compiled_schema: Option<Arc<Value>>,
    // tool_data merged or separate
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ToolKind {
    Function(ToolFunction),
    Transformed {
        original: ToolFunction,
        // ... transformation logic ...
    },
}

pub type Tool = crate::util::component::Component<ToolKind>;

impl Tool {
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

    // Better API: with_handler overrides the handler
    pub fn with_handler(mut self, handler: ToolHandler) -> Self {
        if let ToolKind::Function(ref mut func) = self.data {
            func.fn_handler = Arc::new(handler);
        }
        self
    }
}
