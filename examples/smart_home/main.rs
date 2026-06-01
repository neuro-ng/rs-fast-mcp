use rs_fast_mcp::mcp::types::{BaseMetadata, Resource};
use rs_fast_mcp::prompts::prompt::{Prompt, PromptFunction};
use rs_fast_mcp::prompts::types::{Message, PromptResult};
use rs_fast_mcp::resources::types::ResourceResult;
use rs_fast_mcp::server::core::FastMCPServer;
use rs_fast_mcp::server::transport::Transport;
use rs_fast_mcp::server::transport::stdio::StdioTransport;
use rs_fast_mcp::tools::tool::{Tool, ToolFunction, ToolKind, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

// --- Domain Model ---

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RoomState {
    lights_on: bool,
    temperature: f64,
    door_locked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SmartHomeState {
    rooms: HashMap<String, RoomState>,
}

impl SmartHomeState {
    fn new() -> Self {
        let mut rooms = HashMap::new();
        rooms.insert(
            "living_room".to_string(),
            RoomState {
                lights_on: false,
                temperature: 21.0,
                door_locked: true,
            },
        );
        rooms.insert(
            "kitchen".to_string(),
            RoomState {
                lights_on: true,
                temperature: 22.0,
                door_locked: false,
            },
        );
        rooms.insert(
            "bedroom".to_string(),
            RoomState {
                lights_on: false,
                temperature: 20.0,
                door_locked: true,
            },
        );
        Self { rooms }
    }
}

// --- Main ---

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    rs_fast_mcp::server::logging::init_logging("info").expect("Failed to initialize logging");

    let server = FastMCPServer::new("smart-home", "1.0.0");
    let state = Arc::new(Mutex::new(SmartHomeState::new()));

    // --- Resources ---

    // home://state
    let state_clone = state.clone();
    let resource_state = Resource {
        uri: "home://state".to_string(),
        description: Some("Current state of the entire home".to_string()),
        mime_type: Some("application/json".to_string()),
        size: None,
        annotations: None,
        icons: None,
        tags: None,
        base_metadata: BaseMetadata {
            name: "home_state".to_string(),
            title: None,
        },
    };
    server
        .add_resource(
            resource_state,
            Some(Arc::new(Box::new(move |_uri, _| {
                let s = state_clone.clone();
                Box::pin(async move {
                    let data = s.lock().unwrap();
                    let json = serde_json::to_string_pretty(&*data).unwrap_or_default();
                    Ok(ResourceResult::from_text(
                        json,
                        Some("application/json".to_string()),
                    ))
                })
                    as Pin<
                        Box<
                            dyn Future<
                                    Output = Result<
                                        ResourceResult,
                                        rs_fast_mcp::error::FastMCPError,
                                    >,
                                > + Send,
                        >,
                    >
            }))),
        )
        .unwrap();

    // home://rooms/{room_id}
    for room_id in ["living_room", "kitchen", "bedroom"] {
        let state_clone = state.clone();
        let r_id = room_id.to_string();
        let resource_room = Resource {
            uri: format!("home://rooms/{}", room_id),
            description: Some(format!("State of {}", room_id)),
            mime_type: Some("application/json".to_string()),
            size: None,
            annotations: None,
            icons: None,
            tags: None,
            base_metadata: BaseMetadata {
                name: format!("room_{}", room_id),
                title: None,
            },
        };

        server
            .add_resource(
                resource_room,
                Some(Arc::new(Box::new(move |_uri, _| {
                    let s = state_clone.clone();
                    let rid = r_id.clone();
                    Box::pin(async move {
                        let data = s.lock().unwrap();
                        if let Some(room) = data.rooms.get(&rid) {
                            let json = serde_json::to_string_pretty(room).unwrap_or_default();
                            Ok(ResourceResult::from_text(
                                json,
                                Some("application/json".to_string()),
                            ))
                        } else {
                            Err(rs_fast_mcp::error::FastMCPError::Resource(
                                rs_fast_mcp::error::ErrorData {
                                    code: Some(404),
                                    message: "Room not found".to_string(),
                                    data: None,
                                },
                            ))
                        }
                    })
                        as Pin<
                            Box<
                                dyn Future<
                                        Output = Result<
                                            ResourceResult,
                                            rs_fast_mcp::error::FastMCPError,
                                        >,
                                    > + Send,
                            >,
                        >
                }))),
            )
            .unwrap();
    }

    // --- Tools ---

    // turn_on_light
    let state_clone = state.clone();
    server
        .add_tool(Tool {
            name: "turn_on_light".to_string(),
            description: Some("Turn on the light in a specific room".to_string()),
            title: None,
            enabled: true,
            key: None,
            tags: std::collections::HashSet::new(),
            meta: None,
            data: ToolKind::Function(ToolFunction {
                name: "turn_on_light".to_string(),
                description: None,
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "room_id": { "type": "string" }
                    },
                    "required": ["room_id"]
                }),
                output_schema: None,
                fn_handler: Arc::new(Box::new(move |_ctx, args| {
                    let s = state_clone.clone();
                    Box::pin(async move {
                        let room_id =
                            args.get("room_id")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| {
                                    rs_fast_mcp::error::FastMCPError::InvalidRequest(
                                        "Missing room_id".to_string(),
                                    )
                                })?;

                        let mut data = s.lock().unwrap();
                        if let Some(room) = data.rooms.get_mut(room_id) {
                            room.lights_on = true;
                            Ok(ToolResult {
                                content: vec![rs_fast_mcp::mcp::types::ContentBlock::Text(
                                    rs_fast_mcp::mcp::types::TextContent {
                                        type_: "text".to_string(),
                                        text: format!("Turned on lights in {}", room_id),
                                        annotations: None,
                                    },
                                )],
                                structured_content: None,
                            })
                        } else {
                            Err(rs_fast_mcp::error::FastMCPError::Tool(
                                rs_fast_mcp::error::ErrorData {
                                    code: Some(1),
                                    message: "Room not found".to_string(),
                                    data: None,
                                },
                            ))
                        }
                    })
                        as Pin<
                            Box<
                                dyn Future<
                                        Output = Result<
                                            ToolResult,
                                            rs_fast_mcp::error::FastMCPError,
                                        >,
                                    > + Send,
                            >,
                        >
                })),
                compiled_schema: None,
            }),
        })
        .unwrap();

    // turn_off_light
    let state_clone = state.clone();
    server
        .add_tool(Tool {
            name: "turn_off_light".to_string(),
            description: Some("Turn off the light in a specific room".to_string()),
            title: None,
            enabled: true,
            key: None,
            tags: std::collections::HashSet::new(),
            meta: None,
            data: ToolKind::Function(ToolFunction {
                name: "turn_off_light".to_string(),
                description: None,
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "room_id": { "type": "string" }
                    },
                    "required": ["room_id"]
                }),
                output_schema: None,
                fn_handler: Arc::new(Box::new(move |_ctx, args| {
                    let s = state_clone.clone();
                    Box::pin(async move {
                        let room_id =
                            args.get("room_id")
                                .and_then(|v| v.as_str())
                                .ok_or_else(|| {
                                    rs_fast_mcp::error::FastMCPError::InvalidRequest(
                                        "Missing room_id".to_string(),
                                    )
                                })?;

                        let mut data = s.lock().unwrap();
                        if let Some(room) = data.rooms.get_mut(room_id) {
                            room.lights_on = false;
                            Ok(ToolResult {
                                content: vec![rs_fast_mcp::mcp::types::ContentBlock::Text(
                                    rs_fast_mcp::mcp::types::TextContent {
                                        type_: "text".to_string(),
                                        text: format!("Turned off lights in {}", room_id),
                                        annotations: None,
                                    },
                                )],
                                structured_content: None,
                            })
                        } else {
                            Err(rs_fast_mcp::error::FastMCPError::Tool(
                                rs_fast_mcp::error::ErrorData {
                                    code: Some(1),
                                    message: "Room not found".to_string(),
                                    data: None,
                                },
                            ))
                        }
                    })
                        as Pin<
                            Box<
                                dyn Future<
                                        Output = Result<
                                            ToolResult,
                                            rs_fast_mcp::error::FastMCPError,
                                        >,
                                    > + Send,
                            >,
                        >
                })),
                compiled_schema: None,
            }),
        })
        .unwrap();

    // --- Prompts ---

    let state_clone = state.clone();
    server
        .add_prompt(Prompt {
            name: "security_report".to_string(),
            description: Some("Get a security report of the home".to_string()),
            title: None,
            enabled: true,
            key: None,
            tags: std::collections::HashSet::new(),
            meta: None,
            data: PromptFunction {
                name: "security_report".to_string(),
                description: Some("Get a security report of the home".to_string()),
                arguments: None,
                fn_handler: Arc::new(Box::new(move |_args| {
                    let s = state_clone.clone();
                    Box::pin(async move {
                        let data = s.lock().unwrap();
                        let mut report = String::from("Security Report:\n");
                        for (name, room) in &data.rooms {
                            let status = if room.door_locked {
                                "LOCKED"
                            } else {
                                "UNLOCKED"
                            };
                            report.push_str(&format!("- {}: {}\n", name, status));
                        }
                        Ok(PromptResult::new(vec![Message::assistant(report)]))
                    })
                        as Pin<
                            Box<
                                dyn Future<
                                        Output = Result<
                                            PromptResult,
                                            rs_fast_mcp::error::FastMCPError,
                                        >,
                                    > + Send,
                            >,
                        >
                })),
            },
        })
        .unwrap();

    let transport = StdioTransport::new();
    let handler = Arc::new(server);
    transport.start(handler, None).await?;

    Ok(())
}
