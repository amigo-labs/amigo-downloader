//! Embedded video player detection.
//!
//! Detects and extracts media URLs from common embedded players:
//! - JW Player
//! - Video.js
//! - Brightcove
//! - Flowplayer
//! - Wistia

use regex::Regex;
use tracing::debug;

use crate::traits::{MediaStream, StreamProtocol};

use super::{GenericExtractor, resolve_url};

/// Detect JW Player embeds and extract stream URLs.
///
/// Patterns detected:
/// - `jwplayer("...").setup({ file: "..." })`
/// - `jwDefaults = { file: "..." }`
/// - `playerInstance.setup({ sources: [...] })`
/// - `jwConfig = { playlist: [{ file: "..." }] }`
pub fn detect_jwplayer(html: &str, base_url: &str) -> Vec<MediaStream> {
    let mut streams = Vec::new();

    // Pattern 1: jwplayer().setup() with file
    let setup_re = Regex::new(
        r#"jwplayer\s*\([^)]*\)\s*\.\s*setup\s*\(\s*\{[^}]*?["']?file["']?\s*:\s*["']([^"']+)["']"#,
    )
    .unwrap();
    for cap in setup_re.captures_iter(html) {
        if let Some(url) = cap.get(1) {
            add_media_stream(&mut streams, url.as_str(), base_url, "JW Player setup.file");
        }
    }

    // Pattern 2: Generic file property near jwplayer references
    if html.contains("jwplayer") || html.contains("jwDefaults") || html.contains("jw-video") {
        let file_re = Regex::new(r#"["']?file["']?\s*:\s*["'](https?://[^"']+)["']"#).unwrap();
        for cap in file_re.captures_iter(html) {
            if let Some(url) = cap.get(1) {
                let url_str = url.as_str();
                if GenericExtractor::is_media_url(url_str)
                    || url_str.contains(".m3u8")
                    || url_str.contains(".mpd")
                {
                    add_media_stream(&mut streams, url_str, base_url, "JW Player file");
                }
            }
        }

        // Pattern 3: sources array
        let sources_re = Regex::new(r#"["']?sources["']?\s*:\s*\[([^\]]+)\]"#).unwrap();
        let src_file_re = Regex::new(r#"["']?file["']?\s*:\s*["'](https?://[^"']+)["']"#).unwrap();
        for cap in sources_re.captures_iter(html) {
            if let Some(inner) = cap.get(1) {
                for src_cap in src_file_re.captures_iter(inner.as_str()) {
                    if let Some(url) = src_cap.get(1) {
                        add_media_stream(&mut streams, url.as_str(), base_url, "JW Player sources");
                    }
                }
            }
        }
    }

    streams
}

/// Detect Video.js player embeds.
///
/// Patterns:
/// - `<video class="video-js" data-setup='{"sources":[...]}'>`
/// - `videojs("player").src({...})`
pub fn detect_videojs(html: &str, base_url: &str) -> Vec<MediaStream> {
    let mut streams = Vec::new();

    // data-setup attribute with sources
    let data_setup_re = Regex::new(r#"data-setup\s*=\s*'([^']+)'"#).unwrap();
    for cap in data_setup_re.captures_iter(html) {
        if let Some(json) = cap
            .get(1)
            .and_then(|json_str| serde_json::from_str::<serde_json::Value>(json_str.as_str()).ok())
            && let Some(sources) = json.get("sources").and_then(|s| s.as_array())
        {
            for source in sources {
                if let Some(src) = source.get("src").and_then(|s| s.as_str()) {
                    add_media_stream(&mut streams, src, base_url, "Video.js data-setup");
                }
            }
        }
    }

    // videojs().src() calls
    if html.contains("videojs") || html.contains("video-js") {
        let src_re =
            Regex::new(r#"\.src\s*\(\s*\{[^}]*["']?src["']?\s*:\s*["'](https?://[^"']+)["']"#)
                .unwrap();
        for cap in src_re.captures_iter(html) {
            if let Some(url) = cap.get(1) {
                add_media_stream(&mut streams, url.as_str(), base_url, "Video.js src()");
            }
        }
    }

    streams
}

/// Detect Brightcove player embeds.
///
/// Patterns:
/// - `<video-js data-video-id="..." data-account="...">`
/// - Brightcove player API URLs
pub fn detect_brightcove(html: &str, base_url: &str) -> Vec<MediaStream> {
    let mut streams = Vec::new();

    // Brightcove often includes source URLs in data attributes or script configs
    if html.contains("brightcove") || html.contains("bc-player") || html.contains("data-video-id") {
        // Look for video sources in Brightcove config
        let source_re =
            Regex::new(r#"["']?src["']?\s*:\s*["'](https?://[^"']+\.(?:m3u8|mpd|mp4)[^"']*)["']"#)
                .unwrap();
        for cap in source_re.captures_iter(html) {
            if let Some(url) = cap.get(1) {
                add_media_stream(&mut streams, url.as_str(), base_url, "Brightcove");
            }
        }
    }

    streams
}

/// Detect Flowplayer embeds.
///
/// Patterns:
/// - `flowplayer("#player", { clip: { sources: [...] } })`
pub fn detect_flowplayer(html: &str, base_url: &str) -> Vec<MediaStream> {
    let mut streams = Vec::new();

    if html.contains("flowplayer") {
        // Extract clip sources
        let clip_re = Regex::new(
            r#"["']?src["']?\s*:\s*["'](https?://[^"']+\.(?:m3u8|mpd|mp4|webm)[^"']*)["']"#,
        )
        .unwrap();
        for cap in clip_re.captures_iter(html) {
            if let Some(url) = cap.get(1) {
                add_media_stream(&mut streams, url.as_str(), base_url, "Flowplayer");
            }
        }
    }

    streams
}

/// Detect Wistia embeds.
///
/// Patterns:
/// - `<div class="wistia_embed" data-video-id="...">`
/// - Wistia JSON config with media assets
pub fn detect_wistia(html: &str, base_url: &str) -> Vec<MediaStream> {
    let mut streams = Vec::new();

    if html.contains("wistia") {
        // Wistia often embeds asset URLs in JSON config
        let asset_re = Regex::new(
            r#"["']?url["']?\s*:\s*["'](https?://[^"']*wistia[^"']*\.(?:m3u8|mp4|bin)[^"']*)["']"#,
        )
        .unwrap();
        for cap in asset_re.captures_iter(html) {
            if let Some(url) = cap.get(1) {
                add_media_stream(&mut streams, url.as_str(), base_url, "Wistia");
            }
        }
    }

    streams
}

/// Helper: add a media stream if the URL looks valid.
fn add_media_stream(streams: &mut Vec<MediaStream>, url: &str, base_url: &str, source: &str) {
    // Skip data: URLs and empty strings
    if url.is_empty() || url.starts_with("data:") {
        return;
    }

    let resolved = if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else if let Some(resolved) = resolve_url(base_url, url) {
        resolved
    } else {
        return;
    };

    // Avoid duplicates
    if streams.iter().any(|s| s.url == resolved) {
        return;
    }

    let proto = GenericExtractor::protocol_from_url(&resolved).unwrap_or(StreamProtocol::Http);
    debug!(
        "Found media via {}: {} ({})",
        source,
        resolved,
        format!("{:?}", proto)
    );
    streams.push(GenericExtractor::stream_from_url(&resolved, proto));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwplayer_setup() {
        let html = r#"
            <script>
                jwplayer("player").setup({
                    file: "https://cdn.example.com/video.mp4",
                    width: 640,
                    height: 360
                });
            </script>
        "#;
        let streams = detect_jwplayer(html, "https://example.com");
        assert_eq!(streams.len(), 1);
        assert_eq!(streams[0].url, "https://cdn.example.com/video.mp4");
    }

    #[test]
    fn test_jwplayer_hls() {
        let html = r#"
            <script>
                jwplayer("player").setup({
                    file: "https://cdn.example.com/live/stream.m3u8",
                    type: "hls"
                });
            </script>
        "#;
        let streams = detect_jwplayer(html, "https://example.com");
        assert_eq!(streams.len(), 1);
        assert_eq!(streams[0].protocol, StreamProtocol::Hls);
    }

    #[test]
    fn test_jwplayer_sources_array() {
        let html = r#"
            <script>
                jwplayer("player").setup({
                    sources: [
                        { file: "https://cdn.example.com/720p.mp4", label: "720p" },
                        { file: "https://cdn.example.com/480p.mp4", label: "480p" }
                    ]
                });
            </script>
        "#;
        let streams = detect_jwplayer(html, "https://example.com");
        assert_eq!(streams.len(), 2);
    }

    #[test]
    fn test_videojs_data_setup() {
        let html = r#"
            <video class="video-js" data-setup='{"sources":[{"src":"https://cdn.example.com/video.mp4","type":"video/mp4"}]}'>
            </video>
        "#;
        let streams = detect_videojs(html, "https://example.com");
        assert_eq!(streams.len(), 1);
        assert_eq!(streams[0].url, "https://cdn.example.com/video.mp4");
    }

    #[test]
    fn test_no_detection_on_unrelated_page() {
        let html = r#"
            <html><head><title>Blog Post</title></head>
            <body><p>Just a regular page with no video.</p></body></html>
        "#;
        assert!(detect_jwplayer(html, "https://example.com").is_empty());
        assert!(detect_videojs(html, "https://example.com").is_empty());
        assert!(detect_brightcove(html, "https://example.com").is_empty());
        assert!(detect_flowplayer(html, "https://example.com").is_empty());
        assert!(detect_wistia(html, "https://example.com").is_empty());
    }
}
