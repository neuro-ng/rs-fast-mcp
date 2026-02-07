use crate::atproto::types::{FeedItem, Session};
use reqwest::{Client as HttpClient, Url};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Client {
    http: HttpClient,
    base_url: Url,
    session: Arc<RwLock<Option<Session>>>,
}

impl Client {
    pub fn new() -> Self {
        Self {
            http: HttpClient::new(),
            base_url: Url::parse("https://bsky.social").unwrap(),
            session: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn login(&self, identifier: &str, password: &str) -> Result<String, String> {
        let url = self
            .base_url
            .join("/xrpc/com.atproto.server.createSession")
            .unwrap();
        let resp = self
            .http
            .post(url)
            .json(&json!({
                "identifier": identifier,
                "password": password
            }))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("Login failed: {}", resp.status()));
        }

        let session: Session = resp.json().await.map_err(|e| e.to_string())?;
        let handle = session.handle.clone();

        let mut guard = self.session.write().await;
        *guard = Some(session);

        Ok(handle)
    }

    async fn get_token(&self) -> Result<String, String> {
        let guard = self.session.read().await;
        guard
            .as_ref()
            .map(|s| s.access_jwt.clone())
            .ok_or_else(|| "Not logged in".to_string())
    }

    pub async fn get_timeline(&self) -> Result<Vec<FeedItem>, String> {
        let token = self.get_token().await?;
        let url = self
            .base_url
            .join("/xrpc/app.bsky.feed.getTimeline")
            .unwrap();

        let resp = self
            .http
            .get(url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("Fetch timeline failed: {}", resp.status()));
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let feed: Vec<FeedItem> = serde_json::from_value(body["feed"].clone())
            .map_err(|e| format!("Parse error: {}", e))?;

        Ok(feed)
    }

    pub async fn create_record(
        &self,
        collection: &str,
        record: serde_json::Value,
    ) -> Result<String, String> {
        let token = self.get_token().await?;
        let guard = self.session.read().await;
        let repo = guard.as_ref().unwrap().did.clone();

        let url = self
            .base_url
            .join("/xrpc/com.atproto.repo.createRecord")
            .unwrap();

        let resp = self
            .http
            .post(url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&json!({
                "repo": repo,
                "collection": collection,
                "record": record
            }))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let txt = resp.text().await.unwrap_or_default();
            return Err(format!("Create record failed: {}", txt));
        }

        Ok("Created".to_string())
    }
}
