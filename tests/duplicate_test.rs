use rs_fast_mcp::server::core::FastMCP;
use rs_fast_mcp::server::strategy::DuplicateStrategy;
use rs_fast_mcp::tools::tool::{Tool, ToolFunction, ToolKind};
use serde_json::json;
use std::sync::Arc;

fn create_dummy_tool(name: &str) -> Tool {
    Tool {
        name: name.to_string(),
        description: None,
        data: ToolKind::Function(ToolFunction {
            name: name.to_string(),
            description: None,
            input_schema: json!({}),
            output_schema: None,
            fn_handler: Arc::new(Box::new(|_, _| {
                Box::pin(async {
                    Ok(rs_fast_mcp::tools::tool::ToolResult {
                        content: vec![],
                        structured_content: None,
                    })
                })
            })),
            compiled_schema: None,
        }),
        enabled: true,
        key: None,
        title: None,
        meta: None,
        tags: std::collections::HashSet::new(),
    }
}

#[test]
fn test_tool_duplicate_warn() {
    let server = FastMCP::new("test", "1.0");
    server.set_tool_strategy(DuplicateStrategy::Warn); // Default

    server.add_tool(create_dummy_tool("tool1")).unwrap();
    // Should succeed and overwrite
    server.add_tool(create_dummy_tool("tool1")).unwrap();
    assert_eq!(server.list_tools().len(), 1);
}

#[test]
fn test_tool_duplicate_error() {
    let server = FastMCP::new("test", "1.0");
    server.set_tool_strategy(DuplicateStrategy::Error);

    server.add_tool(create_dummy_tool("tool1")).unwrap();
    // Should fail
    let res = server.add_tool(create_dummy_tool("tool1"));
    assert!(res.is_err());
}

#[test]
fn test_tool_duplicate_ignore() {
    let server = FastMCP::new("test", "1.0");
    server.set_tool_strategy(DuplicateStrategy::Ignore);

    let mut t1 = create_dummy_tool("tool1");
    t1.description = Some("Original".to_string());
    server.add_tool(t1).unwrap();

    let mut t2 = create_dummy_tool("tool1");
    t2.description = Some("New".to_string());
    server.add_tool(t2).unwrap();

    let tools = server.list_tools();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].description.as_deref(), Some("Original"));
}
