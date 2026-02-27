//! Crate-wide error types.
//!
//! Every fallible operation returns `Result<T, FastMCPError>`. Variants are
//! grouped by subsystem so callers can match on the category of failure
//! without inspecting the inner message.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Structured error payload carried inside [`FastMCPError::Base`].
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ErrorData {
    /// Human-readable error description.
    pub message: String,
    /// Optional machine-readable error code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
    /// Optional structured data attached to the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Crate-wide error type.
///
/// Every fallible operation in rs-fast-mcp returns `Result<T, FastMCPError>`.
/// Variants are grouped by subsystem so callers can pattern-match on the
/// category of failure.
#[derive(Error, Debug)]
pub enum FastMCPError {
    /// General-purpose error.
    #[error("FastMCP Error: {0:?}")]
    Base(ErrorData),

    /// Input validation failure (e.g. bad JSON Schema).
    #[error("Validation Error: {0:?}")]
    Validation(ErrorData),

    /// Resource subsystem error (read failure, unknown URI, etc.).
    #[error("Resource Error: {0:?}")]
    Resource(ErrorData),

    /// Tool subsystem error (handler panic, schema mismatch, etc.).
    #[error("Tool Error: {0:?}")]
    Tool(ErrorData),

    /// Prompt subsystem error.
    #[error("Prompt Error: {0:?}")]
    Prompt(ErrorData),

    /// Authentication signature verification failure.
    #[error("Invalid Signature: {0:?}")]
    InvalidSignature(ErrorData),

    /// Client-side communication error.
    #[error("Client Error: {0:?}")]
    Client(ErrorData),

    /// Requested entity (tool, resource, prompt) does not exist.
    #[error("Not Found: {0:?}")]
    NotFound(ErrorData),

    /// Requested operation is disabled by configuration.
    #[error("Disabled: {0:?}")]
    Disabled(ErrorData),

    /// Standard I/O error (transport read/write failures).
    #[error("IO Error: {0}")]
    StdIo(#[from] std::io::Error),

    /// JSON serialisation / deserialisation error.
    #[error("JSON Error: {0}")]
    Json(#[from] serde_json::Error),

    /// Malformed or unsupported JSON-RPC request.
    #[error("Invalid Request: {0}")]
    InvalidRequest(String),

    /// Remote JSON-RPC error received from a peer.
    #[error("JSON-RPC Error: {code} {message}")]
    JsonRpcError {
        code: i64,
        message: String,
        data: Option<Value>,
    },
}

impl FastMCPError {
    pub fn new(message: impl Into<String>) -> Self {
        Self::Base(ErrorData {
            message: message.into(),
            code: None,
            data: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_base_variant() {
        let err = FastMCPError::new("something broke");
        let msg = err.to_string();
        assert!(msg.contains("something broke"));
    }

    #[test]
    fn test_invalid_request_display() {
        let err = FastMCPError::InvalidRequest("bad input".to_string());
        assert_eq!(err.to_string(), "Invalid Request: bad input");
    }

    #[test]
    fn test_json_rpc_error_display() {
        let err = FastMCPError::JsonRpcError {
            code: -32600,
            message: "Invalid JSON".to_string(),
            data: None,
        };
        let msg = err.to_string();
        assert!(msg.contains("-32600"));
        assert!(msg.contains("Invalid JSON"));
    }

    #[test]
    fn test_not_found_variant() {
        let err = FastMCPError::NotFound(ErrorData {
            message: "tool xyz".to_string(),
            code: Some(-32601),
            data: None,
        });
        assert!(err.to_string().contains("tool xyz"));
    }
}
