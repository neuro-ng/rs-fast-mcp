//! OAuth proxy configuration and consent-screen utilities.
//!
//! When an MCP server acts as an OAuth proxy it may render an HTML consent
//! screen for the user. The `consent_csp_policy` field allows callers to
//! supply a custom Content-Security-Policy value that is injected into the
//! consent page's `<meta http-equiv="Content-Security-Policy">` tag.
//!
//! **Security note**: the policy string is HTML-escaped before insertion to
//! prevent quote-breakout XSS attacks.

/// Configuration for an OAuth/OIDC proxy endpoint.
#[derive(Debug, Clone)]
pub struct OAuthProxyConfig {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub auth_url: String,
    pub token_url: String,
    pub redirect_url: String,
    /// Custom CSP policy injected into the consent `<meta>` tag.
    ///
    /// Defaults to `"default-src 'none'"` when `None`.
    pub consent_csp_policy: Option<String>,
}

impl OAuthProxyConfig {
    pub fn new(
        client_id: impl Into<String>,
        auth_url: impl Into<String>,
        token_url: impl Into<String>,
        redirect_url: impl Into<String>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: None,
            auth_url: auth_url.into(),
            token_url: token_url.into(),
            redirect_url: redirect_url.into(),
            consent_csp_policy: None,
        }
    }

    pub fn with_client_secret(mut self, secret: impl Into<String>) -> Self {
        self.client_secret = Some(secret.into());
        self
    }

    pub fn with_consent_csp_policy(mut self, policy: impl Into<String>) -> Self {
        self.consent_csp_policy = Some(policy.into());
        self
    }

    /// Render the consent-page `<meta>` CSP tag with the configured policy.
    ///
    /// The policy value is HTML-escaped to prevent injection attacks.
    pub fn consent_csp_meta_tag(&self) -> String {
        let policy = self
            .consent_csp_policy
            .as_deref()
            .unwrap_or("default-src 'none'");
        format!(
            r#"<meta http-equiv="Content-Security-Policy" content="{}">"#,
            html_escape(policy)
        )
    }
}

/// Escape `<`, `>`, `"`, `'`, and `&` in `s` for safe HTML attribute insertion.
pub fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_csp_meta_tag() {
        let cfg = OAuthProxyConfig::new("id", "https://a", "https://b", "https://c");
        let tag = cfg.consent_csp_meta_tag();
        assert!(tag.contains("default-src"));
        assert!(tag.contains(r#"content=""#));
    }

    #[test]
    fn test_custom_csp_policy() {
        let cfg = OAuthProxyConfig::new("id", "https://a", "https://b", "https://c")
            .with_consent_csp_policy("default-src 'self'");
        let tag = cfg.consent_csp_meta_tag();
        assert!(tag.contains("default-src &#x27;self&#x27;"));
    }

    #[test]
    fn test_html_escape_prevents_injection() {
        let malicious = r#"" onload="evil()"#;
        let escaped = html_escape(malicious);
        assert!(!escaped.contains('"'));
        assert!(escaped.contains("&quot;"));
    }
}
