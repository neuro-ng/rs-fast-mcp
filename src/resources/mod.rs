//! Resource management, URI templates, and subscriptions.
//!
//! Resources expose read-only data (files, database rows, live metrics, etc.) to
//! MCP clients. Each resource is identified by a URI and may optionally have a
//! custom read handler.
//!
//! - [`resource`] — Internal `Resource` component type.
//! - [`template`] — URI template matching and variable extraction.
//! - [`manager`] — `ResourceManager` for registration, lookup, subscriptions,
//!   and content reading.
//! - [`types`] — Supporting types.

pub mod manager;
pub mod resource;
pub mod template;
pub mod types;
