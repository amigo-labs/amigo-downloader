//! Protocol backends: HTTP, Usenet, HLS, DASH.

pub mod dash;
pub mod hls;
pub mod http;
pub mod usenet;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Protocol {
    Http,
    Hls,
    Dash,
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
