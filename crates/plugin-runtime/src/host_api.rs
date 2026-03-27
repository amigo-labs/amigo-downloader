//! Host API functions exposed to JavaScript plugins.
//!
//! All network/filesystem access is proxied through these functions.
//! Plugins have NO direct access to the network or filesystem.
//! Functions are registered under `globalThis.amigo.*` in each plugin context.

use std::collections::HashMap;
use std::sync::Arc;

use rquickjs::{Ctx, Function, Object};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Shared state for the Host API, passed into every plugin call.
#[derive(Clone)]
pub struct HostApi {
    http_client: reqwest::Client,
    cookies: Arc<Mutex<HashMap<String, HashMap<String, String>>>>,
    storage: Arc<Mutex<HashMap<String, HashMap<String, String>>>>,
    request_count: Arc<Mutex<u32>>,
    max_requests: u32,
}

impl HostApi {
    pub fn new(max_requests: u32) -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .user_agent("amigo-downloader/0.1.0")
                .build()
                .expect("Failed to build HTTP client"),
            cookies: Arc::new(Mutex::new(HashMap::new())),
            storage: Arc::new(Mutex::new(HashMap::new())),
            request_count: Arc::new(Mutex::new(0)),
            max_requests,
        }
    }

    /// Reset request counter (called before each plugin invocation).
    pub async fn reset_request_count(&self) {
        *self.request_count.lock().await = 0;
    }

    async fn check_request_limit(&self) -> Result<(), String> {
        let mut count = self.request_count.lock().await;
        if *count >= self.max_requests {
            return Err(format!(
                "Plugin exceeded max HTTP requests ({})",
                self.max_requests
            ));
        }
        *count += 1;
        Ok(())
    }

    // --- Network functions (sync wrappers for use from JS) ---

    pub async fn http_get(
        &self,
        url: &str,
        headers: Option<HashMap<String, String>>,
    ) -> Result<(u16, String, HashMap<String, String>), String> {
        self.check_request_limit().await?;
        debug!("Plugin http_get: {url}");

        let mut req = self.http_client.get(url);
        if let Some(hdrs) = headers {
            for (k, v) in hdrs {
                req = req.header(&k, &v);
            }
        }

        let resp = req.send().await.map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let resp_headers: HashMap<String, String> = resp
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let body = resp.text().await.map_err(|e| e.to_string())?;

        Ok((status, body, resp_headers))
    }

    pub async fn http_post(
        &self,
        url: &str,
        body: &str,
        content_type: &str,
    ) -> Result<(u16, String, HashMap<String, String>), String> {
        self.check_request_limit().await?;
        debug!("Plugin http_post: {url}");

        let resp = self
            .http_client
            .post(url)
            .header("Content-Type", content_type)
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = resp.status().as_u16();
        let resp_headers: HashMap<String, String> = resp
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let body = resp.text().await.map_err(|e| e.to_string())?;

        Ok((status, body, resp_headers))
    }

    pub async fn http_head(&self, url: &str) -> Result<(u16, HashMap<String, String>), String> {
        self.check_request_limit().await?;
        debug!("Plugin http_head: {url}");

        let resp = self
            .http_client
            .head(url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = resp.status().as_u16();
        let headers: HashMap<String, String> = resp
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        Ok((status, headers))
    }

    // --- Cookie management ---

    pub async fn set_cookie(&self, domain: &str, name: &str, value: &str) {
        let mut cookies = self.cookies.lock().await;
        cookies
            .entry(domain.to_string())
            .or_default()
            .insert(name.to_string(), value.to_string());
    }

    pub async fn get_cookie(&self, domain: &str, name: &str) -> Option<String> {
        let cookies = self.cookies.lock().await;
        cookies.get(domain)?.get(name).cloned()
    }

    pub async fn clear_cookies(&self, domain: &str) {
        let mut cookies = self.cookies.lock().await;
        cookies.remove(domain);
    }

    // --- Parsing helpers ---

    pub fn regex_match(&self, pattern: &str, text: &str) -> Option<String> {
        let re = regex::Regex::new(pattern).ok()?;
        let caps = re.captures(text)?;
        caps.get(1)
            .or_else(|| caps.get(0))
            .map(|m| m.as_str().to_string())
    }

    pub fn regex_match_all(&self, pattern: &str, text: &str) -> Vec<String> {
        let re = match regex::Regex::new(pattern) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
        re.captures_iter(text)
            .filter_map(|caps| {
                caps.get(1)
                    .or_else(|| caps.get(0))
                    .map(|m| m.as_str().to_string())
            })
            .collect()
    }

    pub fn json_parse(&self, text: &str) -> Result<serde_json::Value, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }

    pub fn base64_decode(&self, input: &str) -> Result<String, String> {
        let bytes = base64_decode_bytes(input)?;
        String::from_utf8(bytes).map_err(|e| e.to_string())
    }

    pub fn base64_encode(&self, input: &str) -> String {
        base64_encode_bytes(input.as_bytes())
    }

    // --- Plugin storage ---

    pub async fn storage_get(&self, plugin_id: &str, key: &str) -> Option<String> {
        let storage = self.storage.lock().await;
        storage.get(plugin_id)?.get(key).cloned()
    }

    pub async fn storage_set(&self, plugin_id: &str, key: &str, value: &str) {
        let mut storage = self.storage.lock().await;
        storage
            .entry(plugin_id.to_string())
            .or_default()
            .insert(key.to_string(), value.to_string());
    }

    pub async fn storage_delete(&self, plugin_id: &str, key: &str) {
        let mut storage = self.storage.lock().await;
        if let Some(store) = storage.get_mut(plugin_id) {
            store.remove(key);
        }
    }

    // --- Logging ---

    pub fn log_info(&self, plugin_id: &str, msg: &str) {
        info!("[plugin:{plugin_id}] {msg}");
    }

    pub fn log_warn(&self, plugin_id: &str, msg: &str) {
        warn!("[plugin:{plugin_id}] {msg}");
    }

    pub fn log_debug(&self, plugin_id: &str, msg: &str) {
        debug!("[plugin:{plugin_id}] {msg}");
    }

    pub fn log_error(&self, plugin_id: &str, msg: &str) {
        error!("[plugin:{plugin_id}] {msg}");
    }
}

/// Register host API functions into a QuickJS context under `globalThis.amigo`.
///
/// Since QuickJS is synchronous, async host functions (HTTP) are exposed as
/// synchronous blocking calls. The plugin JS code can call them directly
/// without await — they block until the result is ready.
///
/// This is acceptable because each plugin runs in its own context and the
/// blocking is done within a `spawn_blocking` task.
pub fn register_host_api(
    ctx: &Ctx<'_>,
    host: Arc<HostApi>,
    plugin_id: &str,
) -> Result<(), crate::Error> {
    let global = ctx.globals();
    let amigo = Object::new(ctx.clone())
        .map_err(|e| crate::Error::Execution(format!("Failed to create amigo object: {e}")))?;

    // --- Synchronous network wrappers ---
    // We use tokio::runtime::Handle to block on async functions from sync QuickJS callbacks.
    // This works because plugins execute within spawn_blocking.

    let h = host.clone();
    amigo
        .set(
            "httpGet",
            Function::new(
                ctx.clone(),
                move |url: String| -> rquickjs::Result<String> {
                    let h = h.clone();
                    let rt = tokio::runtime::Handle::current();
                    let result = rt.block_on(async { h.http_get(&url, None).await });
                    match result {
                        Ok((status, body, headers)) => {
                            let resp = serde_json::json!({
                                "status": status,
                                "body": body,
                                "headers": headers,
                            });
                            Ok(resp.to_string())
                        }
                        Err(e) => Err(rquickjs::Error::new_from_js_message("string", "Error", &e)),
                    }
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "httpPost",
            Function::new(
                ctx.clone(),
                move |url: String,
                      body: String,
                      content_type: String|
                      -> rquickjs::Result<String> {
                    let h = h.clone();
                    let rt = tokio::runtime::Handle::current();
                    let result =
                        rt.block_on(async { h.http_post(&url, &body, &content_type).await });
                    match result {
                        Ok((status, body, headers)) => {
                            let resp = serde_json::json!({
                                "status": status,
                                "body": body,
                                "headers": headers,
                            });
                            Ok(resp.to_string())
                        }
                        Err(e) => Err(rquickjs::Error::new_from_js_message("string", "Error", &e)),
                    }
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "httpHead",
            Function::new(
                ctx.clone(),
                move |url: String| -> rquickjs::Result<String> {
                    let h = h.clone();
                    let rt = tokio::runtime::Handle::current();
                    let result = rt.block_on(async { h.http_head(&url).await });
                    match result {
                        Ok((status, headers)) => {
                            let resp = serde_json::json!({
                                "status": status,
                                "headers": headers,
                            });
                            Ok(resp.to_string())
                        }
                        Err(e) => Err(rquickjs::Error::new_from_js_message("string", "Error", &e)),
                    }
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    // --- Cookies ---
    let h = host.clone();
    amigo
        .set(
            "setCookie",
            Function::new(
                ctx.clone(),
                move |domain: String, name: String, value: String| {
                    let h = h.clone();
                    let rt = tokio::runtime::Handle::current();
                    rt.block_on(async { h.set_cookie(&domain, &name, &value).await });
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "getCookie",
            Function::new(
                ctx.clone(),
                move |domain: String, name: String| -> rquickjs::Result<Option<String>> {
                    let h = h.clone();
                    let rt = tokio::runtime::Handle::current();
                    Ok(rt.block_on(async { h.get_cookie(&domain, &name).await }))
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "clearCookies",
            Function::new(ctx.clone(), move |domain: String| {
                let h = h.clone();
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async { h.clear_cookies(&domain).await });
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    // --- Parsing helpers ---
    let h = host.clone();
    amigo
        .set(
            "regexMatch",
            Function::new(
                ctx.clone(),
                move |pattern: String, text: String| -> Option<String> {
                    h.regex_match(&pattern, &text)
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "regexMatchAll",
            Function::new(
                ctx.clone(),
                move |pattern: String, text: String| -> Vec<String> {
                    h.regex_match_all(&pattern, &text)
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "base64Decode",
            Function::new(
                ctx.clone(),
                move |input: String| -> rquickjs::Result<String> {
                    h.base64_decode(&input)
                        .map_err(|e| rquickjs::Error::new_from_js_message("string", "Error", &e))
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "base64Encode",
            Function::new(ctx.clone(), move |input: String| -> String {
                h.base64_encode(&input)
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    // --- Logging ---
    let pid = plugin_id.to_string();
    let h = host.clone();
    amigo
        .set(
            "logInfo",
            Function::new(ctx.clone(), move |msg: String| {
                h.log_info(&pid, &msg);
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let pid = plugin_id.to_string();
    let h = host.clone();
    amigo
        .set(
            "logWarn",
            Function::new(ctx.clone(), move |msg: String| {
                h.log_warn(&pid, &msg);
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let pid = plugin_id.to_string();
    let h = host.clone();
    amigo
        .set(
            "logError",
            Function::new(ctx.clone(), move |msg: String| {
                h.log_error(&pid, &msg);
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let pid = plugin_id.to_string();
    let h = host.clone();
    amigo
        .set(
            "logDebug",
            Function::new(ctx.clone(), move |msg: String| {
                h.log_debug(&pid, &msg);
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    // --- Storage ---
    let pid = plugin_id.to_string();
    let h = host.clone();
    amigo
        .set(
            "storageGet",
            Function::new(
                ctx.clone(),
                move |key: String| -> rquickjs::Result<Option<String>> {
                    let h = h.clone();
                    let pid = pid.clone();
                    let rt = tokio::runtime::Handle::current();
                    Ok(rt.block_on(async { h.storage_get(&pid, &key).await }))
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let pid = plugin_id.to_string();
    let h = host.clone();
    amigo
        .set(
            "storageSet",
            Function::new(ctx.clone(), move |key: String, value: String| {
                let h = h.clone();
                let pid = pid.clone();
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async { h.storage_set(&pid, &key, &value).await });
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let pid = plugin_id.to_string();
    let h = host.clone();
    amigo
        .set(
            "storageDelete",
            Function::new(ctx.clone(), move |key: String| {
                let h = h.clone();
                let pid = pid.clone();
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async { h.storage_delete(&pid, &key).await });
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    // --- Console bridge ---
    let console = Object::new(ctx.clone())
        .map_err(|e| crate::Error::Execution(format!("Failed to create console: {e}")))?;

    let pid = plugin_id.to_string();
    console
        .set(
            "log",
            Function::new(ctx.clone(), move |msg: String| {
                info!("[plugin:{pid}] {msg}");
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let pid = plugin_id.to_string();
    console
        .set(
            "warn",
            Function::new(ctx.clone(), move |msg: String| {
                warn!("[plugin:{pid}] {msg}");
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let pid = plugin_id.to_string();
    console
        .set(
            "error",
            Function::new(ctx.clone(), move |msg: String| {
                error!("[plugin:{pid}] {msg}");
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    global
        .set("amigo", amigo)
        .map_err(|e| crate::Error::Execution(e.to_string()))?;
    global
        .set("console", console)
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    Ok(())
}

// --- Base64 helpers ---

fn base64_decode_bytes(input: &str) -> Result<Vec<u8>, String> {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut table = [0u8; 256];
    for (i, &c) in alphabet.iter().enumerate() {
        table[c as usize] = i as u8;
    }

    let filtered: Vec<u8> = input
        .bytes()
        .filter(|&c| c != b'=' && c != b'\n' && c != b'\r' && c != b' ')
        .collect();
    let mut buf = Vec::with_capacity(filtered.len() * 3 / 4);

    for chunk in filtered.chunks(4) {
        let vals: Vec<u8> = chunk.iter().map(|&c| table[c as usize]).collect();
        if vals.len() >= 2 {
            buf.push((vals[0] << 2) | (vals[1] >> 4));
        }
        if vals.len() >= 3 {
            buf.push((vals[1] << 4) | (vals[2] >> 2));
        }
        if vals.len() >= 4 {
            buf.push((vals[2] << 6) | vals[3]);
        }
    }

    Ok(buf)
}

fn base64_encode_bytes(input: &[u8]) -> String {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();

    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(alphabet[((triple >> 18) & 0x3F) as usize] as char);
        result.push(alphabet[((triple >> 12) & 0x3F) as usize] as char);
        result.push(if chunk.len() > 1 {
            alphabet[((triple >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        result.push(if chunk.len() > 2 {
            alphabet[(triple & 0x3F) as usize] as char
        } else {
            '='
        });
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex_match() {
        let api = HostApi::new(20);
        let result = api.regex_match(r"/file/([a-zA-Z0-9]+)", "/file/abc123");
        assert_eq!(result, Some("abc123".to_string()));
    }

    #[test]
    fn test_regex_match_all() {
        let api = HostApi::new(20);
        let result = api.regex_match_all(r#"href="([^"]+)""#, r#"<a href="url1"> <a href="url2">"#);
        assert_eq!(result, vec!["url1", "url2"]);
    }

    #[test]
    fn test_base64_roundtrip() {
        let api = HostApi::new(20);
        let encoded = api.base64_encode("Hello, World!");
        let decoded = api.base64_decode(&encoded).unwrap();
        assert_eq!(decoded, "Hello, World!");
    }

    #[tokio::test]
    async fn test_cookie_management() {
        let api = HostApi::new(20);
        api.set_cookie("example.com", "session", "abc123").await;
        assert_eq!(
            api.get_cookie("example.com", "session").await,
            Some("abc123".to_string())
        );
        api.clear_cookies("example.com").await;
        assert_eq!(api.get_cookie("example.com", "session").await, None);
    }

    #[tokio::test]
    async fn test_storage() {
        let api = HostApi::new(20);
        api.storage_set("test-plugin", "key1", "value1").await;
        assert_eq!(
            api.storage_get("test-plugin", "key1").await,
            Some("value1".to_string())
        );
        api.storage_delete("test-plugin", "key1").await;
        assert_eq!(api.storage_get("test-plugin", "key1").await, None);
    }
}
