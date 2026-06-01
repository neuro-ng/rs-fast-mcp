use crate::error::FastMCPError;
use crate::prompts::prompt::Prompt;
use crate::prompts::types::PromptResult;
use dashmap::DashMap;
use serde_json::Value;
use std::collections::HashMap;
use tracing::{info, warn};

use crate::server::strategy::DuplicateStrategy;
use std::sync::RwLock;

/// Registry of prompt templates: registration, lookup, and execution.
pub struct PromptManager {
    prompts: DashMap<String, Prompt>,
    strategy: RwLock<DuplicateStrategy>,
}

impl PromptManager {
    /// Creates an empty manager with the default [`DuplicateStrategy`].
    pub fn new() -> Self {
        Self {
            prompts: DashMap::new(),
            strategy: RwLock::new(DuplicateStrategy::default()),
        }
    }

    /// Changes the strategy used when a duplicate prompt name is registered.
    pub fn set_strategy(&self, strategy: DuplicateStrategy) {
        *self.strategy.write().unwrap() = strategy;
    }

    /// Registers a prompt template.
    pub fn register(&self, prompt: Prompt) -> Result<(), FastMCPError> {
        let name = prompt.name.clone();
        if self.prompts.contains_key(&name) {
            let strategy = *self.strategy.read().unwrap();
            match strategy {
                DuplicateStrategy::Warn => {
                    warn!("Overwriting duplicate prompt: {}", name);
                    self.prompts.insert(name, prompt);
                }
                DuplicateStrategy::Error => {
                    return Err(FastMCPError::InvalidRequest(format!(
                        "Duplicate prompt: {}",
                        name
                    )));
                }
                DuplicateStrategy::Replace => {
                    self.prompts.insert(name, prompt);
                }
                DuplicateStrategy::Ignore => {
                    warn!("Ignoring duplicate prompt registration: {}", name);
                    return Ok(());
                }
            }
        } else {
            info!("Registering prompt: {}", name);
            self.prompts.insert(name, prompt);
        }
        Ok(())
    }

    /// Looks up a prompt by name.
    pub fn get_prompt(&self, name: &str) -> Option<Prompt> {
        self.prompts.get(name).map(|p| p.value().clone())
    }

    /// Returns all registered prompts.
    pub fn list_prompts(&self) -> Vec<Prompt> {
        let mut list = Vec::new();
        for entry in self.prompts.iter() {
            list.push((*entry.value()).clone());
        }
        list
    }

    /// Removes a prompt by name.
    pub fn remove_prompt(&self, name: &str) {
        self.prompts.remove(name);
    }

    /// Validates required arguments, runs the handler, and returns a [`PromptResult`].
    pub async fn get_prompt_execution(
        &self,
        name: &str,
        arguments: Option<HashMap<String, Value>>,
    ) -> Result<PromptResult, FastMCPError> {
        let prompt = self
            .get_prompt(name)
            .ok_or_else(|| FastMCPError::InvalidRequest(format!("Prompt not found: {}", name)))?;

        let args = arguments.unwrap_or_default();

        // Validate required arguments
        if let Some(defined_args) = &prompt.data.arguments {
            for arg_def in defined_args {
                if arg_def.required.unwrap_or(false) && !args.contains_key(&arg_def.name) {
                    return Err(FastMCPError::InvalidRequest(format!(
                        "Missing required argument: {}",
                        arg_def.name
                    )));
                }
            }
        }

        let handler = &prompt.data.fn_handler;
        let mut result = (handler)(args).await?;

        // Propagate the component-level description if not set by the handler
        if result.description.is_none() {
            result.description = prompt.description.clone();
        }
        Ok(result)
    }
}

impl Default for PromptManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::prompt::PromptFunction;
    use std::sync::Arc;

    fn make_prompt(name: &str) -> Prompt {
        crate::util::component::Component {
            name: name.to_string(),
            title: Some(name.to_string()),
            description: Some(format!("{} prompt", name)),
            tags: std::collections::HashSet::new(),
            meta: None,
            enabled: true,
            key: None,
            data: PromptFunction {
                name: name.to_string(),
                description: Some(format!("{} prompt", name)),
                arguments: None,
                fn_handler: Arc::new(Box::new(|_args| {
                    Box::pin(async {
                        Ok(crate::prompts::types::PromptResult::new(vec![
                            crate::prompts::types::Message::assistant("hello".to_string()),
                        ]))
                    })
                })),
            },
        }
    }

    #[test]
    fn test_register_and_get() {
        let mgr = PromptManager::new();
        mgr.register(make_prompt("greet")).unwrap();
        let p = mgr.get_prompt("greet");
        assert!(p.is_some());
        assert_eq!(p.unwrap().name, "greet");
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let mgr = PromptManager::new();
        assert!(mgr.get_prompt("missing").is_none());
    }

    #[test]
    fn test_list_prompts() {
        let mgr = PromptManager::new();
        mgr.register(make_prompt("a")).unwrap();
        mgr.register(make_prompt("b")).unwrap();
        let list = mgr.list_prompts();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_remove_prompt() {
        let mgr = PromptManager::new();
        mgr.register(make_prompt("temp")).unwrap();
        assert!(mgr.get_prompt("temp").is_some());
        mgr.remove_prompt("temp");
        assert!(mgr.get_prompt("temp").is_none());
    }

    #[test]
    fn test_strategy_error_rejects_duplicate() {
        let mgr = PromptManager::new();
        mgr.set_strategy(DuplicateStrategy::Error);
        mgr.register(make_prompt("dup")).unwrap();
        let result = mgr.register(make_prompt("dup"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Duplicate prompt"));
    }

    #[test]
    fn test_strategy_ignore_keeps_original() {
        let mgr = PromptManager::new();
        mgr.set_strategy(DuplicateStrategy::Ignore);
        mgr.register(make_prompt("keep")).unwrap();
        let mut replacement = make_prompt("keep");
        replacement.description = Some("different".to_string());
        mgr.register(replacement).unwrap();
        let p = mgr.get_prompt("keep").unwrap();
        assert_eq!(p.description.unwrap(), "keep prompt");
    }

    #[test]
    fn test_strategy_replace_overwrites() {
        let mgr = PromptManager::new();
        mgr.set_strategy(DuplicateStrategy::Replace);
        mgr.register(make_prompt("rep")).unwrap();
        let mut replacement = make_prompt("rep");
        replacement.description = Some("new description".to_string());
        mgr.register(replacement).unwrap();
        let p = mgr.get_prompt("rep").unwrap();
        assert_eq!(p.description.unwrap(), "new description");
    }

    #[tokio::test]
    async fn test_prompt_execution_not_found() {
        let mgr = PromptManager::new();
        let result = mgr.get_prompt_execution("missing", None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Prompt not found"));
    }
}
