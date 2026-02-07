use chrono::Utc;
use serde_json::json;

pub fn create_post_record(text: &str) -> serde_json::Value {
    // Simple implementation without facets for now
    json!({
        "$type": "app.bsky.feed.post",
        "text": text,
        "createdAt": Utc::now().to_rfc3339()
    })
}
