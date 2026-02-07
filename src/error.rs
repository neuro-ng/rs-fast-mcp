use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ErrorData {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Error, Debug)]
pub enum FastMCPError {
    #[error("FastMCP Error: {0:?}")]
    Base(ErrorData),

    #[error("Validation Error: {0:?}")]
    Validation(ErrorData),

    #[error("Resource Error: {0:?}")]
    Resource(ErrorData),

    #[error("Tool Error: {0:?}")]
    Tool(ErrorData),

    #[error("Prompt Error: {0:?}")]
    Prompt(ErrorData),

    #[error("Invalid Signature: {0:?}")]
    InvalidSignature(ErrorData),

    #[error("Client Error: {0:?}")]
    Client(ErrorData),

    #[error("Not Found: {0:?}")]
    NotFound(ErrorData),

    #[error("Disabled: {0:?}")]
    Disabled(ErrorData),

    #[error("IO Error: {0}")]
    StdIo(#[from] std::io::Error),

    #[error("JSON Error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid Request: {0}")]
    InvalidRequest(String),

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
