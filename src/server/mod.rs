//! MCP server implementation.
//!
//! Build and run an MCP server with the [`builder::ServerBuilder`] or construct
//! one programmatically via [`core::FastMCP`] / [`core::FastMCPServer`].
//!
//! # Sub-modules
//!
//! - [`core`] — `FastMCP` engine: request routing, tool/resource/prompt management.
//! - [`app`] — `Server` struct that owns transports and drives the run-loop.
//! - [`builder`] — Fluent builder for configuring transports, auth, and tag filters.
//! - [`transport`] — Stdio and HTTP/SSE transport implementations.
//! - [`auth`] — Authentication provider trait and built-in providers.
//! - [`middleware`] — Composable request middleware (rate-limiting, caching).
//! - [`context`] — Per-request execution context with session state.
//! - [`proxy`] — Mount a remote MCP server behind a local prefix.
//! - [`strategy`] — Duplicate-registration strategies (warn, error, replace, ignore).
//! - [`logging`] — `tracing`-based logging initialisation and runtime level changes.

pub mod app;
pub mod auth;
pub mod builder;
pub mod context;
pub mod core;
pub mod middleware;
pub mod proxy;
pub mod strategy;
pub mod transport;

pub mod logging;
