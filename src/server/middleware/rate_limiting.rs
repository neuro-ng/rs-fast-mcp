use crate::error::FastMCPError;
use crate::mcp::types::{JsonRpcRequest, JsonRpcResponse};
use crate::server::middleware::{BoxFuture, Middleware, Next};
use dashmap::DashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone)]
struct TokenBucket {
    capacity: f64,
    refill_rate: f64,
    tokens: f64,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            capacity,
            refill_rate,
            tokens: capacity,
            last_refill: Instant::now(),
        }
    }

    fn consume(&mut self, amount: f64) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();

        // Refill, but don't exceed capacity
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
        self.last_refill = now;

        if self.tokens >= amount {
            self.tokens -= amount;
            true
        } else {
            false
        }
    }

    fn retry_after(&self) -> f64 {
        if self.tokens >= 1.0 {
            0.0
        } else {
            (1.0 - self.tokens) / self.refill_rate
        }
    }
}

pub struct RateLimitMiddleware {
    buckets: DashMap<String, Arc<Mutex<TokenBucket>>>,
    default_capacity: f64,
    default_refill_rate: f64,
    // global_limit: bool, // Implement if needed
    get_client_id: Box<dyn Fn(&JsonRpcRequest) -> String + Send + Sync>,
}

impl RateLimitMiddleware {
    pub fn new(capacity: f64, rate: f64) -> Self {
        Self {
            buckets: DashMap::new(),
            default_capacity: capacity,
            default_refill_rate: rate,
            get_client_id: Box::new(|_| "global".to_string()),
        }
    }

    pub fn per_client(capacity: f64, rate: f64) -> Self {
        Self {
            buckets: DashMap::new(),
            default_capacity: capacity,
            default_refill_rate: rate,
            // Simple heuristic since we don't have AuthContext here easily unless attached to request
            // But we can check for "clientId" in params or rely on future integration.
            // For now, we group by "unknown" if not found, effectively global for unauthed?
            // Or maybe IP based? (Transport specific).
            // Let's use a dummy ID for now.
            get_client_id: Box::new(|_| "client".to_string()),
        }
    }

    pub fn with_client_extractor<F>(mut self, extractor: F) -> Self
    where
        F: Fn(&JsonRpcRequest) -> String + Send + Sync + 'static,
    {
        self.get_client_id = Box::new(extractor);
        self
    }
}

impl Middleware for RateLimitMiddleware {
    fn handle<'a, 'b>(
        &'a self,
        req: JsonRpcRequest,
        next: Next<'b>,
    ) -> BoxFuture<'a, Result<JsonRpcResponse, FastMCPError>>
    where
        'b: 'a,
    {
        Box::pin(async move {
            let client_id = (self.get_client_id)(&req);

            // Get or create bucket
            let bucket = self
                .buckets
                .entry(client_id)
                .or_insert_with(|| {
                    Arc::new(Mutex::new(TokenBucket::new(
                        self.default_capacity,
                        self.default_refill_rate,
                    )))
                })
                .clone();

            let (allowed, retry_after) = {
                let mut b = bucket.lock().unwrap();
                let allowed = b.consume(1.0);
                (allowed, b.retry_after())
            };

            if allowed {
                next(req).await
            } else {
                let msg = format!(
                    "Rate limit exceeded. Retry after {:.2} seconds",
                    retry_after
                );
                Err(FastMCPError::InvalidRequest(msg))
            }
        })
    }
}
