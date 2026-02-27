use crate::error::FastMCPError;
use crate::server::context::Context;
use crate::tools::tool::{Tool, ToolKind, ToolResult};
use crate::util::json_schema::optimize_schema;
use dashmap::DashMap;
use serde_json::Value;

use tracing::{info, warn};

use crate::server::strategy::DuplicateStrategy;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A tool together with its invocation counter.
pub struct RegisteredTool {
    pub tool: Tool,
    pub call_count: Arc<AtomicUsize>,
}

/// Registry of tools: registration, lookup, schema validation, and invocation.
///
/// Every public method is safe to call from multiple threads; the underlying
/// [`DashMap`] provides fine-grained locking.
pub struct ToolManager {
    tools: DashMap<String, RegisteredTool>,
    strategy: RwLock<DuplicateStrategy>,
}

impl ToolManager {
    /// Creates an empty manager with the default [`DuplicateStrategy`].
    pub fn new() -> Self {
        Self {
            tools: DashMap::new(),
            strategy: RwLock::new(DuplicateStrategy::default()),
        }
    }

    /// Changes the strategy used when a duplicate tool name is registered.
    pub fn set_strategy(&self, strategy: DuplicateStrategy) {
        *self.strategy.write().unwrap() = strategy;
    }

    /// Registers a tool, compiling its input schema for validation.
    pub fn register(&self, mut tool: Tool) -> Result<(), FastMCPError> {
        let name = tool.name.clone();

        // Optimize schema for Function tools
        if let ToolKind::Function(ref mut func) = tool.data
            && func.compiled_schema.is_none()
        {
            let optimized = optimize_schema(&func.input_schema);
            func.compiled_schema = Some(Arc::new(optimized));
        }

        let registered = RegisteredTool {
            tool,
            call_count: Arc::new(AtomicUsize::new(0)),
        };

        if self.tools.contains_key(&name) {
            let strategy = *self.strategy.read().unwrap();
            match strategy {
                DuplicateStrategy::Warn => {
                    warn!("Overwriting duplicate tool: {}", name);
                    self.tools.insert(name, registered);
                }
                DuplicateStrategy::Error => {
                    return Err(FastMCPError::InvalidRequest(format!(
                        "Duplicate tool: {}",
                        name
                    )));
                }
                DuplicateStrategy::Replace => {
                    self.tools.insert(name, registered);
                }
                DuplicateStrategy::Ignore => {
                    warn!("Ignoring duplicate tool registration: {}", name);
                    return Ok(());
                }
            }
        } else {
            info!("Registering tool: {}", name);
            self.tools.insert(name, registered);
        }
        Ok(())
    }

    /// Looks up a tool by name.
    pub fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tools.get(name).map(|t| t.tool.clone())
    }

    /// Returns all registered tools.
    pub fn list_tools(&self) -> Vec<Tool> {
        let mut tools_list = Vec::new();
        for entry in self.tools.iter() {
            tools_list.push(entry.value().tool.clone());
        }
        tools_list
    }

    /// Returns the number of times a tool has been invoked.
    pub fn get_usage(&self, name: &str) -> Option<usize> {
        self.tools
            .get(name)
            .map(|t| t.call_count.load(Ordering::Relaxed))
    }

    /// Removes a tool by name.
    pub fn remove_tool(&self, name: &str) {
        self.tools.remove(name);
    }

    /// Validates `arguments` against the tool's schema, then invokes the handler.
    ///
    /// Returns a fuzzy-match suggestion in the error message when the tool name
    /// is not found but a close match exists.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
        context: Context,
    ) -> Result<ToolResult, FastMCPError> {
        let tool_entry = self.tools.get(name).ok_or_else(|| {
            let mut suggestion = None;
            let mut min_dist = usize::MAX;
            for entry in self.tools.iter() {
                let dist = strsim::levenshtein(name, entry.key());
                if dist < min_dist && dist <= 3 {
                    min_dist = dist;
                    suggestion = Some(entry.key().clone());
                }
            }

            if let Some(s) = suggestion {
                FastMCPError::InvalidRequest(format!(
                    "Tool not found: {}. Did you mean '{}'?",
                    name, s
                ))
            } else {
                FastMCPError::InvalidRequest(format!("Tool not found: {}", name))
            }
        })?;

        tool_entry.call_count.fetch_add(1, Ordering::Relaxed);

        let tool = &tool_entry.tool;
        match &tool.data {
            ToolKind::Function(func) => {
                let validation_schema = func
                    .compiled_schema
                    .as_deref()
                    .unwrap_or(&func.input_schema);

                match jsonschema::validator_for(validation_schema) {
                    Ok(schema) => {
                        if let Err(error) = schema.validate(&arguments) {
                            return Err(FastMCPError::InvalidRequest(format!(
                                "Invalid arguments: {} at {}",
                                error,
                                error.instance_path()
                            )));
                        }
                    }
                    Err(e) => {
                        warn!("Failed to compile input schema for tool {}: {}", name, e);
                    }
                }

                let handler = &func.fn_handler;
                (handler)(context, arguments).await
            }
            ToolKind::Transformed { .. } => Err(FastMCPError::new(
                "Transformed tools not supported yet".to_string(),
            )),
        }
    }
}

impl Default for ToolManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::tool::Tool;

    fn make_tool(name: &str) -> Tool {
        Tool::new(name, &format!("{} description", name))
    }

    #[test]
    fn test_register_and_get() {
        let mgr = ToolManager::new();
        mgr.register(make_tool("echo")).unwrap();
        let tool = mgr.get_tool("echo");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().name, "echo");
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let mgr = ToolManager::new();
        assert!(mgr.get_tool("missing").is_none());
    }

    #[test]
    fn test_list_tools() {
        let mgr = ToolManager::new();
        mgr.register(make_tool("a")).unwrap();
        mgr.register(make_tool("b")).unwrap();
        mgr.register(make_tool("c")).unwrap();
        let list = mgr.list_tools();
        assert_eq!(list.len(), 3);
        let names: Vec<String> = list.iter().map(|t| t.name.clone()).collect();
        assert!(names.contains(&"a".to_string()));
        assert!(names.contains(&"b".to_string()));
        assert!(names.contains(&"c".to_string()));
    }

    #[test]
    fn test_remove_tool() {
        let mgr = ToolManager::new();
        mgr.register(make_tool("temp")).unwrap();
        assert!(mgr.get_tool("temp").is_some());
        mgr.remove_tool("temp");
        assert!(mgr.get_tool("temp").is_none());
    }

    #[test]
    fn test_usage_tracking() {
        let mgr = ToolManager::new();
        mgr.register(make_tool("counter")).unwrap();
        assert_eq!(mgr.get_usage("counter"), Some(0));
        assert_eq!(mgr.get_usage("missing"), None);
    }

    #[test]
    fn test_strategy_error_rejects_duplicate() {
        let mgr = ToolManager::new();
        mgr.set_strategy(DuplicateStrategy::Error);
        mgr.register(make_tool("dup")).unwrap();
        let result = mgr.register(make_tool("dup"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Duplicate tool"));
    }

    #[test]
    fn test_strategy_ignore_keeps_original() {
        let mgr = ToolManager::new();
        mgr.set_strategy(DuplicateStrategy::Ignore);
        mgr.register(make_tool("keep")).unwrap();
        mgr.register(Tool::new("keep", "different")).unwrap();
        let tool = mgr.get_tool("keep").unwrap();
        assert_eq!(tool.description.unwrap(), "keep description");
    }

    #[test]
    fn test_strategy_replace_overwrites() {
        let mgr = ToolManager::new();
        mgr.set_strategy(DuplicateStrategy::Replace);
        mgr.register(make_tool("replace_me")).unwrap();
        mgr.register(Tool::new("replace_me", "new desc")).unwrap();
        let tool = mgr.get_tool("replace_me").unwrap();
        assert_eq!(tool.description.unwrap(), "new desc");
    }
}
