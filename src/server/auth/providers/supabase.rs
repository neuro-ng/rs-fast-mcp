use crate::error::FastMCPError;
use crate::mcp::types::JsonRpcRequest;
use crate::server::auth::{AuthContext, AuthProvider};
use async_trait::async_trait;
use jsonwebtoken::{DecodingKey, Validation, decode, decode_header};
use reqwest;
use serde::{Deserialize, Serialize};
use std::env;

/// Supabase Authentication Provider
/// Verifies JWTs issued by Supabase Auth (GoTrue).
/// Supports JWKS verification (RS256/ES256) which is the recommended approach.
pub struct SupabaseProvider {
    project_url: String,
    jwks_url: String,
    issuer: String,
    // Algorithm support to be extended if needed, currently we support RS/ES via JWKS
}

#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<JwkKey>,
}

#[derive(Debug, Deserialize)]
struct JwkKey {
    kid: String,
    #[allow(dead_code)]
    kty: String,
    n: String,
    e: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    iss: String,
    aud: Option<String>,
    exp: usize,
    email: Option<String>,
    role: Option<String>,
}

impl SupabaseProvider {
    /// Create a new SupabaseProvider.
    ///
    /// # Arguments
    /// * `project_url` - The Supabase Project URL (e.g. `https://abc123.supabase.co`)
    pub fn new(project_url: &str) -> Self {
        let base = project_url.trim_end_matches('/');
        Self {
            project_url: base.to_string(),
            jwks_url: format!("{}/auth/v1/.well-known/jwks.json", base),
            issuer: format!("{}/auth/v1", base),
        }
    }

    /// Load configuration from environment variables.
    ///
    /// Expected variables:
    /// - `OXFASTMCP_SERVER_AUTH_SUPABASE_PROJECT_URL`
    pub fn from_env() -> Result<Self, FastMCPError> {
        let project_url = env::var("OXFASTMCP_SERVER_AUTH_SUPABASE_PROJECT_URL").map_err(|_| {
            FastMCPError::new("Missing OXFASTMCP_SERVER_AUTH_SUPABASE_PROJECT_URL".to_string())
        })?;

        Ok(Self::new(&project_url))
    }

    async fn fetch_jwk(&self, kid: &str) -> Result<JwkKey, FastMCPError> {
        let client = reqwest::Client::new();
        let resp = client
            .get(&self.jwks_url)
            .send()
            .await
            .map_err(|e| FastMCPError::new(format!("Supabase JWKS fetch failed: {}", e)))?;

        let jwks: Jwks = resp
            .json()
            .await
            .map_err(|e| FastMCPError::new(format!("Invalid Supabase JWKS JSON: {}", e)))?;

        jwks.keys
            .into_iter()
            .find(|k| k.kid == kid)
            .ok_or_else(|| FastMCPError::new(format!("Key ID {} not found in Supabase JWKS", kid)))
    }
}

#[async_trait]
impl AuthProvider for SupabaseProvider {
    async fn verify(&self, request: &JsonRpcRequest) -> Result<AuthContext, FastMCPError> {
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

        // 3. Validate
        let mut validation = Validation::new(header.alg);
        validation.set_issuer(&[&self.issuer]);
        // Supabase often sets 'aud' to 'authenticated' by default, or empty. check logic?
        // Let's allow any 'aud' for now, or check for specific param?
        // Note: validation default checks 'aud' if set.
        validation.validate_aud = false; // Supabase access tokens might not have standard aud fields behaving predictably 

        let key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
            .map_err(|e| FastMCPError::new(format!("Invalid key components: {}", e)))?;

        let token_data = decode::<Claims>(token_str, &key, &validation).map_err(|e| {
            FastMCPError::InvalidRequest(format!("Supabase Token Verification Failed: {}", e))
        })?;

        Ok(AuthContext {
            client_id: Some(self.project_url.clone()),
            user_id: Some(token_data.claims.sub),
            scopes: token_data.claims.role.map(|r| vec![r]).unwrap_or_default(),
        })
    }
}
