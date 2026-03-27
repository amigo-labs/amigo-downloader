//! YouTube video stream extraction.
//!
//! Extracts direct download URLs from YouTube videos using the innertube API.
//! Supports watch URLs, short URLs, embed URLs, and Shorts.

use std::collections::HashMap;

use serde_json::Value;
use tracing::{debug, info, warn};

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

/// Extract video ID from various YouTube URL formats.
pub fn extract_video_id(url: &str) -> Result<String, crate::Error> {
    // youtube.com/watch?v=ID
    if let Some(pos) = url.find("v=") {
        let rest = &url[pos + 2..];
        let id: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_').collect();
        if id.len() == 11 {
            return Ok(id);
        }
    }

    // youtu.be/ID
    if url.contains("youtu.be/") {
        if let Some(pos) = url.find("youtu.be/") {
            let rest = &url[pos + 9..];
            let id: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_').collect();
            if id.len() == 11 {
                return Ok(id);
            }
        }
    }

    // youtube.com/embed/ID or youtube.com/shorts/ID
    for prefix in &["/embed/", "/shorts/", "/v/"] {
        if let Some(pos) = url.find(prefix) {
            let rest = &url[pos + prefix.len()..];
            let id: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_').collect();
            if id.len() == 11 {
                return Ok(id);
            }
        }
    }

    Err(crate::Error::Other(format!("Could not extract video ID from: {url}")))
}

/// Check whether a URL is a YouTube URL.
pub fn is_youtube_url(url: &str) -> bool {
    url.contains("youtube.com/") || url.contains("youtu.be/")
}

/// Fetch video info and select the best stream.
pub async fn resolve(client: &reqwest::Client, url: &str) -> Result<YoutubeVideo, crate::Error> {
    let video_id = extract_video_id(url)?;
    info!("Resolving YouTube video: {video_id}");

    // Try innertube API with Android client (often returns direct URLs)
    let player = fetch_innertube(client, &video_id).await?;

    let title = player
        .pointer("/videoDetails/title")
        .and_then(|v| v.as_str())
        .unwrap_or("video")
        .to_string();

    debug!("Video title: {title}");

    // Collect all formats with direct URLs
    let mut formats = Vec::new();

    // Combined formats (audio + video) — preferred for single-file download
    if let Some(fmts) = player.pointer("/streamingData/formats").and_then(|v| v.as_array()) {
        for fmt in fmts {
            if let Some(info) = parse_format(fmt) {
                formats.push(info);
            }
        }
    }

    // Adaptive formats as fallback (may be video-only or audio-only)
    if formats.is_empty() {
        if let Some(fmts) = player.pointer("/streamingData/adaptiveFormats").and_then(|v| v.as_array()) {
            for fmt in fmts {
                if let Some(info) = parse_format(fmt) {
                    // Prefer formats that have both audio and video
                    if info.mime_type.starts_with("video/") {
                        formats.push(info);
                    }
                }
            }
        }
    }

    if formats.is_empty() {
        // Check for playability errors
        let reason = player
            .pointer("/playabilityStatus/reason")
            .and_then(|v| v.as_str())
            .unwrap_or("No downloadable streams found");
        return Err(crate::Error::Other(format!("YouTube: {reason}")));
    }

    // Sort by quality (height descending), prefer combined formats
    formats.sort_by(|a, b| b.height.cmp(&a.height));

    let best = &formats[0];
    info!("Selected format: {} ({})", best.quality_label, best.mime_type);

    // Build filename from title
    let ext = mime_to_ext(&best.mime_type);
    let safe_title = sanitize_filename(&title);
    let filename = format!("{safe_title}.{ext}");

    Ok(YoutubeVideo {
        video_id,
        title,
        stream_url: best.url.clone(),
        filename,
        filesize: best.content_length,
        mime_type: best.mime_type.clone(),
        quality: best.quality_label.clone(),
    })
}

/// Intermediate format info for sorting/selection.
#[derive(Debug)]
struct FormatInfo {
    url: String,
    mime_type: String,
    quality_label: String,
    height: u32,
    content_length: Option<u64>,
}

fn parse_format(fmt: &Value) -> Option<FormatInfo> {
    // Must have a direct URL (no signatureCipher)
    let url = fmt.get("url")?.as_str()?.to_string();

    let mime_full = fmt.get("mimeType")?.as_str().unwrap_or("video/mp4");
    // Strip codec info: "video/mp4; codecs=\"avc1.42001E\"" → "video/mp4"
    let mime_type = mime_full.split(';').next().unwrap_or(mime_full).to_string();

    let quality_label = fmt
        .get("qualityLabel")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let height = fmt.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

    let content_length = fmt
        .get("contentLength")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<u64>().ok());

    Some(FormatInfo {
        url,
        mime_type,
        quality_label,
        height,
        content_length,
    })
}

/// Call the YouTube innertube player API.
async fn fetch_innertube(client: &reqwest::Client, video_id: &str) -> Result<Value, crate::Error> {
    // Try Android client first — often returns direct URLs without signature cipher
    let clients: &[(&str, &str, Value)] = &[
        ("ANDROID", "19.09.37", serde_json::json!({
            "videoId": video_id,
            "context": {
                "client": {
                    "clientName": "ANDROID",
                    "clientVersion": "19.09.37",
                    "androidSdkVersion": 30,
                    "hl": "en",
                    "gl": "US",
                    "userAgent": "com.google.android.youtube/19.09.37 (Linux; U; Android 11) gzip"
                }
            }
        })),
        ("WEB", "2.20240313", serde_json::json!({
            "videoId": video_id,
            "context": {
                "client": {
                    "clientName": "WEB",
                    "clientVersion": "2.20240313.01.00",
                    "hl": "en",
                    "gl": "US"
                }
            }
        })),
    ];

    for (name, _version, body) in clients {
        debug!("Trying innertube client: {name}");

        let resp = client
            .post("https://www.youtube.com/youtubei/v1/player?prettyPrint=false")
            .header("Content-Type", "application/json")
            .header("X-YouTube-Client-Name", "3")
            .header("X-YouTube-Client-Version", "19.09.37")
            .body(body.to_string())
            .send()
            .await?;

        if !resp.status().is_success() {
            warn!("Innertube {name} returned status {}", resp.status());
            continue;
        }

        let data: Value = resp.json().await?;

        // Check if we got usable streaming data
        let status = data
            .pointer("/playabilityStatus/status")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if status != "OK" {
            let reason = data
                .pointer("/playabilityStatus/reason")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            warn!("Innertube {name}: status={status}, reason={reason}");
            continue;
        }

        // Check if formats have direct URLs
        let has_direct_urls = data
            .pointer("/streamingData/formats")
            .and_then(|v| v.as_array())
            .is_some_and(|fmts| fmts.iter().any(|f| f.get("url").is_some()));

        let has_adaptive_urls = data
            .pointer("/streamingData/adaptiveFormats")
            .and_then(|v| v.as_array())
            .is_some_and(|fmts| fmts.iter().any(|f| f.get("url").is_some()));

        if has_direct_urls || has_adaptive_urls {
            info!("Got streaming data from innertube client {name}");
            return Ok(data);
        }

        debug!("Innertube {name}: no direct URLs in response");
    }

    // Fallback: try extracting from watch page HTML
    fetch_from_watch_page(client, video_id).await
}

/// Fallback: extract player response from the watch page HTML.
async fn fetch_from_watch_page(client: &reqwest::Client, video_id: &str) -> Result<Value, crate::Error> {
    debug!("Falling back to watch page extraction");

    let url = format!("https://www.youtube.com/watch?v={video_id}&has_verified=1");
    let resp = client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await?;

    let html = resp.text().await?;

    // Extract ytInitialPlayerResponse
    let marker = "var ytInitialPlayerResponse = ";
    let start = html.find(marker)
        .ok_or_else(|| crate::Error::Other("Could not find ytInitialPlayerResponse in page".into()))?;

    let json_start = start + marker.len();
    let rest = &html[json_start..];

    // Find the end of the JSON object by counting braces
    let json_str = extract_json_object(rest)
        .ok_or_else(|| crate::Error::Other("Could not parse ytInitialPlayerResponse JSON".into()))?;

    let data: Value = serde_json::from_str(json_str)?;
    Ok(data)
}

/// Extract a JSON object from a string by matching braces.
fn extract_json_object(s: &str) -> Option<&str> {
    if !s.starts_with('{') {
        return None;
    }

    let mut depth = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, ch) in s.char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

fn mime_to_ext(mime: &str) -> &str {
    match mime {
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "audio/mp4" => "m4a",
        "audio/webm" => "weba",
        _ => "mp4",
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_video_id_watch() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=1mzl2Oo8Ncw").unwrap(),
            "1mzl2Oo8Ncw"
        );
    }

    #[test]
    fn test_extract_video_id_short_url() {
        assert_eq!(
            extract_video_id("https://youtu.be/1mzl2Oo8Ncw").unwrap(),
            "1mzl2Oo8Ncw"
        );
    }

    #[test]
    fn test_extract_video_id_embed() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/embed/1mzl2Oo8Ncw").unwrap(),
            "1mzl2Oo8Ncw"
        );
    }

    #[test]
    fn test_extract_video_id_shorts() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/shorts/1mzl2Oo8Ncw").unwrap(),
            "1mzl2Oo8Ncw"
        );
    }

    #[test]
    fn test_extract_video_id_with_params() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=1mzl2Oo8Ncw&t=120").unwrap(),
            "1mzl2Oo8Ncw"
        );
    }

    #[test]
    fn test_is_youtube_url() {
        assert!(is_youtube_url("https://www.youtube.com/watch?v=abc"));
        assert!(is_youtube_url("https://youtu.be/abc"));
        assert!(!is_youtube_url("https://example.com/file.zip"));
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("Hello: World?"), "Hello_ World_");
        assert_eq!(sanitize_filename("Normal Title"), "Normal Title");
    }

    #[test]
    fn test_extract_json_object() {
        assert_eq!(extract_json_object(r#"{"a":1};"#), Some(r#"{"a":1}"#));
        assert_eq!(
            extract_json_object(r#"{"a":{"b":2}}rest"#),
            Some(r#"{"a":{"b":2}}"#)
        );
        assert_eq!(
            extract_json_object(r#"{"s":"he said \"hi\""}x"#),
            Some(r#"{"s":"he said \"hi\""}"#)
        );
    }
}
