//! Host API functions exposed to JavaScript plugins.
//!
//! All network/filesystem access is proxied through these functions.
//! Plugins have NO direct access to the network or filesystem.
//! Functions are registered under `globalThis.amigo.*` in each plugin context.

use std::collections::HashMap;
use std::sync::Arc;

use rquickjs::{Ctx, Function, Object, Value as JsValue};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Callback for sending notifications from plugins to the UI.
pub type NotifyCallback = Arc<dyn Fn(&str, &str, &str) + Send + Sync>;

/// Callback for requesting a captcha solution from the user.
/// Args: (plugin_id, download_id, image_url, captcha_type) → Result<answer, error>
pub type CaptchaSolveCallback =
    Arc<dyn Fn(String, String, String, String) -> CaptchaFuture + Send + Sync>;

/// Future returned by the captcha solve callback.
pub type CaptchaFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>>;

/// Per-plugin cookie jars: `plugin_id → domain → name → value`.
type CookieJars = HashMap<String, HashMap<String, HashMap<String, String>>>;

// --- Input-size limits for the parsing helpers (audit finding #11) ---
//
// All of these guard the host *outside* the QuickJS context. The 64 MiB
// memory cap on the JS side does not protect against `amigo.htmlQueryAll`
// allocating gigabytes on the host heap, so we cap inputs before they reach
// `scraper`, `regex`, or the base64 decoder.

/// Cap on regex pattern length. Real-world hoster regexes top out at a few
/// hundred chars; anything bigger is almost certainly a ReDoS payload or a
/// plugin bug.
const MAX_REGEX_PATTERN_BYTES: usize = 4 * 1024;

/// Cap on the haystack passed to regex helpers. 4 MiB is comfortably larger
/// than any HTML page a plugin needs to parse with regex (use the proper
/// `htmlQuery*` family for big documents anyway).
const MAX_REGEX_TEXT_BYTES: usize = 4 * 1024 * 1024;

/// Compile-time cap for a single regex's NFA.
const REGEX_COMPILE_SIZE_LIMIT: usize = 1024 * 1024;

/// Compile-time cap for a regex's DFA cache.
const REGEX_DFA_SIZE_LIMIT: usize = 2 * 1024 * 1024;

/// Cap on HTML inputs to scraper-based helpers. 8 MiB is well past any
/// legitimate page; bigger inputs are pathological-nesting OOM bombs.
const MAX_HTML_BYTES: usize = 8 * 1024 * 1024;

/// Cap on base64 input length. The host-side decoder allocates ~3/4 of the
/// input length on the heap, so unbounded inputs map to unbounded host RAM.
const MAX_BASE64_INPUT_BYTES: usize = 8 * 1024 * 1024;

fn enforce_input_limit(field: &str, len: usize, max: usize) -> Result<(), String> {
    if len > max {
        Err(format!(
            "{field} too large ({} bytes; limit {} bytes)",
            len, max
        ))
    } else {
        Ok(())
    }
}

/// Compile a regex with the global plugin-side size limits applied. Returns
/// the compiled `Regex` or a string error suitable for surfacing to the JS
/// caller.
fn compile_plugin_regex(pattern: &str) -> Result<regex::Regex, String> {
    enforce_input_limit("regex pattern", pattern.len(), MAX_REGEX_PATTERN_BYTES)?;
    regex::RegexBuilder::new(pattern)
        .size_limit(REGEX_COMPILE_SIZE_LIMIT)
        .dfa_size_limit(REGEX_DFA_SIZE_LIMIT)
        .build()
        .map_err(|e| format!("invalid regex: {e}"))
}

/// Shared state for the Host API, passed into every plugin call.
#[derive(Clone)]
pub struct HostApi {
    http_client: reqwest::Client,
    /// Per-plugin cookie jars (`plugin_id → domain → name → value`). The
    /// outer key is the calling plugin's id, set at register-time via
    /// `register_host_api` and not under JS control, so
    /// `getCookie("real-debrid.com", "auth")` from plugin A cannot read a
    /// cookie that plugin B stashed under the same domain.
    cookies: Arc<Mutex<CookieJars>>,
    storage: Arc<Mutex<HashMap<String, HashMap<String, String>>>>,
    request_count: Arc<Mutex<u32>>,
    max_requests: u32,
    /// Per-plugin storage quota (bytes). Counted across all `(key, value)`
    /// pairs of a single plugin's storage map.
    max_storage_bytes: u64,
    notify_callback: Option<NotifyCallback>,
    captcha_callback: Option<CaptchaSolveCallback>,
    /// When false, reject plugin HTTP requests whose resolved IP is loopback,
    /// private (RFC1918), link-local, CGNAT, or otherwise non-public.
    allow_private_network: bool,
}

/// Default per-plugin storage quota when constructed via [`HostApi::new`]
/// without a SandboxLimits. Matches `SandboxLimits::default().max_storage_bytes`.
const DEFAULT_MAX_STORAGE_BYTES: u64 = 1024 * 1024;

impl HostApi {
    pub fn new(max_requests: u32) -> Self {
        Self {
            http_client: reqwest::Client::builder()
                .user_agent("amigo-downloader/0.1.0")
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            cookies: Arc::new(Mutex::new(HashMap::new())),
            storage: Arc::new(Mutex::new(HashMap::new())),
            request_count: Arc::new(Mutex::new(0)),
            max_requests,
            max_storage_bytes: DEFAULT_MAX_STORAGE_BYTES,
            notify_callback: None,
            captcha_callback: None,
            allow_private_network: false,
        }
    }

    /// Build a `HostApi` whose limits match a `SandboxLimits`, including the
    /// `allow_private_network` setting.
    pub fn from_sandbox(limits: &crate::sandbox::SandboxLimits) -> Self {
        let mut api = Self::new(limits.max_http_requests);
        api.allow_private_network = limits.allow_private_network;
        api.max_storage_bytes = limits.max_storage_bytes;
        api
    }

    /// Set the callback for plugin notifications.
    pub fn set_notify_callback(&mut self, callback: NotifyCallback) {
        self.notify_callback = Some(callback);
    }

    /// Set the callback for captcha solving.
    pub fn set_captcha_callback(&mut self, callback: CaptchaSolveCallback) {
        self.captcha_callback = Some(callback);
    }

    /// Allow plugin HTTP requests to reach private / loopback / link-local
    /// addresses. Off by default; see [`crate::sandbox::SandboxLimits`].
    pub fn set_allow_private_network(&mut self, allow: bool) {
        self.allow_private_network = allow;
    }

    /// Validate `url` against the SSRF policy. Rejects non-http(s) schemes and
    /// — when `allow_private_network` is false — any URL that resolves to a
    /// loopback / private / link-local / CGNAT address, which includes the
    /// AWS/GCP metadata endpoint `169.254.169.254`.
    async fn check_url_allowed(&self, url: &str) -> Result<(), String> {
        let parsed = url::Url::parse(url).map_err(|e| format!("Invalid URL: {e}"))?;
        match parsed.scheme() {
            "http" | "https" => {}
            other => return Err(format!("URL scheme not allowed: {other}")),
        }
        if self.allow_private_network {
            return Ok(());
        }
        let host = parsed
            .host_str()
            .ok_or_else(|| "URL is missing a host".to_string())?;
        let port = parsed.port_or_known_default().unwrap_or(80);

        // If the host is an IP literal, check it directly without DNS.
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            if is_blocked_ip(ip) {
                return Err(format!(
                    "Request blocked: {host} is a private/loopback address"
                ));
            }
            return Ok(());
        }

        let addrs = tokio::net::lookup_host((host, port))
            .await
            .map_err(|e| format!("DNS lookup failed for {host}: {e}"))?;
        for addr in addrs {
            if is_blocked_ip(addr.ip()) {
                return Err(format!(
                    "Request blocked: {host} resolves to private/loopback address {}",
                    addr.ip()
                ));
            }
        }
        Ok(())
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
        self.check_url_allowed(url).await?;
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
        headers: Option<HashMap<String, String>>,
    ) -> Result<(u16, String, HashMap<String, String>), String> {
        self.check_request_limit().await?;
        self.check_url_allowed(url).await?;
        debug!("Plugin http_post: {url}");

        let mut req = self
            .http_client
            .post(url)
            .header("Content-Type", content_type)
            .body(body.to_string());
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

    pub async fn http_head(
        &self,
        url: &str,
        headers: Option<HashMap<String, String>>,
    ) -> Result<(u16, HashMap<String, String>), String> {
        self.check_request_limit().await?;
        self.check_url_allowed(url).await?;
        debug!("Plugin http_head: {url}");

        let mut req = self.http_client.head(url);
        if let Some(hdrs) = headers {
            for (k, v) in hdrs {
                req = req.header(&k, &v);
            }
        }

        let resp = req.send().await.map_err(|e| e.to_string())?;

        let status = resp.status().as_u16();
        let headers: HashMap<String, String> = resp
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        Ok((status, headers))
    }

    // --- Cookie management ---
    //
    // Each plugin sees its own private cookie jar. The plugin_id argument is
    // captured at register-time by the JS bindings (see register_host_api)
    // so a malicious plugin cannot pass a different plugin_id from JS to
    // read another plugin's cookies.

    pub async fn set_cookie(&self, plugin_id: &str, domain: &str, name: &str, value: &str) {
        let mut cookies = self.cookies.lock().await;
        cookies
            .entry(plugin_id.to_string())
            .or_default()
            .entry(domain.to_string())
            .or_default()
            .insert(name.to_string(), value.to_string());
    }

    pub async fn get_cookie(&self, plugin_id: &str, domain: &str, name: &str) -> Option<String> {
        let cookies = self.cookies.lock().await;
        cookies.get(plugin_id)?.get(domain)?.get(name).cloned()
    }

    pub async fn clear_cookies(&self, plugin_id: &str, domain: &str) {
        let mut cookies = self.cookies.lock().await;
        if let Some(jar) = cookies.get_mut(plugin_id) {
            jar.remove(domain);
        }
    }

    // --- Parsing helpers ---

    pub fn regex_match(&self, pattern: &str, text: &str) -> Option<String> {
        if enforce_input_limit("regex text", text.len(), MAX_REGEX_TEXT_BYTES).is_err() {
            return None;
        }
        let re = compile_plugin_regex(pattern).ok()?;
        let caps = re.captures(text)?;
        caps.get(1)
            .or_else(|| caps.get(0))
            .map(|m| m.as_str().to_string())
    }

    pub fn regex_match_all(&self, pattern: &str, text: &str) -> Vec<String> {
        if enforce_input_limit("regex text", text.len(), MAX_REGEX_TEXT_BYTES).is_err() {
            return Vec::new();
        }
        let re = match compile_plugin_regex(pattern) {
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

    // --- Crypto functions ---

    pub fn md5(&self, input: &str) -> String {
        use md5::Digest;
        let mut hasher = md5::Md5::new();
        hasher.update(input.as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn sha1(&self, input: &str) -> String {
        use sha1::Digest;
        let mut hasher = sha1::Sha1::new();
        hasher.update(input.as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn sha256(&self, input: &str) -> String {
        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        hasher.update(input.as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn hmac_sha256(&self, key: &str, data: &str) -> Result<String, String> {
        use hmac::{Hmac, Mac};
        type HmacSha256 = Hmac<sha2::Sha256>;
        let mut mac =
            HmacSha256::new_from_slice(key.as_bytes()).map_err(|e| format!("HMAC error: {e}"))?;
        mac.update(data.as_bytes());
        Ok(hex::encode(mac.finalize().into_bytes()))
    }

    pub fn aes_decrypt_cbc(
        &self,
        data_b64: &str,
        key_hex: &str,
        iv_hex: &str,
    ) -> Result<String, String> {
        use aes::cipher::{BlockDecryptMut, KeyIvInit};
        type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

        let data = base64_decode_bytes(data_b64)?;
        let key = hex::decode(key_hex).map_err(|e| format!("Invalid key hex: {e}"))?;
        let iv = hex::decode(iv_hex).map_err(|e| format!("Invalid IV hex: {e}"))?;

        if key.len() != 16 {
            return Err(format!("AES key must be 16 bytes, got {}", key.len()));
        }
        if iv.len() != 16 {
            return Err(format!("AES IV must be 16 bytes, got {}", iv.len()));
        }

        let mut buf = data.clone();
        let decryptor =
            Aes128CbcDec::new_from_slices(&key, &iv).map_err(|e| format!("AES init error: {e}"))?;
        let decrypted = decryptor
            .decrypt_padded_mut::<aes::cipher::block_padding::Pkcs7>(&mut buf)
            .map_err(|e| format!("AES decrypt error: {e}"))?;

        Ok(base64_encode_bytes(decrypted))
    }

    pub fn aes_encrypt_cbc(
        &self,
        data_b64: &str,
        key_hex: &str,
        iv_hex: &str,
    ) -> Result<String, String> {
        use aes::cipher::{BlockEncryptMut, KeyIvInit};
        type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;

        let data = base64_decode_bytes(data_b64)?;
        let key = hex::decode(key_hex).map_err(|e| format!("Invalid key hex: {e}"))?;
        let iv = hex::decode(iv_hex).map_err(|e| format!("Invalid IV hex: {e}"))?;

        if key.len() != 16 {
            return Err(format!("AES key must be 16 bytes, got {}", key.len()));
        }
        if iv.len() != 16 {
            return Err(format!("AES IV must be 16 bytes, got {}", iv.len()));
        }

        // Allocate buffer with padding space
        let block_size = 16;
        let padded_len = ((data.len() / block_size) + 1) * block_size;
        let mut buf = vec![0u8; padded_len];
        buf[..data.len()].copy_from_slice(&data);

        let encryptor =
            Aes128CbcEnc::new_from_slices(&key, &iv).map_err(|e| format!("AES init error: {e}"))?;
        let encrypted = encryptor
            .encrypt_padded_mut::<aes::cipher::block_padding::Pkcs7>(&mut buf, data.len())
            .map_err(|e| format!("AES encrypt error: {e}"))?;

        Ok(base64_encode_bytes(encrypted))
    }

    // --- Plugin storage ---

    pub async fn storage_get(&self, plugin_id: &str, key: &str) -> Option<String> {
        let storage = self.storage.lock().await;
        storage.get(plugin_id)?.get(key).cloned()
    }

    /// Persist `value` under `(plugin_id, key)`. Enforces the per-plugin
    /// quota configured via [`SandboxLimits::max_storage_bytes`]. The total
    /// is computed across all `(key, value)` byte lengths in this plugin's
    /// map after the proposed write; an oversize write is rejected without
    /// mutating state. Returns `Err` with a human-readable message on quota
    /// breach so the JS binding can surface it as a thrown exception.
    pub async fn storage_set(&self, plugin_id: &str, key: &str, value: &str) -> Result<(), String> {
        let mut storage = self.storage.lock().await;
        let map = storage.entry(plugin_id.to_string()).or_default();

        let old_size = map
            .get(key)
            .map(|v| v.len() as u64 + key.len() as u64)
            .unwrap_or(0);
        let new_pair = key.len() as u64 + value.len() as u64;
        let mut total: u64 = map
            .iter()
            .map(|(k, v)| k.len() as u64 + v.len() as u64)
            .sum();
        total = total.saturating_sub(old_size).saturating_add(new_pair);

        if total > self.max_storage_bytes {
            return Err(format!(
                "storage quota exceeded for plugin '{plugin_id}': {total} > {} bytes",
                self.max_storage_bytes
            ));
        }
        map.insert(key.to_string(), value.to_string());
        Ok(())
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

    // --- Notifications ---

    pub fn notify(&self, plugin_id: &str, title: &str, message: &str) {
        info!("[plugin:{plugin_id}] notify: {title} — {message}");
        if let Some(cb) = &self.notify_callback {
            cb(plugin_id, title, message);
        }
    }

    // --- Captcha ---

    pub async fn solve_captcha(
        &self,
        plugin_id: &str,
        download_id: &str,
        image_url: &str,
        captcha_type: &str,
    ) -> Result<String, String> {
        let cb = self
            .captcha_callback
            .as_ref()
            .ok_or_else(|| "Captcha solving not available (no UI connected)".to_string())?;

        info!("[plugin:{plugin_id}] captcha requested: {captcha_type}");
        (cb)(
            plugin_id.to_string(),
            download_id.to_string(),
            image_url.to_string(),
            captcha_type.to_string(),
        )
        .await
    }

    // --- URL helpers ---

    pub fn url_resolve(&self, base: &str, relative: &str) -> Result<String, String> {
        let base_url = url::Url::parse(base).map_err(|e| format!("Invalid base URL: {e}"))?;
        let resolved = base_url
            .join(relative)
            .map_err(|e| format!("Failed to resolve URL: {e}"))?;
        Ok(resolved.to_string())
    }

    pub fn url_parse(&self, raw: &str) -> Result<serde_json::Value, String> {
        let u = url::Url::parse(raw).map_err(|e| format!("Invalid URL: {e}"))?;
        Ok(serde_json::json!({
            "protocol": u.scheme(),
            "host": u.host_str().unwrap_or(""),
            "port": u.port(),
            "pathname": u.path(),
            "search": u.query().map(|q| format!("?{q}")).unwrap_or_default(),
            "hash": u.fragment().map(|f| format!("#{f}")).unwrap_or_default(),
            "origin": u.origin().unicode_serialization(),
        }))
    }

    pub fn url_filename(&self, raw: &str) -> Option<String> {
        let path = raw.split('?').next().unwrap_or(raw);
        let segment = path.rsplit('/').next()?;
        if segment.is_empty() {
            return None;
        }
        Some(
            urlencoding::decode(segment)
                .unwrap_or(std::borrow::Cow::Borrowed(segment))
                .into_owned(),
        )
    }

    // --- HTML helpers ---

    pub fn html_query_all(&self, html: &str, selector: &str) -> Result<Vec<String>, String> {
        use scraper::{Html, Selector};
        enforce_input_limit("html", html.len(), MAX_HTML_BYTES)?;
        let doc = Html::parse_document(html);
        let sel = Selector::parse(selector).map_err(|e| format!("Invalid CSS selector: {e:?}"))?;
        Ok(doc.select(&sel).map(|el| el.html()).collect())
    }

    pub fn html_query_text(&self, html: &str, selector: &str) -> Result<Option<String>, String> {
        use scraper::{Html, Selector};
        enforce_input_limit("html", html.len(), MAX_HTML_BYTES)?;
        let doc = Html::parse_document(html);
        let sel = Selector::parse(selector).map_err(|e| format!("Invalid CSS selector: {e:?}"))?;
        Ok(doc
            .select(&sel)
            .next()
            .map(|el| el.text().collect::<String>()))
    }

    pub fn html_query_attr(
        &self,
        html: &str,
        selector: &str,
        attr: &str,
    ) -> Result<Option<String>, String> {
        use scraper::{Html, Selector};
        enforce_input_limit("html", html.len(), MAX_HTML_BYTES)?;
        let doc = Html::parse_document(html);
        let sel = Selector::parse(selector).map_err(|e| format!("Invalid CSS selector: {e:?}"))?;
        Ok(doc
            .select(&sel)
            .next()
            .and_then(|el| el.value().attr(attr).map(|s| s.to_string())))
    }

    pub fn html_search_meta(&self, html: &str, names: &[String]) -> Option<String> {
        use scraper::{Html, Selector};
        if enforce_input_limit("html", html.len(), MAX_HTML_BYTES).is_err() {
            return None;
        }
        let doc = Html::parse_document(html);
        let selectors = ["meta[name='{name}']", "meta[property='{name}']"];
        for name in names {
            for tmpl in &selectors {
                let css = tmpl.replace("{name}", name);
                if let Ok(sel) = Selector::parse(&css)
                    && let Some(content) = doc
                        .select(&sel)
                        .next()
                        .and_then(|el| el.value().attr("content"))
                {
                    return Some(content.to_string());
                }
            }
        }
        None
    }

    pub fn html_extract_title(&self, html: &str) -> Option<String> {
        use scraper::{Html, Selector};
        if enforce_input_limit("html", html.len(), MAX_HTML_BYTES).is_err() {
            return None;
        }
        let doc = Html::parse_document(html);
        let sel = Selector::parse("title").ok()?;
        doc.select(&sel)
            .next()
            .map(|el| el.text().collect::<String>())
    }

    pub fn html_hidden_inputs(&self, html: &str) -> HashMap<String, String> {
        use scraper::{Html, Selector};
        if enforce_input_limit("html", html.len(), MAX_HTML_BYTES).is_err() {
            return HashMap::new();
        }
        let doc = Html::parse_document(html);
        let sel = match Selector::parse("input[type='hidden']") {
            Ok(s) => s,
            Err(_) => return HashMap::new(),
        };
        doc.select(&sel)
            .filter_map(|el| {
                let name = el.value().attr("name")?.to_string();
                let value = el.value().attr("value").unwrap_or("").to_string();
                Some((name, value))
            })
            .collect()
    }

    pub fn search_json(&self, start_pattern: &str, html: &str) -> Option<String> {
        if enforce_input_limit("search_json html", html.len(), MAX_HTML_BYTES).is_err() {
            return None;
        }
        let re = compile_plugin_regex(start_pattern).ok()?;
        let m = re.find(html)?;
        let rest = &html[m.end()..];

        // Find balanced braces or brackets
        let first = rest.chars().next()?;
        let (open, close) = match first {
            '{' => ('{', '}'),
            '[' => ('[', ']'),
            _ => return None,
        };

        let mut depth = 0;
        let mut in_string = false;
        let mut escape_next = false;

        for (i, ch) in rest.char_indices() {
            if escape_next {
                escape_next = false;
                continue;
            }
            if ch == '\\' && in_string {
                escape_next = true;
                continue;
            }
            if ch == '"' {
                in_string = !in_string;
                continue;
            }
            if !in_string {
                if ch == open {
                    depth += 1;
                } else if ch == close {
                    depth -= 1;
                    if depth == 0 {
                        let json_str = &rest[..=i];
                        // Validate it's actually JSON
                        if serde_json::from_str::<serde_json::Value>(json_str).is_ok() {
                            return Some(json_str.to_string());
                        }
                        return None;
                    }
                }
            }
        }
        None
    }

    // --- Additional regex helpers ---

    pub fn regex_replace(&self, pattern: &str, text: &str, replacement: &str) -> Option<String> {
        if enforce_input_limit("regex text", text.len(), MAX_REGEX_TEXT_BYTES).is_err() {
            return None;
        }
        let re = compile_plugin_regex(pattern).ok()?;
        Some(re.replace_all(text, replacement).into_owned())
    }

    pub fn regex_test(&self, pattern: &str, text: &str) -> bool {
        if enforce_input_limit("regex text", text.len(), MAX_REGEX_TEXT_BYTES).is_err() {
            return false;
        }
        compile_plugin_regex(pattern)
            .map(|re| re.is_match(text))
            .unwrap_or(false)
    }

    pub fn regex_split(&self, pattern: &str, text: &str) -> Vec<String> {
        if enforce_input_limit("regex text", text.len(), MAX_REGEX_TEXT_BYTES).is_err() {
            return vec![text.to_string()];
        }
        match compile_plugin_regex(pattern) {
            Ok(re) => re.split(text).map(|s| s.to_string()).collect(),
            Err(_) => vec![text.to_string()],
        }
    }

    // --- Utility helpers ---

    pub fn parse_duration(&self, input: &str) -> Option<f64> {
        let input = input.trim();

        // ISO 8601: PT1H23M45S
        if input.starts_with("PT") || input.starts_with("P") {
            return parse_iso8601_duration(input);
        }

        // HH:MM:SS or MM:SS
        let parts: Vec<&str> = input.split(':').collect();
        match parts.len() {
            2 => {
                let m: f64 = parts[0].parse().ok()?;
                let s: f64 = parts[1].parse().ok()?;
                Some(m * 60.0 + s)
            }
            3 => {
                let h: f64 = parts[0].parse().ok()?;
                let m: f64 = parts[1].parse().ok()?;
                let s: f64 = parts[2].parse().ok()?;
                Some(h * 3600.0 + m * 60.0 + s)
            }
            _ => {
                // Try plain seconds
                input.parse::<f64>().ok()
            }
        }
    }

    pub fn sanitize_filename(&self, name: &str) -> String {
        let mut result: String = name
            .chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
                c if c.is_control() => '_',
                c => c,
            })
            .collect();
        // Remove leading/trailing dots and spaces
        result = result.trim_matches(|c| c == '.' || c == ' ').to_string();
        if result.is_empty() {
            result = "download".to_string();
        }
        result
    }

    // --- New Host-API extensions for plugin strategy ---

    /// HTTP POST with form-encoded body (application/x-www-form-urlencoded).
    pub async fn http_post_form(
        &self,
        url: &str,
        fields: &HashMap<String, String>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<(u16, String, HashMap<String, String>), String> {
        self.check_request_limit().await?;
        self.check_url_allowed(url).await?;
        debug!("Plugin http_post_form: {url}");

        let mut req = self.http_client.post(url).form(fields);
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

    /// HTTP GET returning binary content as base64-encoded string.
    pub async fn http_get_binary(
        &self,
        url: &str,
        headers: Option<HashMap<String, String>>,
    ) -> Result<String, String> {
        self.check_request_limit().await?;
        self.check_url_allowed(url).await?;
        debug!("Plugin http_get_binary: {url}");

        let mut req = self.http_client.get(url);
        if let Some(hdrs) = headers {
            for (k, v) in hdrs {
                req = req.header(&k, &v);
            }
        }

        let resp = req.send().await.map_err(|e| e.to_string())?;
        let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
        Ok(base64_encode_bytes(&bytes))
    }

    /// Follow all redirects for a URL and return the final URL.
    pub async fn http_follow_redirects(
        &self,
        url: &str,
        headers: Option<HashMap<String, String>>,
    ) -> Result<String, String> {
        self.check_request_limit().await?;
        self.check_url_allowed(url).await?;
        debug!("Plugin http_follow_redirects: {url}");

        let mut req = self.http_client.head(url);
        if let Some(hdrs) = headers {
            for (k, v) in hdrs {
                req = req.header(&k, &v);
            }
        }

        let resp = req.send().await.map_err(|e| e.to_string())?;
        Ok(resp.url().to_string())
    }

    /// Get an attribute value from ALL elements matching a CSS selector.
    pub fn html_query_all_attrs(
        &self,
        html: &str,
        selector: &str,
        attr: &str,
    ) -> Result<Vec<String>, String> {
        use scraper::{Html, Selector};
        enforce_input_limit("html", html.len(), MAX_HTML_BYTES)?;
        let doc = Html::parse_document(html);
        let sel = Selector::parse(selector).map_err(|e| format!("Invalid CSS selector: {e:?}"))?;
        Ok(doc
            .select(&sel)
            .filter_map(|el| el.value().attr(attr).map(|s| s.to_string()))
            .collect())
    }
}

fn parse_iso8601_duration(input: &str) -> Option<f64> {
    let s = input.strip_prefix('P')?;
    let mut total = 0.0;
    let mut num_buf = String::new();
    let mut in_time = false;

    for c in s.chars() {
        if c == 'T' {
            in_time = true;
            continue;
        }
        if c.is_ascii_digit() || c == '.' {
            num_buf.push(c);
        } else {
            let val: f64 = num_buf.parse().ok()?;
            num_buf.clear();
            match (c, in_time) {
                ('H', true) => total += val * 3600.0,
                ('M', true) => total += val * 60.0,
                ('S', true) => total += val,
                ('D', false) => total += val * 86400.0,
                _ => {}
            }
        }
    }
    Some(total)
}

/// JavaScript shim that wraps raw JSON-returning functions into object-returning ones.
/// This is injected after all Rust functions are registered.
const JS_SHIM: &str = r#"
(function() {
    var a = amigo;

    // HTTP: wrap raw JSON-string returns into parsed objects
    a.httpGet = function(url, opts) {
        var hdr = (opts && opts.headers) ? JSON.stringify(opts.headers) : null;
        return JSON.parse(a.__rawHttpGet(url, hdr));
    };
    a.httpPost = function(url, body, contentType, opts) {
        var hdr = (opts && opts.headers) ? JSON.stringify(opts.headers) : null;
        return JSON.parse(a.__rawHttpPost(url, body, contentType, hdr));
    };
    a.httpHead = function(url, opts) {
        var hdr = (opts && opts.headers) ? JSON.stringify(opts.headers) : null;
        return JSON.parse(a.__rawHttpHead(url, hdr));
    };

    // Convenience: GET and parse body as JSON
    a.httpGetJson = function(url, opts) {
        var resp = a.httpGet(url, opts);
        resp.data = JSON.parse(resp.body);
        return resp;
    };

    // HTTP extensions
    a.httpPostForm = function(url, fields, opts) {
        var hdr = (opts && opts.headers) ? JSON.stringify(opts.headers) : null;
        return JSON.parse(a.__rawHttpPostForm(url, JSON.stringify(fields), hdr));
    };
    a.httpGetBinary = function(url, opts) {
        var hdr = (opts && opts.headers) ? JSON.stringify(opts.headers) : null;
        return a.__rawHttpGetBinary(url, hdr);
    };
    a.httpFollowRedirects = function(url, opts) {
        var hdr = (opts && opts.headers) ? JSON.stringify(opts.headers) : null;
        return a.__rawHttpFollowRedirects(url, hdr);
    };

    // HTML: batch attribute extraction
    a.htmlQueryAllAttrs = function(html, selector, attr) {
        return JSON.parse(a.__rawHtmlQueryAllAttrs(html, selector, attr));
    };

    // URL: parse returns object
    a.urlParse = function(url) {
        return JSON.parse(a.__rawUrlParse(url));
    };

    // HTML: arrays and objects
    a.htmlQueryAll = function(html, selector) {
        return JSON.parse(a.__rawHtmlQueryAll(html, selector));
    };
    a.htmlSearchMeta = function(html, names) {
        var arr = Array.isArray(names) ? names : [names];
        return a.__rawHtmlSearchMeta(html, JSON.stringify(arr));
    };
    a.htmlHiddenInputs = function(html) {
        return JSON.parse(a.__rawHtmlHiddenInputs(html));
    };
    a.searchJson = function(startPattern, html) {
        var s = a.__rawSearchJson(startPattern, html);
        return s ? JSON.parse(s) : null;
    };

    // Regex: split returns array
    a.regexSplit = function(pattern, text) {
        return JSON.parse(a.__rawRegexSplit(pattern, text));
    };

    // Captcha: wraps raw function
    a.solveCaptcha = function(imageUrl, captchaType) {
        return a.__rawSolveCaptcha(imageUrl, captchaType || "image");
    };

    // Safe deep property access: amigo.traverse(obj, ["a", "b", "c"]) or amigo.traverse(obj, "a.b.c")
    a.traverse = function(obj, path) {
        if (typeof path === "string") path = path.split(".");
        var cur = obj;
        for (var i = 0; i < path.length; i++) {
            if (cur == null) return null;
            cur = cur[path[i]];
        }
        return cur == null ? null : cur;
    };
})();
"#;

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
    //
    // Raw functions return JSON strings. A JS shim (injected below) wraps them
    // into `amigo.httpGet(url, opts?)` that returns parsed objects directly.

    let h = host.clone();
    amigo
        .set(
            "__rawHttpGet",
            Function::new(
                ctx.clone(),
                move |url: String, headers_json: Option<String>| -> rquickjs::Result<String> {
                    let h = h.clone();
                    let headers: Option<HashMap<String, String>> = headers_json
                        .as_deref()
                        .and_then(|s| serde_json::from_str(s).ok());
                    let rt = tokio::runtime::Handle::current();
                    let result = tokio::task::block_in_place(|| {
                        rt.block_on(async { h.http_get(&url, headers).await })
                    });
                    match result {
                        Ok((status, body, resp_headers)) => {
                            let resp = serde_json::json!({
                                "status": status,
                                "body": body,
                                "headers": resp_headers,
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
            "__rawHttpPost",
            Function::new(
                ctx.clone(),
                move |url: String,
                      body: String,
                      content_type: String,
                      headers_json: Option<String>|
                      -> rquickjs::Result<String> {
                    let h = h.clone();
                    let headers: Option<HashMap<String, String>> = headers_json
                        .as_deref()
                        .and_then(|s| serde_json::from_str(s).ok());
                    let rt = tokio::runtime::Handle::current();
                    let result = tokio::task::block_in_place(|| {
                        rt.block_on(async {
                            h.http_post(&url, &body, &content_type, headers).await
                        })
                    });
                    match result {
                        Ok((status, body, resp_headers)) => {
                            let resp = serde_json::json!({
                                "status": status,
                                "body": body,
                                "headers": resp_headers,
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
            "__rawHttpHead",
            Function::new(
                ctx.clone(),
                move |url: String, headers_json: Option<String>| -> rquickjs::Result<String> {
                    let h = h.clone();
                    let headers: Option<HashMap<String, String>> = headers_json
                        .as_deref()
                        .and_then(|s| serde_json::from_str(s).ok());
                    let rt = tokio::runtime::Handle::current();
                    let result = tokio::task::block_in_place(|| {
                        rt.block_on(async { h.http_head(&url, headers).await })
                    });
                    match result {
                        Ok((status, resp_headers)) => {
                            let resp = serde_json::json!({
                                "status": status,
                                "headers": resp_headers,
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
    // plugin_id is captured at register-time and is NOT exposed to JS, so a
    // plugin's setCookie / getCookie / clearCookies always operate on its
    // own private cookie jar.
    let pid = plugin_id.to_string();
    let h = host.clone();
    amigo
        .set(
            "setCookie",
            Function::new(
                ctx.clone(),
                move |domain: String, name: String, value: String| {
                    let h = h.clone();
                    let pid = pid.clone();
                    let rt = tokio::runtime::Handle::current();
                    tokio::task::block_in_place(|| {
                        rt.block_on(async { h.set_cookie(&pid, &domain, &name, &value).await })
                    });
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let pid = plugin_id.to_string();
    let h = host.clone();
    amigo
        .set(
            "getCookie",
            Function::new(
                ctx.clone(),
                move |domain: String, name: String| -> rquickjs::Result<Option<String>> {
                    let h = h.clone();
                    let pid = pid.clone();
                    let rt = tokio::runtime::Handle::current();
                    Ok(tokio::task::block_in_place(|| {
                        rt.block_on(async { h.get_cookie(&pid, &domain, &name).await })
                    }))
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let pid = plugin_id.to_string();
    let h = host.clone();
    amigo
        .set(
            "clearCookies",
            Function::new(ctx.clone(), move |domain: String| {
                let h = h.clone();
                let pid = pid.clone();
                let rt = tokio::runtime::Handle::current();
                tokio::task::block_in_place(|| {
                    rt.block_on(async { h.clear_cookies(&pid, &domain).await })
                });
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

    // --- Crypto ---
    let h = host.clone();
    amigo
        .set(
            "md5",
            Function::new(ctx.clone(), move |input: String| -> String {
                h.md5(&input)
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "sha1",
            Function::new(ctx.clone(), move |input: String| -> String {
                h.sha1(&input)
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "sha256",
            Function::new(ctx.clone(), move |input: String| -> String {
                h.sha256(&input)
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "hmacSha256",
            Function::new(
                ctx.clone(),
                move |key: String, data: String| -> rquickjs::Result<String> {
                    h.hmac_sha256(&key, &data)
                        .map_err(|e| rquickjs::Error::new_from_js_message("string", "Error", &e))
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "aesDecryptCbc",
            Function::new(
                ctx.clone(),
                move |data: String, key: String, iv: String| -> rquickjs::Result<String> {
                    h.aes_decrypt_cbc(&data, &key, &iv)
                        .map_err(|e| rquickjs::Error::new_from_js_message("string", "Error", &e))
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "aesEncryptCbc",
            Function::new(
                ctx.clone(),
                move |data: String, key: String, iv: String| -> rquickjs::Result<String> {
                    h.aes_encrypt_cbc(&data, &key, &iv)
                        .map_err(|e| rquickjs::Error::new_from_js_message("string", "Error", &e))
                },
            ),
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
                    Ok(tokio::task::block_in_place(|| {
                        rt.block_on(async { h.storage_get(&pid, &key).await })
                    }))
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let pid = plugin_id.to_string();
    let h = host.clone();
    amigo
        .set(
            "storageSet",
            Function::new(
                ctx.clone(),
                move |key: String, value: String| -> rquickjs::Result<()> {
                    let h = h.clone();
                    let pid = pid.clone();
                    let rt = tokio::runtime::Handle::current();
                    tokio::task::block_in_place(|| {
                        rt.block_on(async { h.storage_set(&pid, &key, &value).await })
                    })
                    .map_err(|e| rquickjs::Error::new_from_js_message("string", "Error", &e))
                },
            ),
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
                tokio::task::block_in_place(|| {
                    rt.block_on(async { h.storage_delete(&pid, &key).await })
                });
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    // --- URL helpers ---
    let h = host.clone();
    amigo
        .set(
            "urlResolve",
            Function::new(
                ctx.clone(),
                move |base: String, relative: String| -> rquickjs::Result<String> {
                    h.url_resolve(&base, &relative)
                        .map_err(|e| rquickjs::Error::new_from_js_message("string", "Error", &e))
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "__rawUrlParse",
            Function::new(
                ctx.clone(),
                move |url: String| -> rquickjs::Result<String> {
                    h.url_parse(&url)
                        .map(|v| v.to_string())
                        .map_err(|e| rquickjs::Error::new_from_js_message("string", "Error", &e))
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "urlFilename",
            Function::new(ctx.clone(), move |url: String| -> Option<String> {
                h.url_filename(&url)
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    // --- HTML helpers ---
    let h = host.clone();
    amigo
        .set(
            "__rawHtmlQueryAll",
            Function::new(
                ctx.clone(),
                move |html: String, selector: String| -> rquickjs::Result<String> {
                    h.html_query_all(&html, &selector)
                        .map(|v| serde_json::to_string(&v).unwrap_or_else(|_| "[]".into()))
                        .map_err(|e| rquickjs::Error::new_from_js_message("string", "Error", &e))
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "htmlQueryText",
            Function::new(
                ctx.clone(),
                move |html: String, selector: String| -> rquickjs::Result<Option<String>> {
                    h.html_query_text(&html, &selector)
                        .map_err(|e| rquickjs::Error::new_from_js_message("string", "Error", &e))
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "htmlQueryAttr",
            Function::new(
                ctx.clone(),
                move |html: String,
                      selector: String,
                      attr: String|
                      -> rquickjs::Result<Option<String>> {
                    h.html_query_attr(&html, &selector, &attr)
                        .map_err(|e| rquickjs::Error::new_from_js_message("string", "Error", &e))
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "__rawHtmlSearchMeta",
            Function::new(
                ctx.clone(),
                move |html: String, names_json: String| -> Option<String> {
                    let names: Vec<String> = serde_json::from_str(&names_json).ok()?;
                    h.html_search_meta(&html, &names)
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "htmlExtractTitle",
            Function::new(ctx.clone(), move |html: String| -> Option<String> {
                h.html_extract_title(&html)
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "__rawHtmlHiddenInputs",
            Function::new(
                ctx.clone(),
                move |html: String| -> rquickjs::Result<String> {
                    let inputs = h.html_hidden_inputs(&html);
                    Ok(serde_json::to_string(&inputs).unwrap_or_else(|_| "{}".into()))
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "__rawSearchJson",
            Function::new(
                ctx.clone(),
                move |start_pattern: String, html: String| -> Option<String> {
                    h.search_json(&start_pattern, &html)
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    // --- Additional regex helpers ---
    let h = host.clone();
    amigo
        .set(
            "regexReplace",
            Function::new(
                ctx.clone(),
                move |pattern: String, text: String, replacement: String| -> Option<String> {
                    h.regex_replace(&pattern, &text, &replacement)
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "regexTest",
            Function::new(ctx.clone(), move |pattern: String, text: String| -> bool {
                h.regex_test(&pattern, &text)
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "__rawRegexSplit",
            Function::new(
                ctx.clone(),
                move |pattern: String, text: String| -> String {
                    serde_json::to_string(&h.regex_split(&pattern, &text))
                        .unwrap_or_else(|_| "[]".into())
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    // --- Utility helpers ---
    let h = host.clone();
    amigo
        .set(
            "parseDuration",
            Function::new(ctx.clone(), move |input: String| -> Option<f64> {
                h.parse_duration(&input)
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "sanitizeFilename",
            Function::new(ctx.clone(), move |name: String| -> String {
                h.sanitize_filename(&name)
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    // --- Notifications ---
    let pid = plugin_id.to_string();
    let h = host.clone();
    amigo
        .set(
            "notify",
            Function::new(ctx.clone(), move |title: String, message: String| {
                h.notify(&pid, &title, &message);
            }),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    // --- New HTTP extensions ---
    let h = host.clone();
    amigo
        .set(
            "__rawHttpPostForm",
            Function::new(
                ctx.clone(),
                move |url: String,
                      fields_json: String,
                      headers_json: Option<String>|
                      -> rquickjs::Result<String> {
                    let h = h.clone();
                    let fields: HashMap<String, String> = serde_json::from_str(&fields_json)
                        .map_err(|e| {
                            rquickjs::Error::new_from_js_message(
                                "string",
                                "Error",
                                &format!("Invalid fields JSON: {e}"),
                            )
                        })?;
                    let headers: Option<HashMap<String, String>> = headers_json
                        .as_deref()
                        .and_then(|s| serde_json::from_str(s).ok());
                    let rt = tokio::runtime::Handle::current();
                    let result = tokio::task::block_in_place(|| {
                        rt.block_on(async { h.http_post_form(&url, &fields, headers).await })
                    });
                    match result {
                        Ok((status, body, resp_headers)) => {
                            let resp = serde_json::json!({
                                "status": status,
                                "body": body,
                                "headers": resp_headers,
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
            "__rawHttpGetBinary",
            Function::new(
                ctx.clone(),
                move |url: String, headers_json: Option<String>| -> rquickjs::Result<String> {
                    let h = h.clone();
                    let headers: Option<HashMap<String, String>> = headers_json
                        .as_deref()
                        .and_then(|s| serde_json::from_str(s).ok());
                    let rt = tokio::runtime::Handle::current();
                    let result = tokio::task::block_in_place(|| {
                        rt.block_on(async { h.http_get_binary(&url, headers).await })
                    });
                    result.map_err(|e| rquickjs::Error::new_from_js_message("string", "Error", &e))
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "__rawHttpFollowRedirects",
            Function::new(
                ctx.clone(),
                move |url: String, headers_json: Option<String>| -> rquickjs::Result<String> {
                    let h = h.clone();
                    let headers: Option<HashMap<String, String>> = headers_json
                        .as_deref()
                        .and_then(|s| serde_json::from_str(s).ok());
                    let rt = tokio::runtime::Handle::current();
                    let result = tokio::task::block_in_place(|| {
                        rt.block_on(async { h.http_follow_redirects(&url, headers).await })
                    });
                    result.map_err(|e| rquickjs::Error::new_from_js_message("string", "Error", &e))
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    let h = host.clone();
    amigo
        .set(
            "__rawHtmlQueryAllAttrs",
            Function::new(
                ctx.clone(),
                move |html: String, selector: String, attr: String| -> rquickjs::Result<String> {
                    h.html_query_all_attrs(&html, &selector, &attr)
                        .map(|v| serde_json::to_string(&v).unwrap_or_else(|_| "[]".into()))
                        .map_err(|e| rquickjs::Error::new_from_js_message("string", "Error", &e))
                },
            ),
        )
        .map_err(|e| crate::Error::Execution(e.to_string()))?;

    // --- Captcha ---
    let pid = plugin_id.to_string();
    let h = host.clone();
    amigo
        .set(
            "__rawSolveCaptcha",
            Function::new(
                ctx.clone(),
                move |image_url: String,
                      captcha_type: Option<String>|
                      -> rquickjs::Result<String> {
                    let h = h.clone();
                    let pid = pid.clone();
                    let ct = captcha_type.unwrap_or_else(|| "image".to_string());
                    let rt = tokio::runtime::Handle::current();
                    let result = tokio::task::block_in_place(|| {
                        rt.block_on(async { h.solve_captcha(&pid, "", &image_url, &ct).await })
                    });
                    result.map_err(|e| rquickjs::Error::new_from_js_message("string", "Error", &e))
                },
            ),
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

    // Inject JS shim layer: wraps __raw* functions to return objects instead of JSON strings.
    // This keeps the Rust→JS boundary simple (string returns) while giving plugins a clean API.
    ctx.eval::<JsValue<'_>, _>(JS_SHIM)
        .map_err(|e| crate::Error::Execution(format!("Failed to inject JS shim: {e}")))?;

    Ok(())
}

// --- Base64 helpers ---

fn base64_decode_bytes(input: &str) -> Result<Vec<u8>, String> {
    enforce_input_limit("base64 input", input.len(), MAX_BASE64_INPUT_BYTES)?;
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

/// Reject IPs that plugins must not be able to reach: loopback, RFC1918
/// private, link-local (169.254.0.0/16 — covers cloud-metadata services),
/// CGNAT (100.64.0.0/10), broadcast, unspecified, documentation, and the
/// IPv6 equivalents plus IPv4-mapped IPv6.
fn is_blocked_ip(ip: std::net::IpAddr) -> bool {
    use std::net::IpAddr;
    match ip {
        IpAddr::V4(v4) => {
            if v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.is_documentation()
            {
                return true;
            }
            let o = v4.octets();
            // CGNAT 100.64.0.0/10 is treated as non-public.
            o[0] == 100 && (64..=127).contains(&o[1])
        }
        IpAddr::V6(v6) => {
            if v6.is_loopback() || v6.is_unspecified() {
                return true;
            }
            let segs = v6.segments();
            // Unique-local fc00::/7
            if (segs[0] & 0xfe00) == 0xfc00 {
                return true;
            }
            // Link-local fe80::/10
            if (segs[0] & 0xffc0) == 0xfe80 {
                return true;
            }
            // Walk IPv4-mapped addresses through the v4 check.
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_blocked_ip(IpAddr::V4(v4));
            }
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_ip_v4_coverage() {
        use std::net::{IpAddr, Ipv4Addr};
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        // Cloud metadata endpoint.
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254))));
        // CGNAT
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1))));
        assert!(is_blocked_ip(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))));
        // Public addresses are allowed.
        assert!(!is_blocked_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_blocked_ip(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
    }

    #[test]
    fn blocked_ip_v6_coverage() {
        use std::net::IpAddr;
        assert!(is_blocked_ip("::1".parse::<IpAddr>().unwrap()));
        assert!(is_blocked_ip("::".parse::<IpAddr>().unwrap()));
        assert!(is_blocked_ip("fe80::1".parse::<IpAddr>().unwrap()));
        assert!(is_blocked_ip("fc00::1".parse::<IpAddr>().unwrap()));
        assert!(is_blocked_ip("fd00::1".parse::<IpAddr>().unwrap()));
        // IPv4-mapped loopback
        assert!(is_blocked_ip("::ffff:127.0.0.1".parse::<IpAddr>().unwrap()));
        // Public v6 is allowed.
        assert!(!is_blocked_ip(
            "2606:4700:4700::1111".parse::<IpAddr>().unwrap()
        ));
    }

    #[tokio::test]
    async fn ssrf_rejects_loopback_literal() {
        let api = HostApi::new(20);
        let err = api
            .check_url_allowed("http://127.0.0.1:22/")
            .await
            .unwrap_err();
        assert!(err.contains("private/loopback"), "{err}");
    }

    #[tokio::test]
    async fn ssrf_rejects_metadata_endpoint() {
        let api = HostApi::new(20);
        let err = api
            .check_url_allowed("http://169.254.169.254/latest/meta-data/")
            .await
            .unwrap_err();
        assert!(err.contains("private/loopback"), "{err}");
    }

    #[tokio::test]
    async fn ssrf_rejects_non_http_scheme() {
        let api = HostApi::new(20);
        let err = api
            .check_url_allowed("file:///etc/passwd")
            .await
            .unwrap_err();
        assert!(err.contains("scheme not allowed"), "{err}");
    }

    #[tokio::test]
    async fn ssrf_allows_when_flag_set() {
        let mut api = HostApi::new(20);
        api.set_allow_private_network(true);
        api.check_url_allowed("http://127.0.0.1:22/").await.unwrap();
    }

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
        api.set_cookie("plugin-x", "example.com", "session", "abc123")
            .await;
        assert_eq!(
            api.get_cookie("plugin-x", "example.com", "session").await,
            Some("abc123".to_string())
        );
        api.clear_cookies("plugin-x", "example.com").await;
        assert_eq!(
            api.get_cookie("plugin-x", "example.com", "session").await,
            None
        );
    }

    #[tokio::test]
    async fn cookies_isolated_between_plugins() {
        // The previous shared-jar implementation let plugin B read a
        // cookie that plugin A had stashed under the same domain. Each
        // plugin must now see only its own cookies.
        let api = HostApi::new(20);
        api.set_cookie("real-debrid", "real-debrid.com", "auth", "secret-A")
            .await;
        api.set_cookie("attacker", "real-debrid.com", "auth", "secret-B")
            .await;
        assert_eq!(
            api.get_cookie("real-debrid", "real-debrid.com", "auth")
                .await,
            Some("secret-A".to_string())
        );
        assert_eq!(
            api.get_cookie("attacker", "real-debrid.com", "auth").await,
            Some("secret-B".to_string())
        );
        // Clearing one plugin's jar must not touch the other's.
        api.clear_cookies("attacker", "real-debrid.com").await;
        assert_eq!(
            api.get_cookie("attacker", "real-debrid.com", "auth").await,
            None
        );
        assert_eq!(
            api.get_cookie("real-debrid", "real-debrid.com", "auth")
                .await,
            Some("secret-A".to_string())
        );
    }

    #[tokio::test]
    async fn test_storage() {
        let api = HostApi::new(20);
        api.storage_set("test-plugin", "key1", "value1")
            .await
            .expect("write within quota must succeed");
        assert_eq!(
            api.storage_get("test-plugin", "key1").await,
            Some("value1".to_string())
        );
        api.storage_delete("test-plugin", "key1").await;
        assert_eq!(api.storage_get("test-plugin", "key1").await, None);
    }

    #[tokio::test]
    async fn storage_quota_rejects_oversized_write() {
        // 1 KiB quota — we should be able to set a value just under the
        // limit, then fail to grow past it.
        let limits = crate::sandbox::SandboxLimits {
            max_storage_bytes: 1024,
            ..Default::default()
        };
        let api = HostApi::from_sandbox(&limits);

        // Fill with ~900 bytes (key+value), still under quota.
        let big = "x".repeat(900);
        api.storage_set("p", "big", &big)
            .await
            .expect("first write fits");

        // Try to push the total above 1 KiB with a 200-byte value.
        let err = api
            .storage_set("p", "extra", &"y".repeat(200))
            .await
            .expect_err("second write must exceed quota");
        assert!(err.contains("storage quota exceeded"), "msg: {err}");

        // The rejected write must not have mutated state.
        assert_eq!(api.storage_get("p", "extra").await, None);
        assert_eq!(api.storage_get("p", "big").await, Some(big));
    }

    #[tokio::test]
    async fn storage_quota_replacing_existing_key_uses_delta() {
        // Replacing an existing key must charge only the *delta*, not the
        // full new value, otherwise plugins can't shrink an oversized blob
        // even when freeing space is the goal.
        let limits = crate::sandbox::SandboxLimits {
            max_storage_bytes: 1024,
            ..Default::default()
        };
        let api = HostApi::from_sandbox(&limits);
        api.storage_set("p", "k", &"a".repeat(900))
            .await
            .expect("initial 900-byte write fits");
        // Overwriting with the same size must succeed.
        api.storage_set("p", "k", &"b".repeat(900))
            .await
            .expect("same-size overwrite must succeed");
        // Shrinking is also fine.
        api.storage_set("p", "k", &"c".repeat(10))
            .await
            .expect("shrinking write must succeed");
    }

    #[tokio::test]
    async fn storage_quota_isolated_per_plugin() {
        let limits = crate::sandbox::SandboxLimits {
            max_storage_bytes: 1024,
            ..Default::default()
        };
        let api = HostApi::from_sandbox(&limits);
        api.storage_set("plugin-a", "k", &"a".repeat(900))
            .await
            .expect("a fits");
        // Plugin B has its own quota — must still be able to write.
        api.storage_set("plugin-b", "k", &"b".repeat(900))
            .await
            .expect("b has its own quota");
    }

    #[test]
    fn test_md5() {
        let api = HostApi::new(20);
        assert_eq!(api.md5("hello"), "5d41402abc4b2a76b9719d911017c592");
        assert_eq!(api.md5(""), "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn test_sha1() {
        let api = HostApi::new(20);
        assert_eq!(
            api.sha1("hello"),
            "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d"
        );
    }

    #[test]
    fn test_sha256() {
        let api = HostApi::new(20);
        assert_eq!(
            api.sha256("hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_hmac_sha256() {
        let api = HostApi::new(20);
        let result = api.hmac_sha256("key", "hello").unwrap();
        assert_eq!(
            result,
            "9307b3b915efb5171ff14d8cb55fbcc798c6c0ef1456d66ded1a6aa723a58b7b"
        );
    }

    #[test]
    fn test_aes_cbc_roundtrip() {
        let api = HostApi::new(20);
        let key = "00112233445566778899aabbccddeeff";
        let iv = "00112233445566778899aabbccddeeff";
        let plaintext_b64 = api.base64_encode("Hello AES-CBC!");

        let encrypted = api.aes_encrypt_cbc(&plaintext_b64, key, iv).unwrap();
        let decrypted_b64 = api.aes_decrypt_cbc(&encrypted, key, iv).unwrap();
        let decrypted = api.base64_decode(&decrypted_b64).unwrap();

        assert_eq!(decrypted, "Hello AES-CBC!");
    }

    #[test]
    fn test_sanitize_filename() {
        let api = HostApi::new(20);
        assert_eq!(
            api.sanitize_filename("My Video: \"Best\" <2024>"),
            "My Video_ _Best_ _2024_"
        );
        assert_eq!(api.sanitize_filename("..."), "download");
        assert_eq!(api.sanitize_filename("normal.txt"), "normal.txt");
    }

    #[test]
    fn test_parse_duration() {
        let api = HostApi::new(20);
        assert_eq!(api.parse_duration("1:23:45"), Some(5025.0));
        assert_eq!(api.parse_duration("12:34"), Some(754.0));
        assert_eq!(api.parse_duration("PT1H23M45S"), Some(5025.0));
        assert_eq!(api.parse_duration("PT2H30M"), Some(9000.0));
    }

    #[test]
    fn test_url_helpers() {
        let api = HostApi::new(20);
        assert_eq!(
            api.url_resolve("https://example.com/page/", "../file.zip")
                .unwrap(),
            "https://example.com/file.zip"
        );
        assert_eq!(
            api.url_filename("https://cdn.example.com/files/doc%20v2.pdf?token=abc"),
            Some("doc v2.pdf".to_string())
        );
    }

    #[test]
    fn test_html_helpers() {
        let api = HostApi::new(20);
        let html = r#"<html><head><title>Test Page</title>
            <meta property="og:title" content="OG Title">
            </head><body><a href="/file.zip">Download</a></body></html>"#;

        assert_eq!(api.html_extract_title(html), Some("Test Page".to_string()));
        assert_eq!(
            api.html_search_meta(html, &["og:title".to_string()]),
            Some("OG Title".to_string())
        );
        assert_eq!(
            api.html_query_attr(html, "a", "href").unwrap(),
            Some("/file.zip".to_string())
        );
    }

    // --- Resource-limit regression tests (audit #11) ---

    #[test]
    fn regex_helpers_reject_oversize_text() {
        let api = HostApi::new(20);
        let huge = "a".repeat(MAX_REGEX_TEXT_BYTES + 1);
        assert_eq!(huge.len(), MAX_REGEX_TEXT_BYTES + 1, "sanity");
        // Match returns None instead of grinding through gigabytes.
        assert!(api.regex_match(r"a+", &huge).is_none());
        // match_all degrades gracefully to an empty result.
        assert!(api.regex_match_all(r"a+", &huge).is_empty());
        // replace returns None when the text is oversized. Arg order is
        // regex_replace(pattern, text, replacement) — putting the huge
        // string in the replacement slot is a different test.
        let replaced = api.regex_replace(r"a", &huge, "b");
        assert!(
            replaced.is_none(),
            "regex_replace must short-circuit on oversize input"
        );
        // test returns false.
        assert!(!api.regex_test(r"a", &huge));
        // split returns the input untouched.
        let split = api.regex_split(r"a", &huge);
        assert_eq!(split.len(), 1);
        assert_eq!(split[0].len(), huge.len());
    }

    #[test]
    fn regex_helpers_reject_oversize_pattern() {
        // A pathologically long pattern is rejected at compile time so we
        // never even try to run it.
        let api = HostApi::new(20);
        let pattern = "a".repeat(MAX_REGEX_PATTERN_BYTES + 1);
        assert_eq!(api.regex_match(&pattern, "aaaa"), None);
    }

    #[test]
    fn html_helpers_reject_oversize_input() {
        let api = HostApi::new(20);
        let huge = "<p>".repeat(MAX_HTML_BYTES); // > 8 MiB
        assert!(api.html_query_all(&huge, "p").is_err());
        assert!(api.html_query_text(&huge, "p").is_err());
        assert!(api.html_query_attr(&huge, "p", "x").is_err());
        assert_eq!(api.html_extract_title(&huge), None);
        assert!(api.html_hidden_inputs(&huge).is_empty());
    }

    #[test]
    fn base64_decode_rejects_oversize_input() {
        let api = HostApi::new(20);
        let huge = "A".repeat(MAX_BASE64_INPUT_BYTES + 1);
        let err = api.base64_decode(&huge).expect_err("must reject");
        assert!(err.contains("base64 input too large"), "msg: {err}");
    }
}
