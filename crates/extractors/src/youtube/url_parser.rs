//! YouTube URL parsing — extract video IDs from various URL formats.

use crate::ExtractorError;

/// Extract video ID from various YouTube URL formats.
pub fn extract_video_id(url: &str) -> Result<String, ExtractorError> {
    // youtube.com/watch?v=ID
    if let Some(pos) = url.find("v=") {
        let rest = &url[pos + 2..];
        let id: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if id.len() == 11 {
            return Ok(id);
        }
    }

    // youtu.be/ID
    if let Some(pos) = url.find("youtu.be/") {
        let rest = &url[pos + 9..];
        let id: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if id.len() == 11 {
            return Ok(id);
        }
    }

    // youtube.com/embed/ID, youtube.com/shorts/ID, youtube.com/v/ID
    for prefix in &["/embed/", "/shorts/", "/v/"] {
        if let Some(pos) = url.find(prefix) {
            let rest = &url[pos + prefix.len()..];
            let id: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            if id.len() == 11 {
                return Ok(id);
            }
        }
    }

    Err(ExtractorError::UnsupportedUrl(format!(
        "Could not extract video ID from: {url}"
    )))
}

/// Check whether a URL is a YouTube URL.
pub fn is_youtube_url(url: &str) -> bool {
    url.contains("youtube.com/") || url.contains("youtu.be/")
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
}
