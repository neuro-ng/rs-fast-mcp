use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    #[serde(rename = "accessJwt")]
    pub access_jwt: String,
    #[serde(rename = "refreshJwt")]
    pub refresh_jwt: String,
    pub handle: String,
    pub did: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Profile {
    pub did: String,
    pub handle: String,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub avatar: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FeedItem {
    pub uri: String,
    pub cid: String,
    pub post: PostView,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostView {
    pub uri: String,
    pub cid: String,
    pub author: Profile,
    pub record: serde_json::Value,
    #[serde(rename = "replyCount")]
    pub reply_count: Option<i32>,
    #[serde(rename = "repostCount")]
    pub repost_count: Option<i32>,
    #[serde(rename = "likeCount")]
    pub like_count: Option<i32>,
}
