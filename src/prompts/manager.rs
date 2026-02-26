use crate::error::FastMCPError;
use crate::prompts::prompt::{Prompt, PromptMessage};
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

    /// Validates required arguments, runs the handler, and returns the
    /// prompt description together with the rendered messages.
    pub async fn get_prompt_execution(
        &self,
        name: &str,
        arguments: Option<HashMap<String, Value>>,
    ) -> Result<(Option<String>, Vec<PromptMessage>), FastMCPError> {
        let prompt = self
            .get_prompt(name)
            .ok_or_else(|| FastMCPError::InvalidRequest(format!("Prompt not found: {}", name)))?;

        let args = arguments.unwrap_or_default();

        // Validation: Check required arguments
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
        let messages = (handler)(args).await?;

        Ok((prompt.description.clone(), messages))
    }
}

impl Default for PromptManager {
    fn default() -> Self {
        Self::new()
    }
}
