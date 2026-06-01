use serde::{Deserialize, Serialize};

/// Raw content variant of a resource — either UTF-8 text or binary bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResourceData {
    Text(String),
    Binary(Vec<u8>),
}

/// A single piece of content contained within a resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContent {
    pub content: ResourceData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

impl ResourceContent {
    /// Create text content; mime_type defaults to `text/plain`.
    pub fn text(content: String, mime_type: Option<String>) -> Self {
        Self {
            content: ResourceData::Text(content),
            mime_type: Some(mime_type.unwrap_or_else(|| "text/plain".to_string())),
            meta: None,
        }
    }

    /// Create binary content; mime_type defaults to `application/octet-stream`.
    pub fn binary(content: Vec<u8>, mime_type: Option<String>) -> Self {
        Self {
            content: ResourceData::Binary(content),
            mime_type: Some(
                mime_type.unwrap_or_else(|| "application/octet-stream".to_string()),
            ),
            meta: None,
        }
    }

    /// Create JSON content by serialising any `Serialize` value; mime_type is `application/json`.
    pub fn json<T: Serialize>(value: &T) -> Result<Self, serde_json::Error> {
        let serialized = serde_json::to_string(value)?;
        Ok(Self {
            content: ResourceData::Text(serialized),
            mime_type: Some("application/json".to_string()),
            meta: None,
        })
    }
}

/// The typed result returned from reading a resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceResult {
    pub contents: Vec<ResourceContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

impl ResourceResult {
    pub fn from_text(text: String, mime_type: Option<String>) -> Self {
        Self {
            contents: vec![ResourceContent::text(text, mime_type)],
            meta: None,
        }
    }

    pub fn from_binary(data: Vec<u8>, mime_type: Option<String>) -> Self {
        Self {
            contents: vec![ResourceContent::binary(data, mime_type)],
            meta: None,
        }
    }
}

/// Convert a `ResourceResult` into the wire-protocol `Vec<ResourceContents>`.
///
/// The caller is responsible for injecting the correct `uri` into each entry
/// after this conversion.
impl From<ResourceResult> for Vec<crate::mcp::types::ResourceContents> {
    fn from(res: ResourceResult) -> Self {
        res.contents
            .into_iter()
            .map(|content| match content.content {
                ResourceData::Text(text) => crate::mcp::types::ResourceContents {
                    uri: String::new(),
                    mime_type: content.mime_type,
                    text: Some(text),
                    blob: None,
                },
                ResourceData::Binary(bin) => crate::mcp::types::ResourceContents {
                    uri: String::new(),
                    mime_type: content.mime_type,
                    text: None,
                    blob: Some(base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        bin,
                    )),
                },
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_content_defaults_mime() {
        let c = ResourceContent::text("hello".to_string(), None);
        assert_eq!(c.mime_type.as_deref(), Some("text/plain"));
        assert!(matches!(c.content, ResourceData::Text(ref t) if t == "hello"));
    }

    #[test]
    fn test_binary_content_defaults_mime() {
        let c = ResourceContent::binary(vec![1, 2, 3], None);
        assert_eq!(c.mime_type.as_deref(), Some("application/octet-stream"));
    }

    #[test]
    fn test_json_content() {
        #[derive(Serialize)]
        struct Payload {
            x: i32,
        }
        let c = ResourceContent::json(&Payload { x: 42 }).unwrap();
        assert_eq!(c.mime_type.as_deref(), Some("application/json"));
        assert!(matches!(&c.content, ResourceData::Text(s) if s.contains("42")));
    }

    #[test]
    fn test_resource_result_from_text() {
        let r = ResourceResult::from_text("data".to_string(), None);
        assert_eq!(r.contents.len(), 1);
    }

    #[test]
    fn test_into_resource_contents() {
        let r = ResourceResult::from_text("hello".to_string(), Some("text/plain".to_string()));
        let wire: Vec<crate::mcp::types::ResourceContents> = r.into();
        assert_eq!(wire.len(), 1);
        assert_eq!(wire[0].text.as_deref(), Some("hello"));
    }
}
