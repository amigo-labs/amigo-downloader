//! Enhanced Generic Extractor — automatically detects media on any page.
//!
//! Detection pipeline:
//! 1. Direct media URL detection (extensions, Content-Type)
//! 2. HTML metadata (OpenGraph, Twitter Cards, JSON-LD)
//! 3. Embedded player detection (JW Player, Video.js, Brightcove, etc.)
//! 4. Script data mining (m3u8/mpd/mp4 URLs in JavaScript)
//! 5. HTML5 <video>/<audio>/<source> tags
//! 6. RSS/Atom feed enclosures
//! 7. <iframe> recursion (up to 3 levels deep)

pub mod metadata;
pub mod players;

use regex::Regex;
use scraper::{Html, Selector};
use tracing::{debug, info};
use url::Url;

use crate::error::ExtractorError;
use crate::traits::{ExtractedMedia, Extractor, MediaStream, StreamProtocol};

/// Media file extensions that indicate a direct download.
const MEDIA_EXTENSIONS: &[&str] = &[
    ".mp4", ".mkv", ".avi", ".mov", ".wmv", ".flv", ".webm", ".m4v", ".ts",
    ".mp3", ".flac", ".wav", ".ogg", ".aac", ".m4a", ".opus", ".wma",
];

/// Streaming manifest extensions.
const MANIFEST_EXTENSIONS: &[&str] = &[".m3u8", ".mpd"];

/// Content-Type prefixes that indicate media.
#[allow(dead_code)]
const MEDIA_CONTENT_TYPES: &[&str] = &[
    "video/", "audio/", "application/x-mpegurl", "application/dash+xml",
    "application/vnd.apple.mpegurl",
];

/// Maximum iframe recursion depth.
const MAX_IFRAME_DEPTH: u32 = 3;

pub struct GenericExtractor;

impl GenericExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Detect protocol from URL extension.
    fn protocol_from_url(url: &str) -> Option<StreamProtocol> {
        let path = url.split('?').next().unwrap_or(url).to_lowercase();
        if path.ends_with(".m3u8") {
            Some(StreamProtocol::Hls)
        } else if path.ends_with(".mpd") {
            Some(StreamProtocol::Dash)
        } else if MEDIA_EXTENSIONS.iter().any(|ext| path.ends_with(ext)) {
            Some(StreamProtocol::Http)
        } else {
            None
        }
    }

    /// Detect protocol from Content-Type header.
    fn protocol_from_content_type(ct: &str) -> Option<StreamProtocol> {
        let ct = ct.to_lowercase();
        if ct.contains("mpegurl") || ct.contains("x-mpegurl") {
            Some(StreamProtocol::Hls)
        } else if ct.contains("dash+xml") {
            Some(StreamProtocol::Dash)
        } else if ct.starts_with("video/") || ct.starts_with("audio/") {
            Some(StreamProtocol::Http)
        } else {
            None
        }
    }

    /// Check if a URL points directly to media content.
    fn is_media_url(url: &str) -> bool {
        let path = url.split('?').next().unwrap_or(url).to_lowercase();
        MEDIA_EXTENSIONS.iter().any(|ext| path.ends_with(ext))
            || MANIFEST_EXTENSIONS.iter().any(|ext| path.ends_with(ext))
    }

    /// Build a MediaStream from a discovered URL.
    fn stream_from_url(url: &str, protocol: StreamProtocol) -> MediaStream {
        let has_video = !url.to_lowercase().contains("audio");
        let has_audio = true; // Assume audio by default
        let ext = url
            .split('?')
            .next()
            .unwrap_or(url)
            .rsplit('.')
            .next()
            .unwrap_or("mp4");
        let mime = match ext.to_lowercase().as_str() {
            "mp4" | "m4v" => "video/mp4",
            "webm" => "video/webm",
            "mkv" => "video/x-matroska",
            "mp3" => "audio/mpeg",
            "m4a" => "audio/mp4",
            "flac" => "audio/flac",
            "ogg" | "opus" => "audio/ogg",
            "m3u8" => "application/x-mpegurl",
            "mpd" => "application/dash+xml",
            _ => "video/mp4",
        };

        MediaStream {
            url: url.to_string(),
            protocol,
            quality_label: "auto".to_string(),
            height: 0,
            mime_type: mime.to_string(),
            filesize: None,
            has_audio,
            has_video,
        }
    }

    /// Extract media from an HTML page using all detection methods.
    fn extract_from_html<'a>(
        &'a self,
        client: &'a reqwest::Client,
        page_url: &'a str,
        html: &'a str,
        depth: u32,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<MediaStream>> + Send + 'a>> {
        Box::pin(self.extract_from_html_inner(client, page_url, html, depth))
    }

    async fn extract_from_html_inner(
        &self,
        client: &reqwest::Client,
        page_url: &str,
        html: &str,
        depth: u32,
    ) -> Vec<MediaStream> {
        let mut streams = Vec::new();

        // Phase 2: HTML metadata (OpenGraph, Twitter Cards, JSON-LD)
        streams.extend(metadata::extract_og_video(html, page_url));
        streams.extend(metadata::extract_twitter_player(html, page_url));
        streams.extend(metadata::extract_json_ld(html, page_url));
        streams.extend(metadata::extract_oembed(html, page_url));

        // Phase 3: Embedded player detection
        streams.extend(players::detect_jwplayer(html, page_url));
        streams.extend(players::detect_videojs(html, page_url));
        streams.extend(players::detect_brightcove(html, page_url));
        streams.extend(players::detect_flowplayer(html, page_url));
        streams.extend(players::detect_wistia(html, page_url));

        // Phase 4: Script data mining — find m3u8/mpd/mp4 URLs in scripts
        streams.extend(Self::mine_script_urls(html, page_url));

        // Phase 5: HTML5 <video>/<audio>/<source> tags
        streams.extend(Self::extract_html5_media(html, page_url));

        // Phase 6: RSS/Atom feed enclosures
        streams.extend(Self::extract_feed_enclosures(html));

        // Phase 7: iframe recursion
        if depth < MAX_IFRAME_DEPTH && streams.is_empty() {
            if let Ok(iframe_streams) = self.extract_from_iframes(client, html, page_url, depth).await {
                streams.extend(iframe_streams);
            }
        }

        // Deduplicate by URL
        streams.sort_by(|a, b| a.url.cmp(&b.url));
        streams.dedup_by(|a, b| a.url == b.url);

        streams
    }

    /// Mine JavaScript source for media URLs.
    fn mine_script_urls(html: &str, _base_url: &str) -> Vec<MediaStream> {
        let mut streams = Vec::new();

        // Find m3u8 URLs
        let m3u8_re = Regex::new(r#"["\']?(https?://[^"'\s]+\.m3u8(?:\?[^"'\s]*)?)["\']?"#).unwrap();
        for cap in m3u8_re.captures_iter(html) {
            if let Some(url) = cap.get(1) {
                debug!("Found m3u8 in script: {}", url.as_str());
                streams.push(Self::stream_from_url(url.as_str(), StreamProtocol::Hls));
            }
        }

        // Find mpd URLs
        let mpd_re = Regex::new(r#"["\']?(https?://[^"'\s]+\.mpd(?:\?[^"'\s]*)?)["\']?"#).unwrap();
        for cap in mpd_re.captures_iter(html) {
            if let Some(url) = cap.get(1) {
                debug!("Found mpd in script: {}", url.as_str());
                streams.push(Self::stream_from_url(url.as_str(), StreamProtocol::Dash));
            }
        }

        // Find direct mp4/webm URLs in JavaScript (but not in HTML href/src which we handle elsewhere)
        let mp4_re = Regex::new(r#"["\']?(https?://[^"'\s]+\.(?:mp4|webm)(?:\?[^"'\s]*)?)["\']?"#).unwrap();
        for cap in mp4_re.captures_iter(html) {
            if let Some(url) = cap.get(1) {
                let url_str = url.as_str();
                // Skip obvious non-video mp4 URLs (thumbnails, etc.)
                if !url_str.contains("thumb") && !url_str.contains("poster") && !url_str.contains("preview") {
                    debug!("Found media URL in script: {}", url_str);
                    streams.push(Self::stream_from_url(url_str, StreamProtocol::Http));
                }
            }
        }

        streams
    }

    /// Extract media from HTML5 <video> and <audio> tags.
    fn extract_html5_media(html: &str, base_url: &str) -> Vec<MediaStream> {
        let mut streams = Vec::new();
        let document = Html::parse_document(html);

        // <video src="..."> and <audio src="...">
        let media_sel = Selector::parse("video[src], audio[src]").unwrap();
        for elem in document.select(&media_sel) {
            if let Some(src) = elem.value().attr("src") {
                if let Some(url) = resolve_url(base_url, src) {
                    let proto = Self::protocol_from_url(&url).unwrap_or(StreamProtocol::Http);
                    streams.push(Self::stream_from_url(&url, proto));
                }
            }
        }

        // <source> tags inside <video> or <audio>
        let source_sel = Selector::parse("video source, audio source").unwrap();
        for elem in document.select(&source_sel) {
            if let Some(src) = elem.value().attr("src") {
                if let Some(url) = resolve_url(base_url, src) {
                    let proto = Self::protocol_from_url(&url).unwrap_or(StreamProtocol::Http);
                    let mime = elem.value().attr("type").unwrap_or("").to_string();
                    let mut stream = Self::stream_from_url(&url, proto);
                    if !mime.is_empty() {
                        stream.mime_type = mime;
                    }
                    streams.push(stream);
                }
            }
        }

        streams
    }

    /// Extract enclosures from RSS/Atom feeds.
    fn extract_feed_enclosures(html: &str) -> Vec<MediaStream> {
        let mut streams = Vec::new();

        // RSS <enclosure url="..." type="..."/>
        let enclosure_re = Regex::new(
            r#"<enclosure[^>]+url=["']([^"']+)["'][^>]*(?:type=["']([^"']+)["'])?"#
        ).unwrap();
        for cap in enclosure_re.captures_iter(html) {
            if let Some(url) = cap.get(1) {
                let url_str = url.as_str();
                let ct = cap.get(2).map(|m| m.as_str()).unwrap_or("");
                if ct.starts_with("video/") || ct.starts_with("audio/") || Self::is_media_url(url_str) {
                    let proto = Self::protocol_from_url(url_str).unwrap_or(StreamProtocol::Http);
                    streams.push(Self::stream_from_url(url_str, proto));
                }
            }
        }

        streams
    }

    /// Follow iframes and extract media from embedded pages.
    async fn extract_from_iframes(
        &self,
        client: &reqwest::Client,
        html: &str,
        base_url: &str,
        depth: u32,
    ) -> Result<Vec<MediaStream>, ExtractorError> {
        // Collect iframe URLs first to avoid holding non-Send scraper types across await
        let iframe_urls: Vec<String> = {
            let document = Html::parse_document(html);
            let iframe_sel = Selector::parse("iframe[src]").unwrap();
            document
                .select(&iframe_sel)
                .filter_map(|elem| {
                    let src = elem.value().attr("src")?;
                    if src.is_empty() || src.starts_with("about:") || src.starts_with("javascript:") {
                        return None;
                    }
                    resolve_url(base_url, src)
                })
                .collect()
        };

        let mut streams = Vec::new();
        for iframe_url in iframe_urls {
            debug!("Following iframe (depth {}): {}", depth + 1, iframe_url);

            match client.get(&iframe_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(body) = resp.text().await {
                        let iframe_streams = self
                            .extract_from_html(client, &iframe_url, &body, depth + 1)
                            .await;
                        streams.extend(iframe_streams);
                    }
                }
                Ok(resp) => {
                    debug!("iframe returned {}: {}", resp.status(), iframe_url);
                }
                Err(e) => {
                    debug!("Failed to fetch iframe {}: {}", iframe_url, e);
                }
            }
        }

        Ok(streams)
    }
}

#[async_trait::async_trait]
impl Extractor for GenericExtractor {
    fn name(&self) -> &str {
        "Generic"
    }

    fn supports_url(&self, _url: &str) -> bool {
        // Generic extractor supports any HTTP/HTTPS URL
        true
    }

    async fn extract(
        &self,
        client: &reqwest::Client,
        url: &str,
    ) -> Result<ExtractedMedia, ExtractorError> {
        info!("Generic extractor running for: {}", url);

        // Phase 1: Check if URL points directly to media
        if let Some(proto) = Self::protocol_from_url(url) {
            return Ok(ExtractedMedia {
                title: url
                    .rsplit('/')
                    .next()
                    .unwrap_or("Download")
                    .split('?')
                    .next()
                    .unwrap_or("Download")
                    .to_string(),
                streams: vec![Self::stream_from_url(url, proto)],
            });
        }

        // Phase 1b: HEAD request to check Content-Type
        if let Ok(resp) = client.head(url).send().await {
            if let Some(ct) = resp.headers().get("content-type") {
                let ct_str = ct.to_str().unwrap_or("");
                if let Some(proto) = Self::protocol_from_content_type(ct_str) {
                    return Ok(ExtractedMedia {
                        title: url
                            .rsplit('/')
                            .next()
                            .unwrap_or("Download")
                            .to_string(),
                        streams: vec![Self::stream_from_url(url, proto)],
                    });
                }
            }
        }

        // Fetch the page
        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| ExtractorError::Other(e.to_string()))?;
        let body = resp
            .text()
            .await
            .map_err(|e| ExtractorError::Other(e.to_string()))?;

        // Extract title
        let title = extract_page_title(&body).unwrap_or_else(|| "Download".to_string());

        // Run the full extraction pipeline
        let streams = self.extract_from_html(client, url, &body, 0).await;

        if streams.is_empty() {
            return Err(ExtractorError::NoStreams(format!(
                "No media streams found on page: {url}"
            )));
        }

        info!(
            "Generic extractor found {} stream(s) for: {}",
            streams.len(),
            url
        );

        Ok(ExtractedMedia { title, streams })
    }
}

/// Resolve a relative URL against a base URL.
fn resolve_url(base: &str, relative: &str) -> Option<String> {
    if relative.starts_with("http://") || relative.starts_with("https://") {
        return Some(relative.to_string());
    }
    if relative.starts_with("//") {
        let scheme = if base.starts_with("https") {
            "https:"
        } else {
            "http:"
        };
        return Some(format!("{}{}", scheme, relative));
    }
    Url::parse(base)
        .ok()
        .and_then(|b| b.join(relative).ok())
        .map(|u| u.to_string())
}

/// Extract <title> from HTML.
fn extract_page_title(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let sel = Selector::parse("title").ok()?;
    document
        .select(&sel)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_detection() {
        assert_eq!(
            GenericExtractor::protocol_from_url("https://example.com/video.m3u8"),
            Some(StreamProtocol::Hls)
        );
        assert_eq!(
            GenericExtractor::protocol_from_url("https://example.com/video.mpd?token=abc"),
            Some(StreamProtocol::Dash)
        );
        assert_eq!(
            GenericExtractor::protocol_from_url("https://example.com/video.mp4"),
            Some(StreamProtocol::Http)
        );
        assert_eq!(
            GenericExtractor::protocol_from_url("https://example.com/page"),
            None
        );
    }

    #[test]
    fn test_html5_video_extraction() {
        let html = r#"
            <html><body>
                <video src="https://cdn.example.com/video.mp4"></video>
            </body></html>
        "#;
        let streams = GenericExtractor::extract_html5_media(html, "https://example.com");
        assert_eq!(streams.len(), 1);
        assert_eq!(streams[0].url, "https://cdn.example.com/video.mp4");
        assert_eq!(streams[0].protocol, StreamProtocol::Http);
    }

    #[test]
    fn test_html5_source_tags() {
        let html = r#"
            <html><body>
                <video>
                    <source src="/video/720p.mp4" type="video/mp4">
                    <source src="/video/stream.m3u8" type="application/x-mpegurl">
                </video>
            </body></html>
        "#;
        let streams =
            GenericExtractor::extract_html5_media(html, "https://example.com");
        assert_eq!(streams.len(), 2);
        assert!(streams[0].url.ends_with("/video/720p.mp4"));
        assert!(streams[1].url.ends_with("/video/stream.m3u8"));
        assert_eq!(streams[1].protocol, StreamProtocol::Hls);
    }

    #[test]
    fn test_script_mining() {
        let html = r#"
            <html><body>
            <script>
                var config = {
                    source: "https://cdn.example.com/live/stream.m3u8?token=abc123",
                    fallback: "https://cdn.example.com/vod/video.mp4"
                };
            </script>
            </body></html>
        "#;
        let streams = GenericExtractor::mine_script_urls(html, "https://example.com");
        assert_eq!(streams.len(), 2);
        assert!(streams.iter().any(|s| s.protocol == StreamProtocol::Hls));
        assert!(streams.iter().any(|s| s.protocol == StreamProtocol::Http));
    }

    #[test]
    fn test_rss_enclosure() {
        let html = r#"
            <rss><channel><item>
                <enclosure url="https://podcast.example.com/ep1.mp3" type="audio/mpeg" length="12345678"/>
            </item></channel></rss>
        "#;
        let streams = GenericExtractor::extract_feed_enclosures(html);
        assert_eq!(streams.len(), 1);
        assert_eq!(streams[0].url, "https://podcast.example.com/ep1.mp3");
    }

    #[test]
    fn test_resolve_url() {
        assert_eq!(
            resolve_url("https://example.com/page", "/video.mp4"),
            Some("https://example.com/video.mp4".to_string())
        );
        assert_eq!(
            resolve_url("https://example.com/page", "//cdn.example.com/v.mp4"),
            Some("https://cdn.example.com/v.mp4".to_string())
        );
        assert_eq!(
            resolve_url("https://example.com/page", "https://other.com/v.mp4"),
            Some("https://other.com/v.mp4".to_string())
        );
    }

    #[test]
    fn test_page_title() {
        let html = "<html><head><title>My Video Page</title></head><body></body></html>";
        assert_eq!(
            extract_page_title(html),
            Some("My Video Page".to_string())
        );
    }
}
