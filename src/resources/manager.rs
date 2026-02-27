use crate::error::FastMCPError;
use crate::mcp::types::{Resource, ResourceContents};
use crate::server::context::Context;
use dashmap::DashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{info, warn};

/// An async function that reads a resource given its URI and a [`Context`].
pub type ResourceReadHandler = Box<
    dyn Fn(
            String,
            Context,
        )
            -> Pin<Box<dyn Future<Output = Result<Vec<ResourceContents>, FastMCPError>> + Send>>
        + Send
        + Sync,
>;

use std::sync::atomic::{AtomicUsize, Ordering};

/// A resource together with its optional read handler and usage counter.
pub struct RegisteredResource {
    pub metadata: Resource,
    pub handler: Option<Arc<ResourceReadHandler>>,
    pub read_count: Arc<AtomicUsize>,
}

/// A URI template paired with its handler and compiled regex.
pub struct RegisteredTemplate {
    pub template: crate::mcp::types::ResourceTemplate,
    pub handler: Arc<ResourceReadHandler>,
    pub regex: regex::Regex,
}

use crate::server::strategy::DuplicateStrategy;
use std::sync::RwLock;

/// Manages resource registration, lookup, subscriptions, and reading.
pub struct ResourceManager {
    resources: DashMap<String, RegisteredResource>,
    templates: DashMap<String, RegisteredTemplate>,
    subscriptions: DashMap<String, std::collections::HashSet<String>>,
    strategy: RwLock<DuplicateStrategy>,
}

impl ResourceManager {
    /// Creates an empty resource manager with default duplicate strategy.
    pub fn new() -> Self {
        Self {
            resources: DashMap::new(),
            templates: DashMap::new(),
            subscriptions: DashMap::new(),
            strategy: RwLock::new(DuplicateStrategy::default()),
        }
    }

    /// Sets the duplicate-resource registration strategy.
    pub fn set_strategy(&self, strategy: DuplicateStrategy) {
        *self.strategy.write().unwrap() = strategy;
    }

    /// Registers a resource with an optional read handler.
    pub fn register(
        &self,
        resource: Resource,
        handler: Option<Arc<ResourceReadHandler>>,
    ) -> Result<(), FastMCPError> {
        let uri = resource.uri.clone();
        let registered = RegisteredResource {
            metadata: resource,
            handler,
            read_count: Arc::new(AtomicUsize::new(0)),
        };

        if self.resources.contains_key(&uri) {
            let strategy = *self.strategy.read().unwrap();
            match strategy {
                DuplicateStrategy::Warn => {
                    warn!("Overwriting duplicate resource: {}", uri);
                    self.resources.insert(uri, registered);
                }
                DuplicateStrategy::Error => {
                    return Err(FastMCPError::InvalidRequest(format!(
                        "Duplicate resource: {}",
                        uri
                    )));
                }
                DuplicateStrategy::Replace => {
                    self.resources.insert(uri, registered);
                }
                DuplicateStrategy::Ignore => {
                    warn!("Ignoring duplicate resource registration: {}", uri);
                    return Ok(());
                }
            }
        } else {
            info!("Registering resource: {}", uri);
            self.resources.insert(uri, registered);
        }
        Ok(())
    }

    /// Registers a URI template with a read handler for dynamic resources.
    ///
    /// Template variables (`{var}`) are extracted into [`Context::arguments`](crate::server::context::Context)
    /// before the handler is called.
    pub fn register_template(
        &self,
        template: crate::mcp::types::ResourceTemplate,
        handler: Arc<ResourceReadHandler>,
    ) -> Result<(), FastMCPError> {
        let uri_template = template.uri_template.clone();
        // Convert `{var}` placeholders in the URI template to named capture groups.
        let pattern =
            regex::Regex::new(r"\{([^}]+)\}").map_err(|e| FastMCPError::new(e.to_string()))?;
        let regex_str = pattern.replace_all(&uri_template, "(?P<$1>.*)").to_string();
        let regex = regex::Regex::new(&format!("^{}$", regex_str))
            .map_err(|e| FastMCPError::new(e.to_string()))?;

        let registered = RegisteredTemplate {
            template,
            handler,
            regex,
        };

        self.templates.insert(uri_template.clone(), registered);
        info!("Registering resource template: {}", uri_template);
        Ok(())
    }

    /// Looks up a resource by exact URI.
    pub fn get_resource(&self, uri: &str) -> Option<Resource> {
        self.resources.get(uri).map(|r| r.metadata.clone())
    }

    /// Returns all registered static resources.
    pub fn list_resources(&self) -> Vec<Resource> {
        let mut list = Vec::new();
        for entry in self.resources.iter() {
            list.push(entry.value().metadata.clone());
        }
        list
    }

    /// Returns the read count for a given URI.
    pub fn get_usage(&self, uri: &str) -> Option<usize> {
        self.resources
            .get(uri)
            .map(|r| r.read_count.load(Ordering::Relaxed))
    }

    /// Removes a resource and its subscriptions.
    pub fn remove_resource(&self, uri: &str) {
        self.resources.remove(uri);
        self.subscriptions.remove(uri);
    }

    /// Returns all registered URI templates.
    pub fn list_templates(&self) -> Vec<crate::mcp::types::ResourceTemplate> {
        let mut list = Vec::new();
        for entry in self.templates.iter() {
            list.push(entry.value().template.clone());
        }
        list
    }

    /// Records a subscription for a resource URI from the given session.
    pub fn subscribe(&self, uri: String, session_id: Option<String>) {
        if let Some(sid) = session_id {
            let mut subs = self.subscriptions.entry(uri).or_default();
            subs.insert(sid);
        } else {
            warn!("Attempted to subscribe to {} without session_id", uri);
        }
    }

    /// Removes a subscription for a resource URI from the given session.
    pub fn unsubscribe(&self, uri: String, session_id: Option<String>) {
        if let Some(sid) = session_id
            && let Some(mut subs) = self.subscriptions.get_mut(&uri)
        {
            subs.remove(&sid);
        }
    }

    /// Reads a resource by URI, dispatching to the registered handler.
    ///
    /// Falls back to URI template matching and fuzzy-match suggestions.
    pub async fn read_resource(
        &self,
        uri: &str,
        context: Context,
    ) -> Result<Vec<ResourceContents>, FastMCPError> {
        // Validation: Check valid URI
        if let Err(e) = url::Url::parse(uri) {
            return Err(FastMCPError::InvalidRequest(format!("Invalid URI: {}", e)));
        }

        let resource_entry = self.resources.get(uri);

        if let Some(resource_entry) = resource_entry {
            // Found exact match
            resource_entry.read_count.fetch_add(1, Ordering::Relaxed);
            let handler = resource_entry.handler.clone();
            drop(resource_entry);

            if let Some(h) = handler {
                return (h)(uri.to_string(), context).await;
            } else {
                return Err(FastMCPError::InvalidRequest(format!(
                    "Resource {} has no read handler",
                    uri
                )));
            }
        }

        // No exact match; try URI template matching.
        for template in self.templates.iter() {
            if let Some(caps) = template.regex.captures(uri) {
                let mut context = context.clone();
                for name in template.regex.capture_names().flatten() {
                    if let Some(m) = caps.name(name) {
                        context
                            .arguments
                            .insert(name.to_string(), m.as_str().to_string());
                    }
                }

                let handler = template.handler.clone();
                return (handler)(uri.to_string(), context).await;
            }
        }

        // Fuzzy match on static resources
        let mut suggestion = None;
        let mut min_dist = usize::MAX;
        for entry in self.resources.iter() {
            let dist = strsim::levenshtein(uri, entry.key());
            if dist < min_dist && dist <= 5 {
                // Threshold 5 (URIs are longer)
                min_dist = dist;
                suggestion = Some(entry.key().clone());
            }
        }

        if let Some(s) = suggestion {
            Err(FastMCPError::InvalidRequest(format!(
                "Resource not found: {}. Did you mean '{}'?",
                uri, s
            )))
        } else {
            Err(FastMCPError::InvalidRequest(format!(
                "Resource not found: {}",
                uri
            )))
        }
    }
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::types::{BaseMetadata, Resource};

    fn make_resource(uri: &str, name: &str) -> Resource {
        Resource {
            uri: uri.to_string(),
            description: Some(format!("{} resource", name)),
            mime_type: Some("text/plain".to_string()),
            size: None,
            annotations: None,
            icons: None,
            tags: None,
            base_metadata: BaseMetadata {
                name: name.to_string(),
                title: None,
            },
        }
    }

    #[test]
    fn test_register_and_get() {
        let mgr = ResourceManager::new();
        mgr.register(make_resource("file:///a.txt", "a"), None)
            .unwrap();
        let r = mgr.get_resource("file:///a.txt");
        assert!(r.is_some());
        assert_eq!(r.unwrap().uri, "file:///a.txt");
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let mgr = ResourceManager::new();
        assert!(mgr.get_resource("file:///missing").is_none());
    }

    #[test]
    fn test_list_resources() {
        let mgr = ResourceManager::new();
        mgr.register(make_resource("file:///a", "a"), None).unwrap();
        mgr.register(make_resource("file:///b", "b"), None).unwrap();
        let list = mgr.list_resources();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_remove_resource() {
        let mgr = ResourceManager::new();
        mgr.register(make_resource("file:///rm", "rm"), None)
            .unwrap();
        assert!(mgr.get_resource("file:///rm").is_some());
        mgr.remove_resource("file:///rm");
        assert!(mgr.get_resource("file:///rm").is_none());
    }

    #[test]
    fn test_usage_tracking() {
        let mgr = ResourceManager::new();
        mgr.register(make_resource("file:///u", "u"), None).unwrap();
        assert_eq!(mgr.get_usage("file:///u"), Some(0));
        assert_eq!(mgr.get_usage("file:///missing"), None);
    }

    #[test]
    fn test_subscribe_and_unsubscribe() {
        let mgr = ResourceManager::new();
        mgr.register(make_resource("file:///s", "s"), None).unwrap();
        mgr.subscribe("file:///s".to_string(), Some("sess1".to_string()));
        mgr.subscribe("file:///s".to_string(), Some("sess2".to_string()));
        mgr.unsubscribe("file:///s".to_string(), Some("sess1".to_string()));
        // No panic = success (subscriptions are internal)
    }

    #[test]
    fn test_strategy_error_rejects_duplicate() {
        let mgr = ResourceManager::new();
        mgr.set_strategy(DuplicateStrategy::Error);
        mgr.register(make_resource("file:///dup", "dup"), None)
            .unwrap();
        let result = mgr.register(make_resource("file:///dup", "dup"), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_strategy_ignore_keeps_original() {
        let mgr = ResourceManager::new();
        mgr.set_strategy(DuplicateStrategy::Ignore);
        mgr.register(make_resource("file:///k", "original"), None)
            .unwrap();
        mgr.register(make_resource("file:///k", "replacement"), None)
            .unwrap();
        let r = mgr.get_resource("file:///k").unwrap();
        assert_eq!(r.description.unwrap(), "original resource");
    }
}
