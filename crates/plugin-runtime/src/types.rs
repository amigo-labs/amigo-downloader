//! Shared types between host and plugins.

use serde::{Deserialize, Serialize};

/// A download package — groups related downloads together.
/// Returned by a plugin's resolve() function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadPackage {
    /// Package name shown in the UI.
    pub name: String,
    /// The downloads in this package.
    pub downloads: Vec<DownloadInfo>,
}

/// A single downloadable file within a package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadInfo {
    pub url: String,
    pub filename: Option<String>,
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
    pub description: Option<String>,
    pub author: Option<String>,
}

/// HTTP response returned to plugins.
#[derive(Debug, Clone)]
pub struct PluginHttpResponse {
    pub status: u16,
    pub body: String,
    pub headers: std::collections::HashMap<String, String>,
}
