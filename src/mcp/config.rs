use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TransportType {
    Stdio,
    Sse,
    #[serde(rename = "streamable-http")]
    StreamableHttp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MCPServerConfig {
    Stdio(StdioMCPServer),
    Remote(RemoteMCPServer),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StdioMCPServer {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    pub cwd: Option<String>,
    pub timeout: Option<u64>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub authentication: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteMCPServer {
    pub url: String,
    pub transport: Option<TransportType>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub sse_read_timeout: Option<f64>,
    pub timeout: Option<u64>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub authentication: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPConfig {
    #[serde(rename = "mcpServers")]
    pub mcp_servers: HashMap<String, MCPServerConfig>,
}

impl MCPConfig {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn new() -> Self {
        Self {
            mcp_servers: HashMap::new(),
        }
    }

    pub fn get_server(&self, name: &str) -> Option<&MCPServerConfig> {
        self.mcp_servers.get(name)
    }

    pub fn add_server(&mut self, name: String, server: MCPServerConfig) {
        self.mcp_servers.insert(name, server);
    }

    pub fn remove_server(&mut self, name: &str) {
        self.mcp_servers.remove(name);
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

impl RemoteMCPServer {
    pub fn get_transport_type(&self) -> TransportType {
        match &self.transport {
            Some(t) => t.clone(),
            None => infer_transport_type_from_url(&self.url),
        }
    }
}

// --- Server Configuration ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub name: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub tools: HashMap<String, ToolConfig>,
    #[serde(default)]
    pub resources: HashMap<String, ResourceConfig>,
}

impl ServerConfig {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ToolConfig {
    Command {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
        #[serde(default)]
        description: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ResourceConfig {
    File {
        path: String,
        mime_type: Option<String>,
    },
    Text {
        content: String,
        mime_type: Option<String>,
    },
}

impl Default for MCPConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Infer the transport type from a URL.
///
/// If the URL ends with `/sse` or `/sse/`, it returns `TransportType::Sse`.
/// Otherwise, it returns `TransportType::StreamableHttp`.
pub fn infer_transport_type_from_url(url: &str) -> TransportType {
    // Basic heuristic matching OCaml implementation
    if url.ends_with("/sse") || url.ends_with("/sse/") {
        TransportType::Sse
    } else {
        TransportType::StreamableHttp
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_stdio_parsing() {
        let json = json!({
            "command": "npx",
            "args": ["-y", "super-server"],
            "env": { "FOO": "bar" }
        });
        let server: MCPServerConfig = serde_json::from_value(json).unwrap();
        match server {
            MCPServerConfig::Stdio(s) => {
                assert_eq!(s.command, "npx");
                assert_eq!(s.args, vec!["-y", "super-server"]);
                assert_eq!(s.env.get("FOO").map(|s| s.as_str()), Some("bar"));
            }
            _ => panic!("Expected Stdio config"),
        }
    }

    #[test]
    fn test_remote_parsing() {
        let json = json!({
            "url": "http://localhost:8080/sse",
            "headers": { "Authorization": "Bearer token" }
        });
        let server: MCPServerConfig = serde_json::from_value(json).unwrap();
        match server {
            MCPServerConfig::Remote(s) => {
                assert_eq!(s.url, "http://localhost:8080/sse");
                assert_eq!(
                    s.headers.get("Authorization").map(|s| s.as_str()),
                    Some("Bearer token")
                );
            }
            _ => panic!("Expected Remote config"),
        }
    }

    #[test]
    fn test_mcp_config_file_parsing() {
        let json = json!({
            "mcpServers": {
                "myserver": {
                    "command": "foo"
                },
                "remoteserver": {
                    "url": "http://example.com"
                }
            }
        });
        let config: MCPConfig = serde_json::from_value(json).unwrap();
        assert!(config.mcp_servers.contains_key("myserver"));
        assert!(config.mcp_servers.contains_key("remoteserver"));
    }

    #[test]
    fn test_infer_transport() {
        assert_eq!(
            infer_transport_type_from_url("http://example.com/sse"),
            TransportType::Sse
        );
        assert_eq!(
            infer_transport_type_from_url("http://example.com/api"),
            TransportType::StreamableHttp
        );
    }
}
