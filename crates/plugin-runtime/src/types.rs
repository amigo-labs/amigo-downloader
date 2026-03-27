//! Shared types between host and plugins.

use serde::{Deserialize, Serialize};

/// Information returned by a plugin's resolve() function.
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

/// Online check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OnlineStatus {
    Online,
    Offline,
    Unknown,
}

/// Metadata about a loaded plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMeta {
    pub id: String,
    pub name: String,
    pub version: String,
    pub url_pattern: String,
    pub file_path: String,
    pub enabled: bool,
}

/// HTTP response returned to plugins.
#[derive(Debug, Clone)]
pub struct PluginHttpResponse {
    pub status: u16,
    pub body: String,
    pub headers: std::collections::HashMap<String, String>,
}
