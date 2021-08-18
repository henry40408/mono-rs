use serde::{Deserialize, Serialize};

#[derive(Default, Serialize)]
pub struct Request {
    pub token: String,
    pub user: String,
    pub device: Option<String>,
    pub title: Option<String>,
    pub message: String,
    pub html: Option<u8>,
    pub timestamp: Option<u64>,
    pub priority: Option<u8>,
    pub url: Option<String>,
    pub url_title: Option<String>,
    pub sound: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Response {
    pub status: u64,
    pub request: String,
    pub errors: Option<Vec<String>>,
}
