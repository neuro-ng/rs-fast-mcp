//! Tool registration, validation, and execution.
//!
//! Tools are the primary way an MCP server exposes callable functionality.
//! Each tool has a JSON Schema for input validation and an async handler.
//!
//! - [`tool`] — `Tool`, `ToolResult`, `ToolFunction`, and the `ToolHandler` type alias.
//! - [`manager`] — `ToolManager` for registration, lookup, fuzzy-match suggestions,
//!   schema validation, and invocation tracking.
//! - [`types`] — Shared supporting types.

pub mod manager;
pub mod tool;
pub mod types;
