//! Client extension traits for roots capability and progress reporting.
//!
//! These helpers make it easier to handle server-originated requests
//! (`roots/list`, `sampling/createMessage`) and to send client-originated
//! notifications (`notifications/roots/list_changed`).

use crate::error::FastMCPError;
use crate::mcp::types::{ListRootsResult, Root};

/// A locally-registered filesystem root that the server can query.
#[derive(Debug, Clone)]
pub struct ClientRoot {
    /// `file://` URI pointing at the root directory.
    pub uri: String,
    /// Human-readable label shown to the server.
    pub name: Option<String>,
}

impl From<ClientRoot> for Root {
    fn from(r: ClientRoot) -> Self {
        Root {
            uri: r.uri,
            name: r.name,
        }
    }
}

/// Extends [`Client`](crate::client::Client) with roots-capability helpers.
pub trait ClientRootsExt {
    /// Register a handler for incoming `roots/list` requests.
    fn register_roots_handler(&self, roots: Vec<ClientRoot>);

    /// Notify the server that the roots list has changed.
    fn notify_roots_changed(&self) -> impl std::future::Future<Output = Result<(), FastMCPError>>;
}

impl ClientRootsExt for crate::client::Client {
    fn register_roots_handler(&self, roots: Vec<ClientRoot>) {
        let mcp_roots: Vec<Root> = roots.into_iter().map(Into::into).collect();
        self.register_handler("roots/list", move |_req| {
            let roots = mcp_roots.clone();
            async move {
                let result = ListRootsResult { roots };
                Ok(Some(
                    serde_json::to_value(result)
                        .map_err(|e| FastMCPError::new(e.to_string()))?,
                ))
            }
        });
    }

    async fn notify_roots_changed(&self) -> Result<(), FastMCPError> {
        use crate::mcp::types::{JsonRpcMessage, JsonRpcNotification};
        let notif = JsonRpcMessage::Notification(JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "notifications/roots/list_changed".to_string(),
            params: None,
        });
        self.send_notification(notif).await
    }
}
