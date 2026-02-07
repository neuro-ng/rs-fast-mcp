use crate::error::FastMCPError;
use crate::mcp::types::{Resource, ResourceContents};
use crate::server::context::Context; // For future handlers
use dashmap::DashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{info, warn};

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

pub struct RegisteredResource {
    pub metadata: Resource,
    pub handler: Option<Arc<ResourceReadHandler>>,
    pub read_count: Arc<AtomicUsize>,
}

pub struct RegisteredTemplate {
    pub template: crate::mcp::types::ResourceTemplate,
    pub handler: Arc<ResourceReadHandler>,
    pub regex: regex::Regex,
}

use crate::server::strategy::DuplicateStrategy;
use std::sync::RwLock;

pub struct ResourceManager {
    resources: DashMap<String, RegisteredResource>,
    templates: DashMap<String, RegisteredTemplate>,
    // URI -> Set of Session IDs (or just a counter/flag if we don't have session tracking fully wired up yet)
    // For now, let's just track a set of "subscribed" URIs (blindly) or SessionIDs if available.
    // Context has session_id.
    // Map<URI, HashSet<String>>
    subscriptions: DashMap<String, std::collections::HashSet<String>>,
    strategy: RwLock<DuplicateStrategy>,
}

impl ResourceManager {
    pub fn new() -> Self {
        Self {
            resources: DashMap::new(),
            templates: DashMap::new(),
            subscriptions: DashMap::new(),
            strategy: RwLock::new(DuplicateStrategy::default()),
        }
    }

    pub fn set_strategy(&self, strategy: DuplicateStrategy) {
        *self.strategy.write().unwrap() = strategy;
    }

    // Register a static resource (metadata only or static content? Usually read handler needed for dynamic)
    // For now, support metadata registration. Handlers need separate method or extended Resource struct?
    // FastMCP pattern: register_resource(resource, handler).

    pub fn register(&self, resource: Resource, handler: Option<Arc<ResourceReadHandler>>) -> Result<(), FastMCPError> {
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
                    return Err(FastMCPError::InvalidRequest(format!("Duplicate resource: {}", uri)));
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

    pub fn register_template(&self, template: crate::mcp::types::ResourceTemplate, handler: Arc<ResourceReadHandler>) -> Result<(), FastMCPError> {
        let uri_template = template.uri_template.clone();
        // Convert URI template {arg} to regex (?P<arg>.*)
        // This is a simplified implementation. Proper URI template parsing is complex.
        // Assuming simple {var} components.
        let pattern = regex::Regex::new(r"\{([^}]+)\}").map_err(|e| FastMCPError::new(e.to_string()))?;
        let regex_str = pattern.replace_all(&uri_template, "(?P<$1>.*)").to_string();
        // Or should we use .*? for broader match? MCP spec isn't super specific on template syntax but likely URI Template RFC.
        // Let's stick to [^/]+ for path segments for now, or .* if it's the end?
        // Let's use [^/]+ which is safe for path segments.
        let regex = regex::Regex::new(&format!("^{}$", regex_str)).map_err(|e| FastMCPError::new(e.to_string()))?;

        let registered = RegisteredTemplate {
            template,
            handler,
            regex,
        };
        
        // Templates are keyed by their URI template string
        self.templates.insert(uri_template.clone(), registered);
        info!("Registering resource template: {}", uri_template);
        Ok(())
    }

    pub fn get_resource(&self, uri: &str) -> Option<Resource> {
        self.resources.get(uri).map(|r| r.metadata.clone())
    }

    pub fn list_resources(&self) -> Vec<Resource> {
        let mut list = Vec::new();
        for entry in self.resources.iter() {
            list.push(entry.value().metadata.clone());
        }
        list
    }

    pub fn get_usage(&self, uri: &str) -> Option<usize> {
        self.resources.get(uri).map(|r| r.read_count.load(Ordering::Relaxed))
    }

    pub fn remove_resource(&self, uri: &str) {
        self.resources.remove(uri);
        self.subscriptions.remove(uri);
    }

    pub fn list_templates(&self) -> Vec<crate::mcp::types::ResourceTemplate> {
        let mut list = Vec::new();
        for entry in self.templates.iter() {
            list.push(entry.value().template.clone());
        }
        list
    }

    pub fn subscribe(&self, uri: String, session_id: Option<String>) {
        if let Some(sid) = session_id {
            let mut subs = self.subscriptions.entry(uri).or_default();
            subs.insert(sid);
        } else {
            warn!("Attempted to subscribe to {} without session_id", uri);
        }
    }

    pub fn unsubscribe(&self, uri: String, session_id: Option<String>) {
        if let Some(sid) = session_id 
            && let Some(mut subs) = self.subscriptions.get_mut(&uri) {
                subs.remove(&sid);
        }
    }

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

        // Try matching templates
        for template in self.templates.iter() {
             // ... match logic ...
            if let Some(caps) = template.regex.captures(uri) {
                // ...
                 let mut context = context.clone();
                for name in template.regex.capture_names().flatten() {
                    if let Some(m) = caps.name(name) {
                        context.arguments.insert(name.to_string(), m.as_str().to_string());
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
            if dist < min_dist && dist <= 5 { // Threshold 5 (URIs are longer)
                min_dist = dist;
                suggestion = Some(entry.key().clone());
            }
        }

        if let Some(s) = suggestion {
            Err(FastMCPError::InvalidRequest(format!("Resource not found: {}. Did you mean '{}'?", uri, s)))
        } else {
            Err(FastMCPError::InvalidRequest(format!("Resource not found: {}", uri)))
        }



    }
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}
