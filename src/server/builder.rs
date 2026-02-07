use crate::server::app::Server;
use crate::server::core::FastMCPServer;
use crate::server::transport::{Transport, http::HttpTransport, stdio::StdioTransport};

use crate::server::middleware::Middleware;
use std::sync::Arc;

pub struct ServerBuilder {
    name: String,
    version: String,
    transports: Vec<Box<dyn Transport>>,
    middlewares: Vec<Arc<dyn Middleware>>,
    include_tags: Vec<String>,
    exclude_tags: Vec<String>,
}

impl ServerBuilder {
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

    pub fn stdio(mut self) -> Self {
        self.transports.push(Box::new(StdioTransport::new()));
        self
    }

    pub fn http(mut self, host: &str, port: u16) -> Self {
        self.transports
            .push(Box::new(HttpTransport::new(host, port)));
        self
    }

    pub fn with_transport(mut self, transport: Box<dyn Transport>) -> Self {
        self.transports.push(transport);
        self
    }

    pub fn with_auth(mut self, provider: Arc<dyn crate::server::auth::AuthProvider>) -> Self {
        let mw = crate::server::auth::AuthMiddleware::new(provider);
        self.middlewares.push(Arc::new(mw));
        self
    }

    pub fn with_simple_auth(self, token: &str) -> Self {
        let provider = Arc::new(crate::server::auth::SimpleAuthProvider::new(token));
        self.with_auth(provider)
    }

    pub fn include_tags(mut self, tags: Vec<String>) -> Self {
        self.include_tags = tags;
        self
    }

    pub fn exclude_tags(mut self, tags: Vec<String>) -> Self {
        self.exclude_tags = tags;
        self
    }

    pub fn build(self) -> Server {
        let core = FastMCPServer::new(&self.name, &self.version);
        for mw in self.middlewares {
            core.add_middleware_arc(mw);
        }
        core.set_filtering(self.include_tags, self.exclude_tags);
        Server::new(core, self.transports)
    }
}
