//! HTML metadata extraction: OpenGraph, Twitter Cards, JSON-LD, OEmbed.

use regex::Regex;
use scraper::{Html, Selector};
use tracing::debug;

use crate::traits::{MediaStream, StreamProtocol};

use super::{resolve_url, GenericExtractor};

/// Extract video URLs from OpenGraph meta tags.
///
/// Looks for: og:video, og:video:url, og:video:secure_url
pub fn extract_og_video(html: &str, base_url: &str) -> Vec<MediaStream> {
    let mut streams = Vec::new();
    let document = Html::parse_document(html);

    let og_tags = ["og:video", "og:video:url", "og:video:secure_url"];

    for tag in og_tags {
        let selector_str = format!(
            r#"meta[property="{tag}"], meta[name="{tag}"]"#
        );
        if let Ok(sel) = Selector::parse(&selector_str) {
            for elem in document.select(&sel) {
                if let Some(content) = elem.value().attr("content") {
                    if content.is_empty() {
                        continue;
                    }
                    if let Some(url) = resolve_url(base_url, content) {
                        // Skip flash/embed URLs
                        if url.contains(".swf") || url.contains("embed") && !GenericExtractor::is_media_url(&url) {
                            continue;
                        }
                        let proto = GenericExtractor::protocol_from_url(&url)
                            .unwrap_or(StreamProtocol::Http);
                        debug!("Found OG video: {}", url);
                        streams.push(GenericExtractor::stream_from_url(&url, proto));
                    }
                }
            }
        }
    }

    // Also check og:video:type for protocol hints
    if let Ok(sel) = Selector::parse(r#"meta[property="og:video:type"]"#) {
        if let Some(elem) = document.select(&sel).next() {
            if let Some(content) = elem.value().attr("content") {
                if let Some(proto) = GenericExtractor::protocol_from_content_type(content) {
                    for stream in &mut streams {
                        if stream.protocol == StreamProtocol::Http {
                            stream.protocol = proto.clone();
                        }
                    }
                }
            }
        }
    }

    streams
}

/// Extract video URLs from Twitter Card meta tags.
///
/// Looks for: twitter:player:stream, twitter:player
pub fn extract_twitter_player(html: &str, base_url: &str) -> Vec<MediaStream> {
    let mut streams = Vec::new();
    let document = Html::parse_document(html);

    let tags = ["twitter:player:stream", "twitter:player"];

    for tag in tags {
        let selector_str = format!(r#"meta[name="{tag}"], meta[property="{tag}"]"#);
        if let Ok(sel) = Selector::parse(&selector_str) {
            for elem in document.select(&sel) {
                if let Some(content) = elem.value().attr("content") {
                    if content.is_empty() {
                        continue;
                    }
                    if let Some(url) = resolve_url(base_url, content) {
                        if GenericExtractor::is_media_url(&url) {
                            let proto = GenericExtractor::protocol_from_url(&url)
                                .unwrap_or(StreamProtocol::Http);
                            debug!("Found Twitter player stream: {}", url);
                            streams.push(GenericExtractor::stream_from_url(&url, proto));
                        }
                    }
                }
            }
        }
    }

    streams
}

/// Extract video URLs from JSON-LD VideoObject schema.
pub fn extract_json_ld(html: &str, base_url: &str) -> Vec<MediaStream> {
    let mut streams = Vec::new();
    let document = Html::parse_document(html);

    if let Ok(sel) = Selector::parse(r#"script[type="application/ld+json"]"#) {
        for elem in document.select(&sel) {
            let text: String = elem.text().collect();
            let text = text.trim();
            if text.is_empty() {
                continue;
            }

            if let Ok(json) = serde_json::from_str::<serde_json::Value>(text) {
                extract_video_object_urls(&json, base_url, &mut streams);

                // Also check @graph array
                if let Some(graph) = json.get("@graph").and_then(|g| g.as_array()) {
                    for item in graph {
                        extract_video_object_urls(item, base_url, &mut streams);
                    }
                }
            }
        }
    }

    streams
}

/// Helper: extract contentUrl/embedUrl from a JSON-LD VideoObject.
fn extract_video_object_urls(
    json: &serde_json::Value,
    base_url: &str,
    streams: &mut Vec<MediaStream>,
) {
    let type_val = json.get("@type").and_then(|t| t.as_str()).unwrap_or("");
    if type_val != "VideoObject" && type_val != "AudioObject" {
        return;
    }

    for key in &["contentUrl", "embedUrl"] {
        if let Some(url_str) = json.get(key).and_then(|v| v.as_str()) {
            if let Some(url) = resolve_url(base_url, url_str) {
                if GenericExtractor::is_media_url(&url) || url.contains(".m3u8") || url.contains(".mpd") {
                    let proto = GenericExtractor::protocol_from_url(&url)
                        .unwrap_or(StreamProtocol::Http);
                    debug!("Found JSON-LD {}: {}", key, url);
                    streams.push(GenericExtractor::stream_from_url(&url, proto));
                }
            }
        }
    }
}

/// Detect OEmbed discovery links and extract video URLs.
///
/// Looks for <link rel="alternate" type="application/json+oembed" href="...">
pub fn extract_oembed(html: &str, _base_url: &str) -> Vec<MediaStream> {
    // OEmbed requires an additional HTTP request which we avoid in the extractor.
    // We just detect the presence for now — the coordinator can follow up.
    // For now, return empty — the actual OEmbed fetch would happen at a higher level.
    let _ = html;
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_og_video() {
        let html = r#"
            <html><head>
                <meta property="og:video" content="https://cdn.example.com/video.mp4">
                <meta property="og:video:type" content="video/mp4">
            </head><body></body></html>
        "#;
        let streams = extract_og_video(html, "https://example.com");
        assert_eq!(streams.len(), 1);
        assert_eq!(streams[0].url, "https://cdn.example.com/video.mp4");
    }

    #[test]
    fn test_og_hls() {
        let html = r#"
            <html><head>
                <meta property="og:video:url" content="https://cdn.example.com/live.m3u8">
            </head><body></body></html>
        "#;
        let streams = extract_og_video(html, "https://example.com");
        assert_eq!(streams.len(), 1);
        assert_eq!(streams[0].protocol, StreamProtocol::Hls);
    }

    #[test]
    fn test_json_ld_video_object() {
        let html = r#"
            <html><head>
                <script type="application/ld+json">
                {
                    "@type": "VideoObject",
                    "name": "Test Video",
                    "contentUrl": "https://cdn.example.com/video.mp4",
                    "embedUrl": "https://cdn.example.com/embed/123"
                }
                </script>
            </head><body></body></html>
        "#;
        let streams = extract_json_ld(html, "https://example.com");
        assert_eq!(streams.len(), 1); // Only contentUrl matches (has media extension)
        assert_eq!(streams[0].url, "https://cdn.example.com/video.mp4");
    }

    #[test]
    fn test_twitter_player_stream() {
        let html = r#"
            <html><head>
                <meta name="twitter:player:stream" content="https://cdn.example.com/video.mp4">
            </head><body></body></html>
        "#;
        let streams = extract_twitter_player(html, "https://example.com");
        assert_eq!(streams.len(), 1);
        assert_eq!(streams[0].url, "https://cdn.example.com/video.mp4");
    }
}
