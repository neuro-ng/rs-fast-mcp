use crate::error::FastMCPError;
use crate::mcp::types::{Resource, ResourceTemplate};
use crate::prompts::prompt::Prompt;
use crate::server::providers::Provider;
use crate::tools::tool::Tool;
use async_trait::async_trait;
use std::collections::HashMap;

/// Wraps any inner [`Provider`] and applies namespace prefixing and optional
/// name overrides.
///
/// **Naming rules**:
/// - Tools: `{namespace}_{original_name}` (or the entry from `tool_renames`)
/// - Prompts: `{namespace}_{original_name}`
/// - Resources / templates: namespace injected as a path segment in the URI
///   authority (`schema://{namespace}/{original_authority}/{original_path}`)
#[derive(Debug)]
pub struct TransformingProvider {
    inner: Box<dyn Provider>,
    namespace: Option<String>,
    /// Map from original tool name → overridden name (applied before prefixing).
    tool_renames: HashMap<String, String>,
}

impl TransformingProvider {
    pub fn new(
        inner: Box<dyn Provider>,
        namespace: Option<String>,
        tool_renames: HashMap<String, String>,
    ) -> Self {
        Self {
            inner,
            namespace,
            tool_renames,
        }
    }

    fn prefix_name(&self, name: &str) -> String {
        match &self.namespace {
            Some(ns) => format!("{}_{}", ns, name),
            None => name.to_string(),
        }
    }

    fn prefix_tool_name(&self, original: &str) -> String {
        let renamed = self.tool_renames.get(original).map(|s| s.as_str()).unwrap_or(original);
        self.prefix_name(renamed)
    }

    /// Inject namespace as a path prefix in the URI.
    ///
    /// `schema://authority/path` → `schema://authority/{namespace}/path`
    /// Falls back to prepending `{ns}/` to the whole URI string if parsing fails.
    fn prefix_uri(&self, uri: &str) -> String {
        let Some(ns) = &self.namespace else {
            return uri.to_string();
        };

        if let Ok(parsed) = url::Url::parse(uri) {
            let scheme = parsed.scheme();
            let host = parsed.host_str().unwrap_or("");
            let path = parsed.path();
            // Insert namespace segment after authority
            let new_path = if path.starts_with('/') {
                format!("/{}/{}", ns, &path[1..])
            } else {
                format!("{}/{}", ns, path)
            };
            format!("{}://{}{}", scheme, host, new_path)
        } else {
            format!("{}/{}", ns, uri)
        }
    }

    /// Reverse the URI prefix transformation (used when routing reads).
    pub fn strip_prefix_uri(&self, uri: &str) -> String {
        let Some(ns) = &self.namespace else {
            return uri.to_string();
        };

        let segment = format!("/{}/", ns);
        if let Some(pos) = uri.find(&segment) {
            let mut stripped = uri.to_string();
            stripped.replace_range(pos..pos + segment.len(), "/");
            stripped
        } else {
            uri.to_string()
        }
    }
}

#[async_trait]
impl Provider for TransformingProvider {
    async fn list_tools(&self) -> Result<Vec<Tool>, FastMCPError> {
        let tools = self.inner.list_tools().await?;
        Ok(tools
            .into_iter()
            .map(|mut t| {
                let new_name = self.prefix_tool_name(&t.name);
                t.name = new_name.clone();
                if let crate::tools::tool::ToolKind::Function(ref mut f) = t.data {
                    f.name = new_name;
                }
                t
            })
            .collect())
    }

    async fn get_tool(&self, name: &str) -> Result<Option<Tool>, FastMCPError> {
        // Strip prefix to look up the original name in the inner provider
        let original = match &self.namespace {
            Some(ns) => {
                let pfx = format!("{}_", ns);
                name.strip_prefix(&pfx).unwrap_or(name)
            }
            None => name,
        };
        // Also check tool_renames in reverse
        let original = self
            .tool_renames
            .iter()
            .find(|(_, v)| v.as_str() == original)
            .map(|(k, _)| k.as_str())
            .unwrap_or(original);

        let tool = self.inner.get_tool(original).await?;
        Ok(tool.map(|mut t| {
            let new_name = self.prefix_tool_name(&t.name);
            t.name = new_name.clone();
            if let crate::tools::tool::ToolKind::Function(ref mut f) = t.data {
                f.name = new_name;
            }
            t
        }))
    }

    async fn list_resources(&self) -> Result<Vec<Resource>, FastMCPError> {
        let resources = self.inner.list_resources().await?;
        Ok(resources
            .into_iter()
            .map(|mut r| {
                r.uri = self.prefix_uri(&r.uri);
                r
            })
            .collect())
    }

    async fn get_resource(&self, uri: &str) -> Result<Option<Resource>, FastMCPError> {
        let inner_uri = self.strip_prefix_uri(uri);
        let resource = self.inner.get_resource(&inner_uri).await?;
        Ok(resource.map(|mut r| {
            r.uri = self.prefix_uri(&r.uri);
            r
        }))
    }

    async fn list_resource_templates(&self) -> Result<Vec<ResourceTemplate>, FastMCPError> {
        let templates = self.inner.list_resource_templates().await?;
        Ok(templates
            .into_iter()
            .map(|mut t| {
                t.uri_template = self.prefix_uri(&t.uri_template);
                t
            })
            .collect())
    }

    async fn list_prompts(&self) -> Result<Vec<Prompt>, FastMCPError> {
        let prompts = self.inner.list_prompts().await?;
        Ok(prompts
            .into_iter()
            .map(|mut p| {
                p.name = self.prefix_name(&p.name);
                p
            })
            .collect())
    }

    async fn get_prompt(&self, name: &str) -> Result<Option<Prompt>, FastMCPError> {
        let original = match &self.namespace {
            Some(ns) => {
                let pfx = format!("{}_", ns);
                name.strip_prefix(&pfx).unwrap_or(name)
            }
            None => name,
        };
        let prompt = self.inner.get_prompt(original).await?;
        Ok(prompt.map(|mut p| {
            p.name = self.prefix_name(&p.name);
            p
        }))
    }

    async fn lifespan(&self) -> Result<(), FastMCPError> {
        self.inner.lifespan().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::providers::FastMCPProvider;
    use crate::server::core::FastMCP;
    use crate::tools::tool::Tool;
    use std::sync::Arc;

    async fn make_provider_with_tool(tool_name: &str) -> FastMCPProvider {
        let server = Arc::new(FastMCP::new("child", "1.0"));
        server.add_tool(Tool::new(tool_name, "test tool")).unwrap();
        FastMCPProvider::new(server)
    }

    #[tokio::test]
    async fn test_transforming_prefixes_tool_names() {
        let inner = Box::new(make_provider_with_tool("greet").await);
        let trans = TransformingProvider::new(inner, Some("api".to_string()), HashMap::new());
        let tools = trans.list_tools().await.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "api_greet");
    }

    #[tokio::test]
    async fn test_transforming_get_tool_strips_prefix() {
        let inner = Box::new(make_provider_with_tool("greet").await);
        let trans = TransformingProvider::new(inner, Some("api".to_string()), HashMap::new());
        let tool = trans.get_tool("api_greet").await.unwrap();
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().name, "api_greet");
    }

    #[tokio::test]
    async fn test_no_namespace_passthrough() {
        let inner = Box::new(make_provider_with_tool("greet").await);
        let trans = TransformingProvider::new(inner, None, HashMap::new());
        let tools = trans.list_tools().await.unwrap();
        assert_eq!(tools[0].name, "greet");
    }

    #[test]
    fn test_prefix_uri() {
        let trans = TransformingProvider::new(
            Box::new(crate::server::providers::FastMCPProvider::new(Arc::new(
                FastMCP::new("x", "1"),
            ))),
            Some("ns".to_string()),
            HashMap::new(),
        );
        let prefixed = trans.prefix_uri("file:///data");
        assert!(prefixed.contains("/ns/"), "got: {}", prefixed);
        let stripped = trans.strip_prefix_uri(&prefixed);
        assert_eq!(stripped, "file:///data");
    }
}
