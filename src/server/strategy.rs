use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DuplicateStrategy {
    #[default]
    Warn,
    Error,
    Replace,
    Ignore,
}
