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
    pub download_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub filename: Option<String>,
    /// Extra headers (e.g. Referer, Authorization from plugin resolution).
    pub headers: HashMap<String, String>,
}

/// Unified trait for all protocol backends.
pub trait ProtocolBackend: Send + Sync {
    /// Which protocol this backend handles.
    fn protocol(&self) -> Protocol;

    /// Execute the download. Returns (bytes_downloaded, output_path).
    fn download(
        &self,
        job: &DownloadJob,
        progress_tx: watch::Sender<DownloadProgress>,
        cancel_rx: tokio::sync::oneshot::Receiver<()>,
    ) -> impl Future<Output = Result<(u64, PathBuf), crate::Error>> + Send;
}

use std::future::Future;
