use serde::{Deserialize, Serialize};

/// The role of the message sender in a prompt conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
}

/// The content payload of a prompt message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Structured(serde_json::Value),
    ContentBlock(crate::mcp::types::ContentBlock),
}

/// A single message in a prompt conversation sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub content: MessageContent,
    pub role: MessageRole,
}

impl Message {
    pub fn user(content: String) -> Self {
        Self {
            content: MessageContent::Text(content),
            role: MessageRole::User,
        }
    }

    pub fn assistant(content: String) -> Self {
        Self {
            content: MessageContent::Text(content),
            role: MessageRole::Assistant,
        }
    }

    /// Create a message whose content is auto-serialised from any `Serialize` value.
    pub fn structured<T: Serialize>(
        value: &T,
        role: MessageRole,
    ) -> Result<Self, serde_json::Error> {
        let serialized = serde_json::to_value(value)?;
        Ok(Self {
            content: MessageContent::Structured(serialized),
            role,
        })
    }
}

/// The typed result returned from rendering a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptResult {
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

impl PromptResult {
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            description: None,
            meta: None,
        }
    }
}

/// Convert a typed `Message` into the MCP wire-protocol `PromptMessage`.
impl From<Message> for crate::prompts::prompt::PromptMessage {
    fn from(msg: Message) -> Self {
        let role = match msg.role {
            MessageRole::User => "user".to_string(),
            MessageRole::Assistant => "assistant".to_string(),
        };
        let content = match msg.content {
            MessageContent::Text(t) => crate::mcp::types::ContentBlock::Text(
                crate::mcp::types::TextContent {
                    type_: "text".to_string(),
                    text: t,
                    annotations: None,
                },
            ),
            MessageContent::Structured(v) => crate::mcp::types::ContentBlock::Text(
                crate::mcp::types::TextContent {
                    type_: "text".to_string(),
                    text: serde_json::to_string(&v).unwrap_or_default(),
                    annotations: None,
                },
            ),
            MessageContent::ContentBlock(cb) => cb,
        };
        crate::prompts::prompt::PromptMessage { role, content }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_message() {
        let m = Message::user("hi".to_string());
        assert_eq!(m.role, MessageRole::User);
        assert!(matches!(&m.content, MessageContent::Text(t) if t == "hi"));
    }

    #[test]
    fn test_assistant_message() {
        let m = Message::assistant("pong".to_string());
        assert_eq!(m.role, MessageRole::Assistant);
    }

    #[test]
    fn test_structured_message() {
        #[derive(Serialize)]
        struct Data {
            x: i32,
        }
        let m = Message::structured(&Data { x: 7 }, MessageRole::User).unwrap();
        assert!(matches!(&m.content, MessageContent::Structured(_)));
    }

    #[test]
    fn test_prompt_result_new() {
        let r = PromptResult::new(vec![Message::user("hello".to_string())]);
        assert_eq!(r.messages.len(), 1);
        assert!(r.description.is_none());
    }

    #[test]
    fn test_message_into_prompt_message() {
        let m = Message::user("text".to_string());
        let pm: crate::prompts::prompt::PromptMessage = m.into();
        assert_eq!(pm.role, "user");
    }

    #[test]
    fn test_structured_message_into_prompt_message() {
        let m = Message::structured(&serde_json::json!({"k":"v"}), MessageRole::Assistant).unwrap();
        let pm: crate::prompts::prompt::PromptMessage = m.into();
        assert_eq!(pm.role, "assistant");
    }
}
