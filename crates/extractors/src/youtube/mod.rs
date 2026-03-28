//! YouTube video stream extraction.
//!
//! Extracts direct download URLs from YouTube videos using the innertube API
//! with the `android_vr` client (Oculus Quest 3). Falls back to `web_embedded`
//! and watch page HTML extraction.

pub mod formats;
pub mod innertube;
pub mod n_challenge;
pub mod url_parser;

use tracing::{debug, info};

use crate::error::ExtractorError;
use crate::traits::{ExtractedMedia, Extractor, MediaStream, StreamProtocol};

pub use url_parser::{extract_video_id, is_youtube_url};

/// Resolved YouTube video with download-ready stream URL.
#[derive(Debug, Clone)]
pub struct YoutubeVideo {
    pub video_id: String,
    pub title: String,
    pub stream_url: String,
    pub filename: String,
    pub filesize: Option<u64>,
    pub mime_type: String,
    pub quality: String,
}

/// YouTube extractor implementing the `Extractor` trait.
pub struct YoutubeExtractor;

#[async_trait::async_trait]
impl Extractor for YoutubeExtractor {
    fn name(&self) -> &str {
        "YouTube"
    }

    fn supports_url(&self, url: &str) -> bool {
        is_youtube_url(url)
    }

    async fn extract(
        &self,
        client: &reqwest::Client,
        url: &str,
    ) -> Result<ExtractedMedia, ExtractorError> {
        let video = resolve(client, url).await?;
        Ok(ExtractedMedia {
            title: video.title,
            streams: vec![MediaStream {
                url: video.stream_url,
                protocol: StreamProtocol::Http,
                quality_label: video.quality,
                height: 0, // already selected best
                mime_type: video.mime_type,
                filesize: video.filesize,
                has_audio: true,
                has_video: true,
            }],
        })
    }
}

/// Fetch video info and select the best stream.
pub async fn resolve(client: &reqwest::Client, url: &str) -> Result<YoutubeVideo, ExtractorError> {
    let video_id = extract_video_id(url)?;
    info!("Resolving YouTube video: {video_id}");

    let player = innertube::fetch_player(client, &video_id).await?;

    let title = player
        .pointer("/videoDetails/title")
        .and_then(|v| v.as_str())
        .unwrap_or("video")
        .to_string();

    debug!("Video title: {title}");

    // Collect all formats with direct URLs
    let mut all_formats = Vec::new();

    // Combined formats (audio + video) — preferred for single-file download
    if let Some(fmts) = player
        .pointer("/streamingData/formats")
        .and_then(|v| v.as_array())
    {
        for fmt in fmts {
            if let Some(info) = formats::parse_format(fmt) {
                all_formats.push(info);
            }
        }
    }

    // Adaptive formats
    if let Some(fmts) = player
        .pointer("/streamingData/adaptiveFormats")
        .and_then(|v| v.as_array())
    {
        for fmt in fmts {
            if let Some(info) = formats::parse_format(fmt) {
                all_formats.push(info);
            }
        }
    }

    if all_formats.is_empty() {
        let reason = player
            .pointer("/playabilityStatus/reason")
            .and_then(|v| v.as_str())
            .unwrap_or("No downloadable streams found");
        return Err(ExtractorError::NoStreams(reason.to_string()));
    }

    let best_idx = formats::select_best_format(&all_formats)
        .ok_or_else(|| ExtractorError::NoStreams("No suitable format found".into()))?;

    let best = &all_formats[best_idx];
    info!(
        "Selected format: {} ({})",
        best.quality_label, best.mime_type
    );

    let mut stream_url = best.url.clone();

    // Try N-parameter transformation to bypass throttling
    match innertube::fetch_player_js_url(client, &video_id).await {
        Ok(player_js_url) => {
            match n_challenge::transform_n_param(client, &player_js_url, &stream_url).await {
                Ok(transformed) => {
                    stream_url = transformed;
                    debug!("N-parameter transformed successfully");
                }
                Err(e) => {
                    debug!("N-parameter transform failed (non-fatal, may be throttled): {e}");
                }
            }
        }
        Err(e) => {
            debug!("Could not fetch player JS URL (non-fatal): {e}");
        }
    }

    // Build filename from title
    let ext = formats::mime_to_ext(&best.mime_type);
    let safe_title = formats::sanitize_filename(&title);
    let filename = format!("{safe_title}.{ext}");

    Ok(YoutubeVideo {
        video_id,
        title,
        stream_url,
        filename,
        filesize: best.content_length,
        mime_type: best.mime_type.clone(),
        quality: best.quality_label.clone(),
    })
}
