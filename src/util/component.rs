//! Generic component wrapper shared by tools, resources, and prompts.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Generic base type shared by tools, resources, and prompts.
///
/// `Component<T>` carries the common envelope fields (name, description, tags,
/// enabled flag, etc.) together with type-specific data `T`.
///
/// You rarely construct this directly — use the type aliases instead:
/// [`Tool`](crate::tools::tool::Tool),
/// [`Prompt`](crate::prompts::prompt::Prompt), and the resource types in
/// [`crate::resources`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component<T> {
    /// Unique identifier within its manager (tool name, resource URI, prompt name).
    pub name: String,
    /// Optional human-readable display title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Human-readable description exposed to clients.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Arbitrary labels used for tag-based filtering.
    #[serde(default)]
    pub tags: HashSet<String>,
    /// Arbitrary key-value metadata for application use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, Value>>,
    /// When `false` the component is registered but excluded from listings.
    pub enabled: bool,
    /// Optional stable key for external reference (e.g. configuration file id).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    /// Type-specific data (e.g. `ToolKind`, `PromptFunction`).
    #[serde(flatten)]
    pub data: T,
}

/// Whether a component lives on this server or was mirrored from a remote one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MirroredFlag {
    /// Component registered locally.
    Local,
    /// Component mirrored via [`MountedServer`](crate::server::proxy::MountedServer).
    Mirrored,
}

/// A [`Component`] annotated with its mirror origin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirroredComponent<T> {
    #[serde(flatten)]
    pub component: Component<T>,
    pub mirrored: MirroredFlag,
}
