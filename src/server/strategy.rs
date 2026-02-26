//! Duplicate-registration strategies for tools, resources, and prompts.

use serde::{Deserialize, Serialize};

/// Controls what happens when a tool, resource, or prompt with the same name
/// is registered twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DuplicateStrategy {
    /// Log a warning and overwrite the existing entry. *(default)*
    #[default]
    Warn,
    /// Return an error and leave the existing entry unchanged.
    Error,
    /// Silently overwrite the existing entry.
    Replace,
    /// Silently discard the new entry and keep the existing one.
    Ignore,
}
