use dashmap::DashMap;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

/// Execution context for FastMCP
/// Centralizes context management for tool, resource, and prompt execution.
#[derive(Clone, Debug)]
pub struct Context {
    pub request_id: Option<String>,
    pub client_id: Option<String>,
    pub session_id: Option<String>,

    /// Session data shared across requests for the same session
    pub session_data: Arc<DashMap<String, Value>>,

    /// Ephemeral state for the current context (if long-lived) or request
    pub state: Arc<DashMap<String, Value>>,
    
    /// Arguments extracted from resource templates
    pub arguments: HashMap<String, String>,

    /// Track changes
    tools_changed: Arc<RwLock<bool>>,
    resources_changed: Arc<RwLock<bool>>,
    prompts_changed: Arc<RwLock<bool>>,
}

impl Context {
    pub fn new(
        request_id: Option<String>,
        client_id: Option<String>,
        session_id: Option<String>,
    ) -> Self {
        Self {
            request_id,
            client_id,
            session_id,
            session_data: Arc::new(DashMap::new()),
            state: Arc::new(DashMap::new()),
            tools_changed: Arc::new(RwLock::new(false)),
            resources_changed: Arc::new(RwLock::new(false)),
            prompts_changed: Arc::new(RwLock::new(false)),
            arguments: HashMap::new(),
        }
    }

    pub fn with_session_data(mut self, session_data: Arc<DashMap<String, Value>>) -> Self {
        self.session_data = session_data;
        self
    }

    pub fn get_state(&self, key: &str) -> Option<Value> {
        self.state.get(key).map(|v| v.clone())
    }

    pub fn set_state(&self, key: String, value: Value) {
        self.state.insert(key, value);
    }

    // Notifications (Queueing logic simplifed for Rust: just setters)
    pub async fn report_tool_list_changed(&self) {
        let mut w = self.tools_changed.write().await;
        *w = true;
    }

    pub async fn report_resource_list_changed(&self) {
        let mut w = self.resources_changed.write().await;
        *w = true;
    }

    pub async fn report_prompt_list_changed(&self) {
        let mut w = self.prompts_changed.write().await;
        *w = true;
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new(None, None, None)
    }
}
