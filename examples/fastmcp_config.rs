use rs_fast_mcp::error::FastMCPError;
use rs_fast_mcp::mcp::types::{BaseMetadata, Resource, ResourceContents};
use rs_fast_mcp::server::core::FastMCPServer;
use rs_fast_mcp::server::transport::Transport;
use rs_fast_mcp::server::transport::stdio::StdioTransport;
use rs_fast_mcp::tools::tool::{Tool, ToolFunction, ToolKind, ToolResult};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

type ConfigStore = Arc<Mutex<HashMap<String, Value>>>;

fn create_set_setting_tool(store: ConfigStore) -> Tool {
    Tool {
        name: "set_setting".to_string(),
        description: Some("Update a configuration setting".to_string()),
        enabled: true,
        key: None,
        title: None,
        meta: None,
        tags: Default::default(),
        data: ToolKind::Function(ToolFunction {
            name: "set_setting".to_string(),
            description: None,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "key": { "type": "string" },
                    "value": {}
                },
                "required": ["key", "value"]
            }),
            output_schema: None,
            fn_handler: Arc::new(Box::new(move |_ctx, args| {
                let store = store.clone();
                Box::pin(async move {
                    let key = args
                        .get("key")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| FastMCPError::InvalidRequest("Missing 'key'".to_string()))?;

                    let value = args.get("value").ok_or_else(|| {
                        FastMCPError::InvalidRequest("Missing 'value'".to_string())
                    })?;

                    let mut guard = store.lock().unwrap();
                    guard.insert(key.to_string(), value.clone());

                    Ok(ToolResult {
                        content: vec![],
                        structured_content: Some(
                            json!({ "status": "updated", "key": key, "value": value }),
                        ),
                    })
                })
                    as Pin<Box<dyn Future<Output = Result<ToolResult, FastMCPError>> + Send>>
            })),
            compiled_schema: None,
        }),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize state
    let mut initial_config = HashMap::new();
    initial_config.insert("debug".to_string(), json!(false));
    initial_config.insert("theme".to_string(), json!("dark"));
    let store = Arc::new(Mutex::new(initial_config));

    // Create Server
    let server = FastMCPServer::new("config_server", "1.0");

    // Add Resource: config://settings
    let store_clone = store.clone();
    let resource = Resource {
        uri: "config://settings".to_string(),
        base_metadata: BaseMetadata {
            name: "settings".to_string(),
            title: None,
        },
        description: Some("Current system settings".to_string()),
        mime_type: Some("application/json".to_string()),
        annotations: None,
        size: None,
        icons: None,
        tags: None,
    };

    server
        .add_resource(
            resource,
            Some(Arc::new(Box::new(move |_req, _ctx| {
                let store = store_clone.clone();
                Box::pin(async move {
                    let guard = store.lock().unwrap();
                    let json_str = serde_json::to_string_pretty(&*guard).unwrap();
                    Ok(vec![ResourceContents {
                        uri: "config://settings".to_string(),
                        mime_type: Some("application/json".to_string()),
                        text: Some(json_str),
                        blob: None,
                    }])
                })
                    as Pin<
                        Box<
                            dyn Future<Output = Result<Vec<ResourceContents>, FastMCPError>> + Send,
                        >,
                    >
            }))),
        )
        .unwrap();

    // Add Tool: set_setting
    server.add_tool(create_set_setting_tool(store)).unwrap();

    // Run
    let transport = StdioTransport::new();
    let handler = Arc::new(server.clone());
    let rx = Some(server.subscribe_notifications());
    transport.start(handler, rx).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    // use rs_fast_mcp::server::transport::RequestHandler;
    // use serde_json::from_str;

    #[tokio::test]
    async fn test_config_workflow() {
        // Setup
        let mut initial_config = HashMap::new();
        initial_config.insert("debug".to_string(), json!(false));
        initial_config.insert("theme".to_string(), json!("dark"));
        let store = Arc::new(Mutex::new(initial_config));

        let server = FastMCPServer::new("config_server", "1.0");
        let store_clone = store.clone();

        // Register Resource
        let resource = Resource {
            uri: "config://settings".to_string(),
            base_metadata: BaseMetadata {
                name: "settings".to_string(),
                title: None,
            },
            description: Some("Current system settings".to_string()),
            mime_type: Some("application/json".to_string()),
            annotations: None,
            size: None,
            icons: None,
            tags: None,
        };
        server
            .add_resource(
                resource,
                Some(Arc::new(Box::new(move |_req, _ctx| {
                    let store = store_clone.clone();
                    Box::pin(async move {
                        let guard = store.lock().unwrap();
                        let json_str = serde_json::to_string_pretty(&*guard).unwrap();
                        Ok(vec![ResourceContents {
                            uri: "config://settings".to_string(),
                            mime_type: Some("application/json".to_string()),
                            text: Some(json_str),
                            blob: None,
                        }])
                    })
                        as Pin<
                            Box<
                                dyn Future<Output = Result<Vec<ResourceContents>, FastMCPError>>
                                    + Send,
                            >,
                        >
                }))),
            )
            .unwrap();

        // Register Tool
        server.add_tool(create_set_setting_tool(store)).unwrap();

        // 1. Read initial config
        let req = rs_fast_mcp::mcp::types::JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "resources/read".to_string(),
            params: Some(json!({ "uri": "config://settings" })),
            id: rs_fast_mcp::mcp::types::RequestId::Int(1),
            transport_metadata: None,
        };
        let resp = server.handle_request(req).await.unwrap();
        let result = resp.result;
        // The result is a list of ResourceContents. We need to parse it?
        // Wait, handle_request returns JsonRpcResponse. result is Option<Value>.
        // Value should be ReadResourceResult { contents: [...] }
        println!("Initial Read: {}", result);
        // We can't easily parse specific types from Value without defining them or using helpers,
        // but we can check string containment.
        assert!(result.to_string().contains("dark"));
        assert!(result.to_string().contains("false"));

        // 2. Update config "debug" -> true
        let req = rs_fast_mcp::mcp::types::JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "set_setting",
                "arguments": {
                    "key": "debug",
                    "value": true
                }
            })),
            id: rs_fast_mcp::mcp::types::RequestId::Int(2),
            transport_metadata: None,
        };
        let resp = server.handle_request(req).await.unwrap();
        // assert!(resp.error.is_none()); // JsonRpcResponse implies success

        // 3. Read config again to verify
        let req = rs_fast_mcp::mcp::types::JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "resources/read".to_string(),
            params: Some(json!({ "uri": "config://settings" })),
            id: rs_fast_mcp::mcp::types::RequestId::Int(3),
            transport_metadata: None,
        };
        let resp = server.handle_request(req).await.unwrap();
        let result = resp.result;
        println!("Final Read: {}", result);
        assert!(result.to_string().contains("true")); // debug: true
        // format is "debug": true, so check for true
    }
}
