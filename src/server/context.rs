//! Per-request execution context.

use dashmap::DashMap;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Per-request execution context passed to tool, resource, and prompt handlers.
///
/// A fresh `Context` is created for each request. Handlers can read
/// caller identity from the id fields, share data within a session via
/// `session_data`, or store ephemeral per-request values in `state`.
#[derive(Clone, Debug)]
pub struct Context {
    /// JSON-RPC request id of the triggering request, if known.
    pub request_id: Option<String>,
    /// Identifier of the authenticated client, if authentication is active.
    pub client_id: Option<String>,
    /// Transport session identifier (e.g. SSE session id).
    pub session_id: Option<String>,

    /// Key-value store shared across all requests within the same session.
    pub session_data: Arc<DashMap<String, Value>>,

    /// Key-value store scoped to this request only.
    pub state: Arc<DashMap<String, Value>>,

    /// Variables extracted from URI template placeholders during resource reads.
    ///
    /// For example, a template `file://{path}` matched against `file:///etc/hosts`
    /// populates `arguments["path"] = "/etc/hosts"`.
    pub arguments: HashMap<String, String>,

    tools_changed: Arc<RwLock<bool>>,
    resources_changed: Arc<RwLock<bool>>,
    prompts_changed: Arc<RwLock<bool>>,
}

impl Context {
    /// Creates a new context with the given identifiers and empty stores.
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

    /// Replaces the session-data store, useful for sharing state between contexts.
    pub fn with_session_data(mut self, session_data: Arc<DashMap<String, Value>>) -> Self {
        self.session_data = session_data;
        self
    }

    /// Reads a value from the per-request state store.
    pub fn get_state(&self, key: &str) -> Option<Value> {
        self.state.get(key).map(|v| v.clone())
    }

    /// Writes a value to the per-request state store.
    pub fn set_state(&self, key: String, value: Value) {
        self.state.insert(key, value);
    }

    /// Flags that the tool list has changed (used internally to queue notifications).
    pub async fn report_tool_list_changed(&self) {
        let mut w = self.tools_changed.write().await;
        *w = true;
    }

    /// Flags that the resource list has changed.
    pub async fn report_resource_list_changed(&self) {
        let mut w = self.resources_changed.write().await;
        *w = true;
    }

    /// Flags that the prompt list has changed.
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
