//! Model Context Protocol (MCP) type definitions and wire-format utilities.
//!
//! This module contains the core protocol building blocks:
//!
//! - [`types`] — Serde-compatible structs for every MCP message, capability, and
//!   content type (text, image, audio, embedded resources).
//! - [`protocol`] — Constructor helpers for JSON-RPC requests, responses,
//!   notifications, and errors.
//! - [`config`] — Deserialization of `mcpServers` configuration files and
//!   transport-type inference.

pub mod config;
pub mod protocol;
pub mod types;
