//! YouTube innertube API client.
//!
//! Uses `android_vr` (Oculus Quest 3) as primary client — delivers direct URLs
//! without PO-tokens or signature decryption. Falls back to `web_embedded` and
//! watch-page HTML extraction.

use serde_json::Value;
use tracing::{debug, info, warn};

use crate::ExtractorError;

/// Innertube client configuration.
struct InnertubeClient {
    name: &'static str,
    _client_name: &'static str,
    client_version: &'static str,
    client_id: &'static str,
    user_agent: &'static str,
    body_builder: fn(&str) -> Value,
}

fn android_vr_body(video_id: &str) -> Value {
    serde_json::json!({
        "videoId": video_id,
        "context": {
            "client": {
                "clientName": "ANDROID_VR",
                "clientVersion": "1.65.10",
                "deviceMake": "Oculus",
                "deviceModel": "Quest 3",
                "androidSdkVersion": 32,
                "osName": "Android",
                "osVersion": "12L",
                "hl": "en",
                "gl": "US"
            }
        }
    })
}

fn web_embedded_body(video_id: &str) -> Value {
    serde_json::json!({
        "videoId": video_id,
        "context": {
            "client": {
                "clientName": "WEB_EMBEDDED_PLAYER",
                "clientVersion": "1.20260115.01.00",
                "hl": "en",
                "gl": "US"
            },
            "thirdParty": {
                "embedUrl": "https://www.reddit.com/"
            }
        }
    })
}

const CLIENTS: &[InnertubeClient] = &[
    InnertubeClient {
        name: "android_vr",
        _client_name: "ANDROID_VR",
        client_version: "1.65.10",
        client_id: "28",
        user_agent: "com.google.android.apps.youtube.vr.oculus/1.65.10 (Linux; U; Android 12L; eureka-user Build/SQ3A.220605.009.A1) gzip",
        body_builder: android_vr_body,
    },
    InnertubeClient {
        name: "web_embedded",
        _client_name: "WEB_EMBEDDED_PLAYER",
        client_version: "1.20260115.01.00",
        client_id: "56",
        user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
        body_builder: web_embedded_body,
    },
];

/// Fetch video player response from YouTube innertube API.
/// Tries android_vr first, then web_embedded, then watch page HTML.
pub async fn fetch_player(
    client: &reqwest::Client,
    video_id: &str,
) -> Result<Value, ExtractorError> {
    for itc in CLIENTS {
        debug!("Trying innertube client: {}", itc.name);

        let body = (itc.body_builder)(video_id);

        let resp = client
            .post("https://www.youtube.com/youtubei/v1/player?prettyPrint=false")
            .header("Content-Type", "application/json")
            .header("User-Agent", itc.user_agent)
            .header("X-YouTube-Client-Name", itc.client_id)
            .header("X-YouTube-Client-Version", itc.client_version)
            .body(body.to_string())
            .send()
            .await?;

        if !resp.status().is_success() {
            warn!("Innertube {} returned status {}", itc.name, resp.status());
            continue;
        }

        let data: Value = resp.json().await?;

        // Check playability
        let status = data
            .pointer("/playabilityStatus/status")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if status != "OK" {
            let reason = data
                .pointer("/playabilityStatus/reason")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            warn!("Innertube {}: status={status}, reason={reason}", itc.name);
            continue;
        }

        // Check for direct URLs
        let has_direct_urls = data
            .pointer("/streamingData/formats")
            .and_then(|v| v.as_array())
            .is_some_and(|fmts| fmts.iter().any(|f| f.get("url").is_some()));

        let has_adaptive_urls = data
            .pointer("/streamingData/adaptiveFormats")
            .and_then(|v| v.as_array())
            .is_some_and(|fmts| fmts.iter().any(|f| f.get("url").is_some()));

        if has_direct_urls || has_adaptive_urls {
            info!("Got streaming data from innertube client {}", itc.name);
            return Ok(data);
        }

        debug!("Innertube {}: no direct URLs in response", itc.name);
    }

    // Fallback: watch page HTML
    fetch_from_watch_page(client, video_id).await
}

/// Extract player response from the watch page HTML.
async fn fetch_from_watch_page(
    client: &reqwest::Client,
    video_id: &str,
) -> Result<Value, ExtractorError> {
    debug!("Falling back to watch page extraction");

    let url = format!("https://www.youtube.com/watch?v={video_id}&has_verified=1");
    let resp = client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await?;

    let html = resp.text().await?;

    let marker = "var ytInitialPlayerResponse = ";
    let start = html.find(marker).ok_or_else(|| {
        ExtractorError::Other("Could not find ytInitialPlayerResponse in page".into())
    })?;

    let json_start = start + marker.len();
    let rest = &html[json_start..];

    let json_str = extract_json_object(rest).ok_or_else(|| {
        ExtractorError::Other("Could not parse ytInitialPlayerResponse JSON".into())
    })?;

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

/// Extract the player.js URL from a watch page HTML (needed for N-parameter challenge).
pub async fn fetch_player_js_url(
    client: &reqwest::Client,
    video_id: &str,
) -> Result<String, ExtractorError> {
    let url = format!("https://www.youtube.com/watch?v={video_id}");
    let resp = client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .send()
        .await?;

    let html = resp.text().await?;

    // Look for player JS URL pattern: /s/player/<hash>/player_ias.vflset/en_US/base.js
    let re = regex::Regex::new(r#"/s/player/[a-f0-9]+/player_ias\.vflset/[^/]+/base\.js"#)
        .map_err(|e| ExtractorError::Other(e.to_string()))?;

    if let Some(m) = re.find(&html) {
        return Ok(format!("https://www.youtube.com{}", m.as_str()));
    }

    // Fallback pattern
    let re2 = regex::Regex::new(r#""jsUrl"\s*:\s*"([^"]+)""#)
        .map_err(|e| ExtractorError::Other(e.to_string()))?;

    if let Some(caps) = re2.captures(&html) {
        let js_path = caps.get(1).unwrap().as_str();
        if js_path.starts_with("http") {
            return Ok(js_path.to_string());
        }
        return Ok(format!("https://www.youtube.com{js_path}"));
    }

    Err(ExtractorError::Other(
        "Could not find player.js URL in watch page".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

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
