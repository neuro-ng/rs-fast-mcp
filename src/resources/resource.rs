use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum ResourceContent {
    Text(String),
    Binary(Vec<u8>),
}

pub type ReadHandler = Box<
    dyn Fn() -> Pin<
            Box<dyn Future<Output = Result<ResourceContent, crate::error::FastMCPError>> + Send>,
        > + Send
        + Sync,
>;

#[derive(Clone)]
pub struct Resource {
    pub uri: String,
    pub name: String,
    pub mime_type: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub read_fn: Option<Arc<ReadHandler>>,
}
