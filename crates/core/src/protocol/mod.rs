//! Protocol backends: HTTP, Usenet.

pub mod http;
pub mod usenet;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Protocol {
    Http,
    Usenet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub url: String,
    pub protocol: Protocol,
    pub filename: Option<String>,
    pub download_dir: Option<String>,
    pub priority: Option<i32>,
    pub package_id: Option<String>,
}
