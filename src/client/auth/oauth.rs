use crate::client::auth::AuthHandler;
use crate::error::FastMCPError;
use async_trait::async_trait;
use reqwest::Client as HttpClient;
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    #[allow(dead_code)]
    token_type: String,
}

struct TokenCache {
    access_token: String,
    expires_at: Instant,
}

pub struct OAuthClientCredentials {
    token_url: String,
    client_id: String,
    client_secret: String,
    scopes: Vec<String>,
    http_client: HttpClient,
    cache: Arc<RwLock<Option<TokenCache>>>,
}

impl OAuthClientCredentials {
    pub fn new(token_url: &str, client_id: &str, client_secret: &str, scopes: Vec<String>) -> Self {
        Self {
            token_url: token_url.to_string(),
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            scopes,
            http_client: HttpClient::new(),
            cache: Arc::new(RwLock::new(None)),
        }
    }

    async fn fetch_token(&self) -> Result<TokenResponse, FastMCPError> {
        let params = [
            ("grant_type", "client_credentials"),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
            ("scope", &self.scopes.join(" ")),
        ];

        let resp = self
            .http_client
            .post(&self.token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| FastMCPError::new(format!("Failed to fetch token: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(FastMCPError::new(format!(
                "Token request failed {}: {}",
                status, text
            )));
        }

        resp.json::<TokenResponse>()
            .await
            .map_err(|e| FastMCPError::new(format!("Failed to parse token response: {}", e)))
    }
}

#[async_trait]
impl AuthHandler for OAuthClientCredentials {
    async fn get_auth_header(&self) -> Result<Option<String>, FastMCPError> {
        // Check cache
        {
            let cache_read = self.cache.read().await;
            if let Some(cached) = &*cache_read
                && cached.expires_at > Instant::now()
            {
                return Ok(Some(format!("Bearer {}", cached.access_token)));
            }
        }

        // Refresh
        // Write lock ensures only one client fetches token (serialized)
        let mut cache_write = self.cache.write().await;

        // Double check after lock
        if let Some(cached) = &*cache_write
            && cached.expires_at > Instant::now()
        {
            return Ok(Some(format!("Bearer {}", cached.access_token)));
        }

        let token_resp = self.fetch_token().await?;
        let expires_at = Instant::now() + Duration::from_secs(token_resp.expires_in - 30); // Buffer 30s

        *cache_write = Some(TokenCache {
            access_token: token_resp.access_token.clone(),
            expires_at,
        });

        Ok(Some(format!("Bearer {}", token_resp.access_token)))
    }
}
