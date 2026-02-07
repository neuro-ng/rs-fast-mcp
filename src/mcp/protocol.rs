use crate::mcp::types::{
    ErrorData, JsonRpcError, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, RequestId,
};
use serde_json::Value;

impl JsonRpcRequest {
    pub fn new(id: RequestId, method: &str, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
            transport_metadata: None,
        }
    }
}

impl JsonRpcNotification {
    pub fn new(method: &str, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        }
    }
}

impl JsonRpcResponse {
    pub fn new(id: RequestId, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result,
        }
    }
}

impl JsonRpcError {
    pub fn new(id: RequestId, code: i64, message: &str, data: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            error: ErrorData {
                code,
                message: message.to_string(),
                data,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::types::JsonRpcMessage;
    use serde_json::json;

    #[test]
    fn test_request_construction() {
        let req = JsonRpcRequest::new(RequestId::Int(1), "ping", Some(json!({})));
        assert_eq!(req.method, "ping");
        assert_eq!(req.id, RequestId::Int(1));
    }

    #[test]
    fn test_error_construction() {
        let err = JsonRpcError::new(
            RequestId::String("abc".to_string()),
            -32700,
            "Parse error",
            None,
        );
        assert_eq!(err.error.code, -32700);
        assert_eq!(err.error.message, "Parse error");
    }

    #[test]
    fn test_parsing() {
        let json_str = r#"{"jsonrpc": "2.0", "method": "ping", "id": 1}"#;
        let msg: JsonRpcMessage = serde_json::from_str(json_str).unwrap();
        match msg {
            JsonRpcMessage::Request(req) => {
                assert_eq!(req.method, "ping");
                assert_eq!(req.id, RequestId::Int(1));
            }
            _ => panic!("Expected Request"),
        }
    }
}
