use crate::error::FastMCPError;
use crate::mcp::types::{ContentBlock, EmbeddedResource, ImageContent};
use base64::{Engine as _, engine::general_purpose};
use tokio::fs;

#[derive(Debug, Clone)]
pub struct Image {
    pub path: Option<String>,
    pub data: Option<Vec<u8>>,
    pub mime_type: String,
}

impl Image {
    pub async fn from_path(path: &str) -> Result<Self, FastMCPError> {
        let data = fs::read(path).await.map_err(|e| {
            FastMCPError::Resource(crate::error::ErrorData {
                code: Some(-32603),
                message: format!("Failed to read image file: {}", e),
                data: None,
            })
        })?;

        let mime_type = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();

        Ok(Self {
            path: Some(path.to_string()),
            data: Some(data),
            mime_type,
        })
    }

    pub fn new(data: Vec<u8>, mime_type: &str) -> Self {
        Self {
            path: None,
            data: Some(data),
            mime_type: mime_type.to_string(),
        }
    }

    pub fn to_content_block(&self) -> Result<ContentBlock, FastMCPError> {
        let data = self
            .data
            .as_ref()
            .ok_or(FastMCPError::InvalidRequest("No data in Image".to_string()))?;
        let base64_data = general_purpose::STANDARD.encode(data);
        Ok(ContentBlock::Image(ImageContent {
            type_: "image".to_string(),
            data: base64_data,
            mime_type: self.mime_type.clone(),
            annotations: None,
        }))
    }
}

// Audio not standard in MCP spec v1 yet (Embedded Resource?), but good to have if needed.
// Specs mention Audio class.
#[derive(Debug, Clone)]
pub struct Audio {
    pub path: Option<String>,
    pub data: Option<Vec<u8>>,
    pub mime_type: String,
}

impl Audio {
    pub async fn from_path(path: &str) -> Result<Self, FastMCPError> {
        let data = fs::read(path).await.map_err(|e| {
            FastMCPError::Resource(crate::error::ErrorData {
                code: Some(-32603),
                message: format!("Failed to read audio file: {}", e),
                data: None,
            })
        })?;

        let mime_type = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();

        Ok(Self {
            path: Some(path.to_string()),
            data: Some(data),
            mime_type,
        })
    }

    // Audio usually sent as EmbeddedResource with blob in current MCP?
    // Or if "audio" content type exists.
    // For now, let's assume it maps to EmbeddedResource if not standard content.
    // But checking types.rs, ContentBlock only has Text, Image, EmbeddedResource.
    pub fn to_resource_content(&self) -> Result<ContentBlock, FastMCPError> {
        let data = self
            .data
            .as_ref()
            .ok_or(FastMCPError::InvalidRequest("No data in Audio".to_string()))?;
        let base64_data = general_purpose::STANDARD.encode(data);

        // Embedded resource
        Ok(ContentBlock::EmbeddedResource(EmbeddedResource {
            type_: "resource".to_string(),
            resource: crate::mcp::types::ResourceContents {
                uri: self.path.clone().unwrap_or_else(|| "audio".to_string()), // Simplified
                mime_type: Some(self.mime_type.clone()),
                text: None,
                blob: Some(base64_data),
            },
            annotations: None,
        }))
    }
}
