use crate::server::app::Server;
use crate::server::core::FastMCPServer;
use crate::server::transport::{Transport, http::HttpTransport, stdio::StdioTransport};

use crate::server::middleware::Middleware;
use std::sync::Arc;

/// Fluent builder for configuring and constructing a [`Server`].
///
/// # Example
///
/// ```rust,no_run
/// use rs_fast_mcp::server::app::Server;
///
/// let server = Server::builder("my-server", "0.1.0")
///     .stdio()
///     .build();
/// ```
pub struct ServerBuilder {
    name: String,
    version: String,
    transports: Vec<Box<dyn Transport>>,
    middlewares: Vec<Arc<dyn Middleware>>,
    include_tags: Vec<String>,
    exclude_tags: Vec<String>,
}

impl ServerBuilder {
    /// Use [`Server::builder`] instead of calling this directly.
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            transports: Vec::new(),
            middlewares: Vec::new(),
            include_tags: Vec::new(),
            exclude_tags: Vec::new(),
        }
    }

    /// Adds the stdin/stdout transport.
    pub fn stdio(mut self) -> Self {
        self.transports.push(Box::new(StdioTransport::new()));
        self
    }

    /// Adds the HTTP/SSE transport on the given host and port.
    pub fn http(mut self, host: &str, port: u16) -> Self {
        self.transports
            .push(Box::new(HttpTransport::new(host, port)));
        self
    }

    /// Adds a custom [`Transport`] implementation.
    pub fn with_transport(mut self, transport: Box<dyn Transport>) -> Self {
        self.transports.push(transport);
        self
    }

    /// Registers an [`AuthProvider`](crate::server::auth::AuthProvider) as middleware.
    pub fn with_auth(mut self, provider: Arc<dyn crate::server::auth::AuthProvider>) -> Self {
        let mw = crate::server::auth::AuthMiddleware::new(provider);
        self.middlewares.push(Arc::new(mw));
        self
    }

    /// Convenience: registers a [`SimpleAuthProvider`](crate::server::auth::SimpleAuthProvider) with a static token.
    pub fn with_simple_auth(self, token: &str) -> Self {
        let provider = Arc::new(crate::server::auth::SimpleAuthProvider::new(token));
        self.with_auth(provider)
    }

    /// Only expose components matching any of these tags.
    pub fn include_tags(mut self, tags: Vec<String>) -> Self {
        self.include_tags = tags;
        self
    }

    /// Hide components matching any of these tags.
    pub fn exclude_tags(mut self, tags: Vec<String>) -> Self {
        self.exclude_tags = tags;
        self
    }

    /// Finalises the builder and returns a ready-to-run [`Server`].
    pub fn build(self) -> Server {
        let core = FastMCPServer::new(&self.name, &self.version);
        for mw in self.middlewares {
            core.add_middleware_arc(mw);
        }
        core.set_filtering(self.include_tags, self.exclude_tags);
        Server::new(core, self.transports)
    }
}
