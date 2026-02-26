//! # Rs Fast MCP
//!
//! High-performance, async-first Rust implementation of the
//! [Model Context Protocol (MCP)](https://modelcontextprotocol.io/).
//!
//! Rs Fast MCP provides a complete framework for building MCP **servers** and **clients**
//! using modern Rust practices. Built on [`tokio`] and `actix-web`, it offers non-blocking
//! I/O, type-safe interfaces, and a rich middleware pipeline out of the box.
//!
//! ## Key Features
//!
//! - **Flexible transports** — Stdio (pipe) and HTTP/SSE with session management.
//! - **Authentication** — Pluggable [`server::auth::AuthProvider`] trait with built-in
//!   providers for Google, GitHub, Azure, AWS Cognito, Auth0, and more.
//! - **Middleware** — Composable request pipeline with rate-limiting, caching, and logging.
//! - **Component managers** — First-class registration, validation, and execution for
//!   [`tools`], [`resources`], and [`prompts`].
//! - **Client SDK** — Async client with transport auto-detection, sampling, and roots support.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use rs_fast_mcp::server::app::Server;
//! use rs_fast_mcp::tools::tool::{Tool, ToolResult};
//! use rs_fast_mcp::mcp::types::ContentBlock;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Build the server (creates an internal FastMCPServer engine)
//!     let server = Server::builder("my-server", "0.1.0")
//!         .stdio()
//!         .build();
//!
//!     // Register a tool on the running engine
//!     let tool = Tool::new("greet", "Say hello to a name")
//!         .add_parameter("name", "string", "The name to greet");
//!     server.core.add_tool(tool).unwrap();
//!
//!     // Run until EOF or SIGINT
//!     server.run().await.unwrap();
//! }
//! ```
//!
//! ## Module Overview
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`mcp`] | Protocol types, JSON-RPC messages, and configuration |
//! | [`server`] | Server core, transports, auth, and middleware |
//! | [`client`] | Async MCP client SDK |
//! | [`tools`] | Tool registration, validation, and execution |
//! | [`resources`] | Resource management and URI templates |
//! | [`prompts`] | Prompt templates and execution |
//! | [`cli`] | Command-line interface (`serve`, `client`, `inspect`) |
//! | [`settings`] | Environment-driven configuration |
//! | [`error`] | Error types used across the crate |
//! | [`util`] | JSON Schema helpers and shared utilities |

pub mod cli;
pub mod client;
pub mod error;
pub mod mcp;

pub mod prompts;
pub mod resources;
pub mod server;
pub mod settings;
pub mod shared;
pub mod tools;
pub mod util;
