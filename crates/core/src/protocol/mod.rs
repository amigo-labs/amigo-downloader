//! Protocol backends: HTTP, Usenet, HLS, DASH.

pub mod dash;
pub mod hls;
pub mod http;
pub mod usenet;

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use self::http::DownloadProgress;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Protocol {
    Http,
    Hls,
    Dash,
    Usenet,
}

/// Everything a protocol backend needs to perform a download.
#[derive(Debug, Clone)]
pub struct DownloadJob {
    pub url: String,
    pub download_id: String,
    pub download_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub filename: Option<String>,
    /// Extra headers (e.g. Referer, Authorization from plugin resolution).
    pub headers: HashMap<String, String>,
    /// HTTP-specific: max parallel chunks (0 = auto).
    pub max_chunks: u32,
    /// HTTP-specific: user agent string.
    pub user_agent: String,
}

/// Unified trait for all protocol backends.
#[async_trait::async_trait]
pub trait ProtocolBackend: Send + Sync {
    /// Which protocol this backend handles.
    fn protocol(&self) -> Protocol;

    /// Execute the download. Returns (bytes_downloaded, output_path).
    async fn download(
        &self,
        job: &DownloadJob,
        progress_tx: watch::Sender<DownloadProgress>,
        cancel_rx: tokio::sync::oneshot::Receiver<()>,
    ) -> Result<(u64, PathBuf), crate::Error>;
}

/// Result of resolving a URL through plugins or extractors.
#[derive(Debug, Clone)]
pub struct ResolvedDownload {
    /// The actual download URL (may differ from the original page URL).
    pub url: String,
    pub filename: Option<String>,
    pub filesize: Option<u64>,
    pub protocol: Protocol,
    /// Extra HTTP headers to use (e.g. Referer, Authorization).
    pub headers: HashMap<String, String>,
}

/// Trait for URL resolution — plugins, extractors, or custom resolvers.
///
/// Defined in core to avoid circular dependencies. Implemented in the server
/// crate by wrapping PluginLoader and Extractor registry.
#[async_trait::async_trait]
pub trait UrlResolver: Send + Sync {
    async fn resolve(&self, url: &str) -> Option<ResolvedDownload>;
}
