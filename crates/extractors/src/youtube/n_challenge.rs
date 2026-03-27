//! YouTube N-parameter challenge solver.
//!
//! YouTube throttles downloads to ~50KB/s unless the `n` query parameter in
//! stream URLs is transformed through a JavaScript function embedded in the
//! player JS. This module extracts that function and runs it via QuickJS.

use std::collections::HashMap;
use std::sync::Mutex;

use tracing::{debug, warn};

use crate::ExtractorError;

/// In-memory cache for player JS code, keyed by player URL.
static PLAYER_JS_CACHE: std::sync::LazyLock<Mutex<HashMap<String, String>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Fetch and cache the player JS source code.
async fn get_player_js(
    client: &reqwest::Client,
    player_js_url: &str,
) -> Result<String, ExtractorError> {
    // Check cache
    {
        let cache = PLAYER_JS_CACHE.lock().unwrap();
        if let Some(js) = cache.get(player_js_url) {
            debug!("Player JS cache hit: {player_js_url}");
            return Ok(js.clone());
        }
    }

    debug!("Fetching player JS: {player_js_url}");
    let resp = client
        .get(player_js_url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .send()
        .await?;

    let js = resp.text().await?;

    // Cache it
    {
        let mut cache = PLAYER_JS_CACHE.lock().unwrap();
        cache.insert(player_js_url.to_string(), js.clone());
    }

    Ok(js)
}

/// Extract the N-parameter transform function from player JS.
///
/// The function is typically found as:
///   var Xva={..., Yy:function(a){...return a.join("")}};
/// and referenced via:
///   a=a.split(""); Xva.Yy(a); ...
///
/// We look for the function name assigned to handle the `n` parameter and
/// extract the complete function body.
fn extract_n_function(player_js: &str) -> Result<String, ExtractorError> {
    // Pattern 1: Modern yt-dlp style — function name from n-parameter assignment
    // Looking for: var b=a.split("") ... .join("")
    // The n-transform function pattern used by yt-dlp:
    let patterns = [
        // Enhanced swap function pattern
        r#"\.get\("n"\)\)&&\(b=([a-zA-Z0-9$]+)(?:\[(\d+)\])?\(b\)"#,
        // Alternative pattern
        r#"var b=a\.split\(""\).*?return b\.join\(""\)"#,
    ];

    let re = regex::Regex::new(patterns[0]).map_err(|e| ExtractorError::Other(e.to_string()))?;

    if let Some(caps) = re.captures(player_js) {
        let func_name = caps.get(1).unwrap().as_str();
        let array_idx = caps.get(2).map(|m| m.as_str());

        debug!("Found n-function reference: {func_name}, index: {array_idx:?}");

        // If there's an array index, we need to find the array and get the function at that index
        if let Some(idx_str) = array_idx {
            let idx: usize = idx_str
                .parse()
                .map_err(|e: std::num::ParseIntError| ExtractorError::Other(e.to_string()))?;
            if let Some(func) = extract_function_from_array(player_js, func_name, idx) {
                return Ok(func);
            }
        }

        // Try to find the function directly
        if let Some(func) = extract_named_function(player_js, func_name) {
            return Ok(func);
        }
    }

    // Fallback: look for the complete n-transform function using a broader pattern
    let fallback_re = regex::Regex::new(
        r#"(?s)function\s*\w+\(a\)\s*\{var b=a\.split\(""\).*?return b\.join\(""\)\}"#,
    )
    .map_err(|e| ExtractorError::Other(e.to_string()))?;

    if let Some(m) = fallback_re.find(player_js) {
        return Ok(m.as_str().to_string());
    }

    Err(ExtractorError::NChallenge(
        "Could not extract n-parameter function from player JS".into(),
    ))
}

/// Extract a named function from player JS.
fn extract_named_function(js: &str, name: &str) -> Option<String> {
    // Pattern: var name=function(a){...};  or  function name(a){...}
    let escaped = regex::escape(name);

    // Try: var name=function(a){...};
    let pat = format!(r"(?s)var\s+{escaped}\s*=\s*function\([^)]*\)\s*\{{");
    if let Ok(re) = regex::Regex::new(&pat)
        && let Some(m) = re.find(js)
    {
        let start = m.start();
        if let Some(end) = find_closing_brace(js, m.end() - 1) {
            return Some(format!(
                "var {name}={}",
                &js[m.start() + m.as_str().find("function").unwrap_or(0) + start - start..=end]
            ));
        }
    }

    // Try: function name(a){...}
    let pat2 = format!(r"(?s)function\s+{escaped}\s*\([^)]*\)\s*\{{");
    if let Ok(re) = regex::Regex::new(&pat2)
        && let Some(m) = re.find(js)
        && let Some(end) = find_closing_brace(js, m.end() - 1)
    {
        return Some(js[m.start()..=end].to_string());
    }

    None
}

/// Extract a function from an array declaration at a specific index.
fn extract_function_from_array(js: &str, array_name: &str, _idx: usize) -> Option<String> {
    let escaped = regex::escape(array_name);
    let pat = format!(r"(?s)var\s+{escaped}\s*=\s*\[");
    let re = regex::Regex::new(&pat).ok()?;
    let m = re.find(js)?;

    // Find the closing bracket
    let start = m.end() - 1;
    let end = find_closing_bracket(js, start)?;

    let array_content = &js[start + 1..end];

    // Split by top-level commas to find functions
    let functions: Vec<&str> = split_top_level(array_content, ',');
    let func_str = functions.get(_idx)?.trim();

    // Wrap anonymous function for execution
    Some(format!("var __n_func = {func_str};"))
}

/// Find the matching closing brace for an opening brace at position `pos`.
fn find_closing_brace(s: &str, pos: usize) -> Option<usize> {
    find_closing_char(s, pos, '{', '}')
}

/// Find the matching closing bracket for an opening bracket at position `pos`.
fn find_closing_bracket(s: &str, pos: usize) -> Option<usize> {
    find_closing_char(s, pos, '[', ']')
}

fn find_closing_char(s: &str, pos: usize, open: char, close: char) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.get(pos).copied()? as char != open {
        return None;
    }

    let mut depth = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, &b) in bytes.iter().enumerate().skip(pos) {
        let ch = b as char;
        if escape_next {
            escape_next = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape_next = true,
            '"' | '\'' => in_string = !in_string,
            c if c == open && !in_string => depth += 1,
            c if c == close && !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Split a string by a delimiter, respecting nested brackets/braces/parens.
fn split_top_level(s: &str, delim: char) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0;
    let mut in_string = false;
    let mut escape_next = false;
    let mut start = 0;

    for (i, ch) in s.char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape_next = true,
            '"' | '\'' => in_string = !in_string,
            '{' | '[' | '(' if !in_string => depth += 1,
            '}' | ']' | ')' if !in_string => depth -= 1,
            c if c == delim && depth == 0 && !in_string => {
                result.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    result.push(&s[start..]);
    result
}

/// Transform the `n` parameter in a stream URL to bypass throttling.
pub async fn transform_n_param(
    client: &reqwest::Client,
    player_js_url: &str,
    stream_url: &str,
) -> Result<String, ExtractorError> {
    // Parse the URL to find the n parameter
    let mut url = reqwest::Url::parse(stream_url)
        .map_err(|e| ExtractorError::Other(format!("Invalid stream URL: {e}")))?;

    let n_value = url
        .query_pairs()
        .find(|(k, _)| k == "n")
        .map(|(_, v)| v.to_string());

    let Some(n_value) = n_value else {
        debug!("No n parameter in URL, skipping transform");
        return Ok(stream_url.to_string());
    };

    debug!("Transforming n parameter: {n_value}");

    // Get player JS
    let player_js = get_player_js(client, player_js_url).await?;

    // Extract the n-function
    let n_function = extract_n_function(&player_js)?;

    // Execute via QuickJS
    let transformed = execute_n_transform(&n_function, &n_value)?;

    debug!("N-parameter transformed: {n_value} → {transformed}");

    // Replace the n parameter in the URL
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(k, v)| {
            if k == "n" {
                (k.to_string(), transformed.clone())
            } else {
                (k.to_string(), v.to_string())
            }
        })
        .collect();

    url.query_pairs_mut().clear();
    for (k, v) in &pairs {
        url.query_pairs_mut().append_pair(k, v);
    }

    Ok(url.to_string())
}

/// Execute the N-parameter transform function using QuickJS.
fn execute_n_transform(n_function_js: &str, n_value: &str) -> Result<String, ExtractorError> {
    // Run JS in a blocking context since rquickjs is sync
    let n_function_js = n_function_js.to_string();
    let n_value = n_value.to_string();

    let rt = rquickjs::Runtime::new()
        .map_err(|e| ExtractorError::NChallenge(format!("Failed to create JS runtime: {e}")))?;
    let ctx = rquickjs::Context::full(&rt)
        .map_err(|e| ExtractorError::NChallenge(format!("Failed to create JS context: {e}")))?;

    ctx.with(|ctx| {
        // Build the script: define the function, then call it
        let script = if n_function_js.contains("__n_func") {
            // Array extraction style
            format!(r#"{n_function_js}; __n_func("{n_value}");"#)
        } else if n_function_js.starts_with("function ") {
            // Named function — extract name and call
            let name_end = n_function_js.find('(').unwrap_or(n_function_js.len());
            let name = n_function_js["function ".len()..name_end].trim();
            format!(r#"{n_function_js}; {name}("{n_value}");"#)
        } else {
            // var name = function style — extract name
            let name_end = n_function_js.find('=').unwrap_or(4);
            let name = n_function_js["var ".len()..name_end].trim();
            format!(r#"{n_function_js}; {name}("{n_value}");"#)
        };

        let result: String = ctx
            .eval(script)
            .map_err(|e| ExtractorError::NChallenge(format!("JS execution failed: {e}")))?;
        Ok(result)
    })
}

/// Transform n-parameters in all stream URLs. Non-fatal — returns original URL on failure.
pub async fn transform_stream_urls(
    client: &reqwest::Client,
    player_js_url: &str,
    urls: &mut [String],
) {
    for url in urls.iter_mut() {
        match transform_n_param(client, player_js_url, url).await {
            Ok(transformed) => *url = transformed,
            Err(e) => {
                warn!("N-parameter transform failed (non-fatal): {e}");
            }
        }
    }
}
