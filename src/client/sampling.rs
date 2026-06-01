//! Unified sampling handler interface for client-side LLM integration.
//!
//! Implement [`SamplingHandler`] to bridge `sampling/createMessage` JSON-RPC
//! calls to any LLM provider (Anthropic, OpenAI, Google GenAI, …).
//!
//! Register a handler on the [`Client`](crate::client::Client) via
//! `Client::set_sampling_handler`.

use crate::error::FastMCPError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Core sampling types (subset of the MCP sampling spec)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SamplingRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingTextContent {
    #[serde(rename = "type")]
    pub type_: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SamplingContent {
    Text(SamplingTextContent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingMessage {
    pub role: SamplingRole,
    pub content: SamplingContent,
}

/// How the model should choose whether and which tool to use.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolChoiceMode {
    Auto,
    Required,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChoice {
    pub mode: ToolChoiceMode,
    /// Specific tool to use when `mode` is not `auto` (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Parameters sent with a `sampling/createMessage` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMessageRequestParams {
    pub messages: Vec<SamplingMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_preferences: Option<Value>,
}

/// The result returned by a sampling handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMessageResult {
    pub role: SamplingRole,
    pub content: SamplingContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Unified interface for delegating `sampling/createMessage` requests to an
/// LLM provider.
#[async_trait]
pub trait SamplingHandler: Send + Sync + std::fmt::Debug {
    async fn create_message(
        &self,
        messages: Vec<SamplingMessage>,
        params: CreateMessageRequestParams,
    ) -> Result<CreateMessageResult, FastMCPError>;
}

// ---------------------------------------------------------------------------
// Anthropic implementation
// ---------------------------------------------------------------------------

/// Sampling handler backed by the Anthropic Claude API (direct `reqwest` calls).
#[derive(Debug)]
pub struct AnthropicSamplingHandler {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl AnthropicSamplingHandler {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: "claude-sonnet-4-6".to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }
}

#[async_trait]
impl SamplingHandler for AnthropicSamplingHandler {
    async fn create_message(
        &self,
        messages: Vec<SamplingMessage>,
        params: CreateMessageRequestParams,
    ) -> Result<CreateMessageResult, FastMCPError> {
        let anthropic_messages: Vec<Value> = messages
            .iter()
            .map(|m| {
                let role = match &m.role {
                    SamplingRole::User => "user",
                    SamplingRole::Assistant => "assistant",
                };
                let content = match &m.content {
                    SamplingContent::Text(t) => t.text.clone(),
                };
                serde_json::json!({ "role": role, "content": content })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": anthropic_messages,
            "max_tokens": params.max_tokens.unwrap_or(1024),
        });

        if let Some(system) = &params.system {
            body["system"] = Value::String(system.clone());
        }

        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| FastMCPError::new(e.to_string()))?;

        let resp_json: Value = resp
            .json()
            .await
            .map_err(|e| FastMCPError::new(e.to_string()))?;

        let text = resp_json["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let model = resp_json["model"].as_str().map(str::to_string);
        let stop_reason = resp_json["stop_reason"].as_str().map(str::to_string);

        Ok(CreateMessageResult {
            role: SamplingRole::Assistant,
            content: SamplingContent::Text(SamplingTextContent {
                type_: "text".to_string(),
                text,
            }),
            model,
            stop_reason,
        })
    }
}

// ---------------------------------------------------------------------------
// OpenAI implementation
// ---------------------------------------------------------------------------

/// Sampling handler backed by the OpenAI Chat Completions API.
#[derive(Debug)]
pub struct OpenAISamplingHandler {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl OpenAISamplingHandler {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: "gpt-4o".to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    fn map_tool_choice(choice: &Option<ToolChoice>) -> Value {
        match choice {
            None => Value::String("auto".to_string()),
            Some(tc) => match tc.mode {
                ToolChoiceMode::Auto => Value::String("auto".to_string()),
                ToolChoiceMode::Required => Value::String("required".to_string()),
                ToolChoiceMode::None => Value::String("none".to_string()),
            },
        }
    }
}

#[async_trait]
impl SamplingHandler for OpenAISamplingHandler {
    async fn create_message(
        &self,
        messages: Vec<SamplingMessage>,
        params: CreateMessageRequestParams,
    ) -> Result<CreateMessageResult, FastMCPError> {
        let openai_messages: Vec<Value> = messages
            .iter()
            .map(|m| {
                let role = match &m.role {
                    SamplingRole::User => "user",
                    SamplingRole::Assistant => "assistant",
                };
                let content = match &m.content {
                    SamplingContent::Text(t) => t.text.clone(),
                };
                serde_json::json!({ "role": role, "content": content })
            })
            .collect();

        let tool_choice = Self::map_tool_choice(&params.tool_choice);

        let body = serde_json::json!({
            "model": self.model,
            "messages": openai_messages,
            "tool_choice": tool_choice,
        });

        let resp = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| FastMCPError::new(e.to_string()))?;

        let resp_json: Value = resp
            .json()
            .await
            .map_err(|e| FastMCPError::new(e.to_string()))?;

        let text = resp_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let model = resp_json["model"].as_str().map(str::to_string);
        let stop_reason = resp_json["choices"][0]["finish_reason"]
            .as_str()
            .map(str::to_string);

        Ok(CreateMessageResult {
            role: SamplingRole::Assistant,
            content: SamplingContent::Text(SamplingTextContent {
                type_: "text".to_string(),
                text,
            }),
            model,
            stop_reason,
        })
    }
}

// ---------------------------------------------------------------------------
// Client integration
// ---------------------------------------------------------------------------

/// Extension methods for wiring a [`SamplingHandler`] into the MCP client.
pub trait ClientSamplingExt {
    fn set_sampling_handler(&self, handler: Arc<dyn SamplingHandler>);
}

impl ClientSamplingExt for crate::client::Client {
    fn set_sampling_handler(&self, handler: Arc<dyn SamplingHandler>) {
        self.register_handler("sampling/createMessage", move |req| {
            let handler = handler.clone();
            async move {
                let params: CreateMessageRequestParams =
                    serde_json::from_value(req.params.unwrap_or_default())
                        .map_err(|e| FastMCPError::new(e.to_string()))?;
                let messages = params.messages.clone();
                let result = handler.create_message(messages, params).await?;
                Ok(Some(
                    serde_json::to_value(result)
                        .map_err(|e| FastMCPError::new(e.to_string()))?,
                ))
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_choice_mapping() {
        let choice = Some(ToolChoice {
            mode: ToolChoiceMode::Required,
            name: None,
        });
        let val = OpenAISamplingHandler::map_tool_choice(&choice);
        assert_eq!(val, Value::String("required".to_string()));
    }

    #[test]
    fn test_prompt_result_serialisation() {
        let result = CreateMessageResult {
            role: SamplingRole::Assistant,
            content: SamplingContent::Text(SamplingTextContent {
                type_: "text".to_string(),
                text: "hello".to_string(),
            }),
            model: Some("claude-sonnet-4-6".to_string()),
            stop_reason: Some("end_turn".to_string()),
        };
        let v = serde_json::to_value(&result).unwrap();
        assert_eq!(v["stopReason"], "end_turn");
    }
}
