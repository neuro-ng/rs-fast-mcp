//! Shared utility modules.
//!
//! - [`component`] — Generic `Component<T>` base type used by tools, resources, and prompts.
//! - [`json_schema`] — JSON Schema optimisation and helper functions.
//! - [`logging`] — Deprecated; see [`crate::server::logging`].
//! - [`macros`] — Internal helper macros.
//! - [`types`] — Common type aliases and small helpers.

pub mod component;
pub mod json_schema;
pub mod logging;
pub mod macros;
pub mod types;
