use crate::error::FastMCPError;
use crate::mcp::types::JsonRpcRequest;
use crate::server::auth::{AuthContext, AuthProvider};
use async_trait::async_trait;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use reqwest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<JwkKey>,
}

#[derive(Debug, Deserialize)]
struct JwkKey {
    kid: String,
    // kty is often used in logic but if unused by us, allow dead code
    #[allow(dead_code)]
    kty: String,
    n: String,
    e: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    // iss and aud are verified by validator, but decoded into struct
    iss: String,
    aud: String,
    exp: usize,
    // Optional
    name: Option<String>,
    email: Option<String>,
}

#[derive(Debug)]
pub struct OIDCProvider {
    issuer: String,
    client_id: String,
    jwks_url: String,
}

impl OIDCProvider {
    pub async fn new(issuer_url: &str, client_id: &str) -> Result<Self, FastMCPError> {
        // Discovery to find jwks_uri
        let discovery_url = format!(
            "{}/.well-known/openid-configuration",
            issuer_url.trim_end_matches('/')
        );
        let client = reqwest::Client::new();
        let resp = client
            .get(&discovery_url)
            .send()
            .await
            .map_err(|e| FastMCPError::new(format!("Discovery failed: {}", e)))?;

        let config: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| FastMCPError::new(format!("Invalid discovery JSON: {}", e)))?;

        let jwks_url = config
            .get("jwks_uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| FastMCPError::new("Missing jwks_uri in discovery document".to_string()))?
            .to_string();

        Ok(Self {
            issuer: issuer_url.to_string(),
            client_id: client_id.to_string(),
            jwks_url,
        })
    }

    async fn fetch_jwk(&self, kid: &str) -> Result<JwkKey, FastMCPError> {
        // In prod, CACHE this!
        let client = reqwest::Client::new();
        let resp = client
            .get(&self.jwks_url)
            .send()
            .await
            .map_err(|e| FastMCPError::new(format!("JWKS fetch failed: {}", e)))?;

        let jwks: Jwks = resp
            .json()
            .await
            .map_err(|e| FastMCPError::new(format!("Invalid JWKS JSON: {}", e)))?;

        jwks.keys
            .into_iter()
            .find(|k| k.kid == kid)
            .ok_or_else(|| FastMCPError::new(format!("Key ID {} not found in JWKS", kid)))
    }
}

#[async_trait]
impl AuthProvider for OIDCProvider {
    async fn verify(&self, request: &JsonRpcRequest) -> Result<AuthContext, FastMCPError> {
        // Extract token
        // Extract token
        let token_str = request
            .transport_metadata
            .as_ref()
            .and_then(|m| m.get("Authorization").or_else(|| m.get("authorization")))
            .and_then(|h| h.strip_prefix("Bearer "))
            .or_else(|| {
                request
                    .params
                    .as_ref()
                    .and_then(|p| p.get("token"))
                    .and_then(|v| v.as_str())
            })
            .ok_or_else(|| FastMCPError::InvalidRequest("Missing token".to_string()))?;

        // 1. Decode Header to find KID
        let header = decode_header(token_str)
            .map_err(|e| FastMCPError::InvalidRequest(format!("Invalid Token Header: {}", e)))?;

        let kid = header.kid.ok_or_else(|| {
            FastMCPError::InvalidRequest("Missing kid in token header".to_string())
        })?;

        // 2. Fetch JWK
        let jwk = self.fetch_jwk(&kid).await?;

        // 3. Construct Validation
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[&self.client_id]);
        validation.set_issuer(&[&self.issuer]);

        let key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
            .map_err(|e| FastMCPError::new(format!("Invalid key components: {}", e)))?;

        let token_data = decode::<Claims>(token_str, &key, &validation).map_err(|e| {
            FastMCPError::InvalidRequest(format!("Token Verification Failed: {}", e))
        })?;

        Ok(AuthContext {
            client_id: Some(self.client_id.clone()),
            user_id: Some(token_data.claims.sub),
            scopes: vec![],
        })
    }
}
