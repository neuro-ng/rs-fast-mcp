use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component<T> {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: HashSet<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<HashMap<String, Value>>,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    #[serde(flatten)]
    pub data: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MirroredFlag {
    Local,
    Mirrored,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MirroredComponent<T> {
    #[serde(flatten)]
    pub component: Component<T>,
    pub mirrored: MirroredFlag,
}
