use crate::error::FastMCPError;
use crate::mcp::types::{JsonRpcRequest, JsonRpcResponse};
use crate::server::middleware::{BoxFuture, Middleware, Next};
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
struct CacheEntry {
    response: JsonRpcResponse,
    created_at: Instant,
    ttl: Duration,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }
}

// Key is (Method, Params JSON String)
type CacheKey = (String, String);

pub struct CacheMiddleware {
    cache: Arc<Mutex<LruCache<CacheKey, CacheEntry>>>,
    default_ttl: Duration,
}

impl CacheMiddleware {
    pub fn new(capacity: usize, default_ttl_secs: u64) -> Self {
        let cap = NonZeroUsize::new(capacity).expect("Capacity must be non-zero");
        Self {
            cache: Arc::new(Mutex::new(LruCache::new(cap))),
            default_ttl: Duration::from_secs(default_ttl_secs),
        }
    }

    fn get_cache_key(req: &JsonRpcRequest) -> Option<CacheKey> {
        // Only cache specific methods? Or all?
        // OCaml caches: tools/list, resources/list, prompts/list, tools/call, resources/read, prompts/get
        // For simplicity, let's cache everything that looks like a read/call if configured?
        // But for this middleware, we might want to just cache everything passed through it,
        // expecting the user to mount it selectively or checking method names.

        let should_cache = matches!(
            req.method.as_str(),
            "tools/list"
                | "resources/list"
                | "prompts/list"
                | "tools/call"
                | "resources/read"
                | "prompts/get"
        );

        if !should_cache {
            return None;
        }

        let params_str = match &req.params {
            Some(v) => v.to_string(),
            None => "null".to_string(),
        };

        Some((req.method.clone(), params_str))
    }
}

impl Middleware for CacheMiddleware {
    fn handle<'a, 'b>(
        &'a self,
        req: JsonRpcRequest,
        next: Next<'b>,
    ) -> BoxFuture<'a, Result<JsonRpcResponse, FastMCPError>>
    where
        'b: 'a,
    {
        Box::pin(async move {
            let key_opt = Self::get_cache_key(&req);

            if let Some(key) = key_opt.clone() {
                let mut cache = self.cache.lock().unwrap();
                if let Some(entry) = cache.get(&key) {
                    if !entry.is_expired() {
                        // Return cached response with updated ID to match current request
                        let mut resp = entry.response.clone();
                        resp.id = req.id.clone();
                        return Ok(resp);
                    } else {
                        cache.pop(&key);
                    }
                }
            }

            // Cache miss or expired
            let result = next(req).await;

            if let Ok(resp) = &result
                && let Some(key) = key_opt
            {
                let entry = CacheEntry {
                    response: resp.clone(),
                    created_at: Instant::now(),
                    ttl: self.default_ttl,
                };
                self.cache.lock().unwrap().put(key, entry);
            }

            result
        })
    }
}
