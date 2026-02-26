//! Prompt template registration and execution.
//!
//! Prompts are reusable, parameterised message templates that MCP clients can
//! retrieve and fill in. They support typed arguments and produce a list of
//! messages suitable for passing to an LLM.
//!
//! - [`prompt`] — `Prompt` component type and argument definitions.
//! - [`manager`] — `PromptManager` for registration, lookup, and execution.
//! - [`types`] — Supporting types.

pub mod manager;
pub mod prompt;
pub mod types;
