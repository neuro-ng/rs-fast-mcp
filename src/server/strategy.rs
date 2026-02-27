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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_warn() {
        assert_eq!(DuplicateStrategy::default(), DuplicateStrategy::Warn);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let strategies = [
            DuplicateStrategy::Warn,
            DuplicateStrategy::Error,
            DuplicateStrategy::Replace,
            DuplicateStrategy::Ignore,
        ];
        for strategy in &strategies {
            let json = serde_json::to_string(strategy).unwrap();
            let parsed: DuplicateStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(&parsed, strategy);
        }
    }

    #[test]
    fn test_clone_and_copy() {
        let s = DuplicateStrategy::Error;
        let cloned = s.clone();
        let copied = s;
        assert_eq!(cloned, copied);
    }
}
