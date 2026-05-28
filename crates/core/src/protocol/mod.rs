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

/// Resolve once the cancellation flag flips to `true`.
///
/// Cancellation is carried over a `watch::<bool>` channel (instead of a
/// one-shot) so the same signal survives across every retry attempt — a
/// download stuck in the retry loop must still be cancellable. If the sender
/// is dropped (download finished, nobody can cancel any more) this future
/// stays pending forever so a `select!` never spuriously resolves it.
pub async fn wait_for_cancel(rx: &mut watch::Receiver<bool>) {
    if *rx.borrow() {
        return;
    }
    while rx.changed().await.is_ok() {
        if *rx.borrow() {
            return;
        }
    }
    std::future::pending::<()>().await;
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
        cancel_rx: watch::Receiver<bool>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn wait_for_cancel_resolves_when_flag_already_true() {
        let (tx, mut rx) = watch::channel(false);
        tx.send_replace(true);
        // Already cancelled: must resolve immediately, not hang.
        tokio::time::timeout(std::time::Duration::from_secs(1), wait_for_cancel(&mut rx))
            .await
            .expect("should resolve immediately when flag is already true");
    }

    #[tokio::test]
    async fn wait_for_cancel_resolves_when_flag_flips() {
        let (tx, mut rx) = watch::channel(false);
        let waiter = tokio::spawn(async move { wait_for_cancel(&mut rx).await });
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        tx.send_replace(true);
        tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .expect("should resolve once flag flips")
            .expect("task should not panic");
    }

    #[tokio::test]
    async fn wait_for_cancel_survives_across_cloned_receivers() {
        // Each retry attempt clones a fresh receiver from the same sender;
        // a single cancel must be observable by a receiver cloned afterwards.
        let (tx, rx) = watch::channel(false);
        tx.send_replace(true);
        let mut attempt_rx = rx.clone();
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            wait_for_cancel(&mut attempt_rx),
        )
        .await
        .expect("cloned receiver must observe the cancellation");
    }

    #[tokio::test]
    async fn wait_for_cancel_stays_pending_without_cancel() {
        let (_tx, mut rx) = watch::channel(false);
        // No cancel sent: the future must not resolve.
        let res = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            wait_for_cancel(&mut rx),
        )
        .await;
        assert!(res.is_err(), "should still be pending when not cancelled");
    }
}
