//! Shared types between host and plugins.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadInfo {
    pub url: String,
    pub filename: String,
    pub filesize: Option<u64>,
    pub chunks_supported: bool,
    pub max_chunks: Option<u32>,
    pub headers: Option<std::collections::HashMap<String, String>>,
    pub cookies: Option<std::collections::HashMap<String, String>>,
    pub wait_seconds: Option<u64>,
    pub mirrors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OnlineStatus {
    Online,
    Offline,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMeta {
    pub id: String,
    pub name: String,
    pub version: String,
    pub url_pattern: String,
}
