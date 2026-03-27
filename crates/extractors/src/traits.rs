use serde::{Deserialize, Serialize};

use crate::error::ExtractorError;

/// Protocol for a resolved media stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StreamProtocol {
    Http,
    Hls,
    Dash,
}

/// A single downloadable media stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaStream {
    pub url: String,
    pub protocol: StreamProtocol,
    pub quality_label: String,
    pub height: u32,
    pub mime_type: String,
    pub filesize: Option<u64>,
    pub has_audio: bool,
    pub has_video: bool,
}

/// Result of extracting media from a page URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedMedia {
    pub title: String,
    pub streams: Vec<MediaStream>,
}

/// Trait that all site-specific extractors implement.
#[async_trait::async_trait]
pub trait Extractor: Send + Sync {
    /// Human-readable name of the extractor.
    fn name(&self) -> &str;

    /// Check if this extractor handles the given URL.
    fn supports_url(&self, url: &str) -> bool;

    /// Extract media streams from the given URL.
    async fn extract(
        &self,
        client: &reqwest::Client,
        url: &str,
    ) -> Result<ExtractedMedia, ExtractorError>;
}
