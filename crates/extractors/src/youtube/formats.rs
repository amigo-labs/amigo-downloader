//! YouTube format parsing and quality selection.

use serde_json::Value;

/// Intermediate format info for sorting/selection.
#[derive(Debug)]
pub struct FormatInfo {
    pub url: String,
    pub mime_type: String,
    pub quality_label: String,
    pub height: u32,
    pub content_length: Option<u64>,
    pub has_audio: bool,
    pub has_video: bool,
}

/// Parse a single format entry from the innertube response.
pub fn parse_format(fmt: &Value) -> Option<FormatInfo> {
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

    let has_video = mime_type.starts_with("video/");
    let has_audio = mime_type.starts_with("audio/")
        || fmt.get("audioQuality").and_then(|v| v.as_str()).is_some();

    Some(FormatInfo {
        url,
        mime_type,
        quality_label,
        height,
        content_length,
        has_audio,
        has_video,
    })
}

/// Select the best format from a list: prefer combined (audio+video), highest resolution.
pub fn select_best_format(formats: &[FormatInfo]) -> Option<usize> {
    if formats.is_empty() {
        return None;
    }

    // First try combined formats (has both audio and video)
    let mut best_idx = None;
    let mut best_height = 0;

    for (i, f) in formats.iter().enumerate() {
        if f.has_video && f.has_audio && f.height > best_height {
            best_height = f.height;
            best_idx = Some(i);
        }
    }

    // If no combined format, fall back to best video-only
    if best_idx.is_none() {
        for (i, f) in formats.iter().enumerate() {
            if f.has_video && f.height > best_height {
                best_height = f.height;
                best_idx = Some(i);
            }
        }
    }

    best_idx
}

/// Map MIME type to file extension.
pub fn mime_to_ext(mime: &str) -> &str {
    match mime {
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "audio/mp4" => "m4a",
        "audio/webm" => "weba",
        _ => "mp4",
    }
}

/// Sanitize a string for use as a filename.
pub fn sanitize_filename(name: &str) -> String {
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
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("Hello: World?"), "Hello_ World_");
        assert_eq!(sanitize_filename("Normal Title"), "Normal Title");
    }

    #[test]
    fn test_mime_to_ext() {
        assert_eq!(mime_to_ext("video/mp4"), "mp4");
        assert_eq!(mime_to_ext("video/webm"), "webm");
        assert_eq!(mime_to_ext("audio/mp4"), "m4a");
        assert_eq!(mime_to_ext("unknown/type"), "mp4");
    }
}
