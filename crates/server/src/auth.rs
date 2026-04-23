//! Authentication middleware + credential helpers for the REST API.
//!
//! Three credential shapes are accepted on protected routes:
//!
//! 1. **Session cookie** (`amigo_session=<id>`) — issued by `POST /login`
//!    after a successful username/password check. Validated against the
//!    `sessions` table; expired sessions are rejected.
//! 2. **API token** (`Authorization: Bearer <token>`) — issued to CLI /
//!    script clients by the pairing flow. The plaintext is hashed with
//!    SHA-256 (fast, deterministic) and looked up in `api_tokens`.
//! 3. **Pre-shared token** (`Authorization: Bearer <cfg.api_token>`) —
//!    legacy config escape hatch that pre-dates the wizard flow.
//!
//! For WebSocket handshakes (where browsers cannot set `Authorization`)
//! we additionally accept `?token=<token>` as a query parameter.
//!
//! A separate layer, [`setup_guard`], intercepts requests while the wizard
//! has not completed and `bind` is non-loopback, returning 503 for any path
//! outside `/api/v1/setup/*`.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, Uri, header},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use sha2::{Digest, Sha256};

use crate::api::AppState;

/// Shared auth configuration.
#[derive(Clone)]
pub struct AuthState {
    /// App state handle — used to look up sessions, API tokens, and the
    /// current `setup_complete` flag from config.
    pub app: AppState,
    /// Optional pre-shared bearer token from `server.api_token`.
    pub preshared_token: Option<Arc<String>>,
    /// Optional setup-mode PIN (from `AMIGO_SETUP_PIN`). Only consulted by
    /// [`require_setup_pin`].
    pub setup_pin: Option<Arc<String>>,
    /// `true` if the server bind is loopback; setup-mode is suppressed there.
    /// This is captured at startup because the bind cannot change at runtime.
    pub bind_is_loopback: bool,
}

impl AuthState {
    pub fn new(
        app: AppState,
        preshared_token: Option<String>,
        setup_pin: Option<String>,
        _setup_complete_initial: bool,
        bind_is_loopback: bool,
    ) -> Self {
        Self {
            app,
            preshared_token: preshared_token.filter(|t| !t.is_empty()).map(Arc::new),
            setup_pin: setup_pin.filter(|t| !t.is_empty()).map(Arc::new),
            bind_is_loopback,
        }
    }

    /// Current value of `setup_complete` — read through the coordinator so
    /// the setup wizard flipping it takes effect in-flight.
    pub async fn setup_complete(&self) -> bool {
        self.app.coordinator.config().await.server.setup_complete
    }
}

/// Cookie name used for browser session tokens.
pub const SESSION_COOKIE: &str = "amigo_session";

/// Return the effective client IP as a string. When `trust_proxy` is true,
/// the first hop from `X-Forwarded-For` (or the RFC 7239 `Forwarded: for=`
/// clause) wins; otherwise we fall back to the direct TCP peer. Untrusted
/// forwarded headers are ignored to prevent source-IP spoofing.
pub fn client_ip(
    headers: &HeaderMap,
    peer: Option<std::net::SocketAddr>,
    trust_proxy: bool,
) -> String {
    if trust_proxy {
        if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            if let Some(first) = xff.split(',').next() {
                let t = first.trim();
                if !t.is_empty() {
                    return t.to_string();
                }
            }
        }
        if let Some(fwd) = headers.get("forwarded").and_then(|v| v.to_str().ok()) {
            for part in fwd.split(';').flat_map(|s| s.split(',')) {
                if let Some(v) = part.trim().strip_prefix("for=") {
                    let t = v.trim_matches('"').trim_start_matches("[").trim_end_matches("]");
                    if !t.is_empty() {
                        return t.to_string();
                    }
                }
            }
        }
    }
    peer.map(|s| s.ip().to_string()).unwrap_or_else(|| "unknown".into())
}

/// Whether the original request came in over HTTPS. When `trust_proxy` is
/// true, honours `X-Forwarded-Proto`; otherwise conservatively returns
/// `false` (the listener itself is plain HTTP).
pub fn request_is_secure(headers: &HeaderMap, trust_proxy: bool) -> bool {
    if !trust_proxy {
        return false;
    }
    headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.eq_ignore_ascii_case("https"))
        .unwrap_or(false)
}

/// Hash a plaintext bearer token for API-token lookups. SHA-256 is fast and
/// deterministic; we never store plaintext, so a DB leak only gives out hashes.
/// Argon2 is reserved for password hashing where the input is low-entropy.
pub fn hash_api_token(plain: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(plain.as_bytes());
    hex::encode(hasher.finalize())
}

/// Constant-time byte comparison.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn extract_bearer<'a>(headers: &'a HeaderMap) -> Option<&'a str> {
    let s = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    s.strip_prefix("Bearer ")
}

fn extract_query_token(uri: &Uri) -> Option<&str> {
    uri.query()?
        .split('&')
        .find_map(|pair| pair.strip_prefix("token="))
}

fn extract_session_cookie<'a>(headers: &'a HeaderMap) -> Option<&'a str> {
    let s = headers.get(header::COOKIE)?.to_str().ok()?;
    for kv in s.split(';') {
        let kv = kv.trim();
        if let Some(v) = kv.strip_prefix(&format!("{SESSION_COOKIE}=")) {
            return Some(v);
        }
    }
    None
}

/// Identity returned by [`authenticate`] on success.
#[derive(Debug, Clone)]
pub enum Principal {
    /// Browser session — carries the username.
    Session { username: String, session_id: String },
    /// API-token holder (CLI / script).
    ApiToken { id: String, name: String },
    /// Legacy pre-shared token.
    Preshared,
}

/// Try every supported credential type. Returns `None` if nothing validates.
pub async fn authenticate(
    auth: &AuthState,
    headers: &HeaderMap,
    uri: &Uri,
) -> Option<Principal> {
    let now = chrono::Utc::now().timestamp();

    // Session cookie.
    if let Some(sid) = extract_session_cookie(headers) {
        if let Ok(Some(row)) = auth.app.coordinator.storage().get_session(sid).await {
            if row.expires_at > now {
                let _ = auth
                    .app
                    .coordinator
                    .storage()
                    .touch_session(sid, now)
                    .await;
                return Some(Principal::Session {
                    username: row.username,
                    session_id: row.id,
                });
            }
        }
    }

    // Bearer tokens — either pre-shared, or an API-token row.
    let bearer = extract_bearer(headers).or_else(|| extract_query_token(uri));
    if let Some(tok) = bearer {
        if let Some(preshared) = auth.preshared_token.as_ref() {
            if ct_eq(tok.as_bytes(), preshared.as_bytes()) {
                return Some(Principal::Preshared);
            }
        }
        let hash = hash_api_token(tok);
        if let Ok(Some(row)) = auth
            .app
            .coordinator
            .storage()
            .get_api_token_by_hash(&hash)
            .await
        {
            if row.expires_at.is_none_or(|exp| exp > now) {
                let _ = auth
                    .app
                    .coordinator
                    .storage()
                    .touch_api_token(&row.id, now, None)
                    .await;
                return Some(Principal::ApiToken {
                    id: row.id,
                    name: row.name,
                });
            }
        }
    }

    None
}

/// Middleware: reject requests that do not carry a valid session or bearer.
/// The authenticated [`Principal`] is stashed in the request extensions so
/// handlers can read it via `req.extensions().get::<Principal>()`.
pub async fn require_auth(
    State(auth): State<AuthState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let principal = authenticate(&auth, req.headers(), req.uri()).await;
    match principal {
        Some(p) => {
            req.extensions_mut().insert(p);
            Ok(next.run(req).await)
        }
        None => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Middleware that returns 503 + `{"error":"setup_required"}` until the
/// first-run wizard has marked `setup_complete = true`. Loopback binds skip
/// the guard entirely — Tauri desktop / local CLI never see a 503.
pub async fn setup_guard(
    State(auth): State<AuthState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if auth.bind_is_loopback || auth.setup_complete().await {
        return next.run(req).await;
    }
    // Allow the setup and pairing-poll endpoints through while in setup mode.
    let path = req.uri().path();
    if path.starts_with("/api/v1/setup")
        || path == "/api/v1/pairing/status"
        || !path.starts_with("/api/v1/")
    {
        return next.run(req).await;
    }
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({ "error": "setup_required" })),
    )
        .into_response()
}

/// Middleware for `/api/v1/setup/*`. If a setup PIN is configured, require
/// the client to present it as `X-Setup-Pin: <pin>`. Otherwise pass through.
/// Once setup is complete, every request is rejected with 410 Gone — the
/// wizard is one-shot.
pub async fn require_setup_pin(
    State(auth): State<AuthState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if auth.setup_complete().await {
        return Err(StatusCode::GONE);
    }
    let Some(expected) = auth.setup_pin.as_ref() else {
        // No PIN configured -> TOFU, anyone who gets here first claims admin.
        return Ok(next.run(req).await);
    };
    let provided = req
        .headers()
        .get("X-Setup-Pin")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    if ct_eq(provided.as_bytes(), expected.as_bytes()) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ct_eq_basic() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"abcd"));
        assert!(ct_eq(b"", b""));
    }

    #[test]
    fn api_token_hash_is_deterministic_sha256_hex() {
        let hash = hash_api_token("hello");
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_eq!(hash_api_token("hello"), hash);
        assert_ne!(hash_api_token("hello"), hash_api_token("Hello"));
    }

    #[test]
    fn extract_session_cookie_picks_correct_entry() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            "other=1; amigo_session=abc; last=z".parse().unwrap(),
        );
        assert_eq!(extract_session_cookie(&headers), Some("abc"));
    }

    #[test]
    fn extract_query_token_from_uri() {
        let uri: Uri = "/api/v1/ws?foo=bar&token=xyz".parse().unwrap();
        assert_eq!(extract_query_token(&uri), Some("xyz"));
        let uri: Uri = "/api/v1/ws".parse().unwrap();
        assert_eq!(extract_query_token(&uri), None);
    }

    #[test]
    fn client_ip_honours_trust_proxy() {
        use std::net::SocketAddr;
        let peer: SocketAddr = "10.0.0.1:4242".parse().unwrap();
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "203.0.113.4, 10.0.0.1".parse().unwrap());

        // Untrusted — proxy header is ignored.
        assert_eq!(client_ip(&h, Some(peer), false), "10.0.0.1");
        // Trusted — first hop wins.
        assert_eq!(client_ip(&h, Some(peer), true), "203.0.113.4");

        // Legacy RFC 7239 Forwarded header.
        let mut h2 = HeaderMap::new();
        h2.insert("forwarded", r#"for="198.51.100.7";proto=https"#.parse().unwrap());
        assert_eq!(client_ip(&h2, Some(peer), true), "198.51.100.7");
    }

    #[test]
    fn request_is_secure_respects_trust_proxy() {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-proto", "https".parse().unwrap());
        assert!(!request_is_secure(&h, false));
        assert!(request_is_secure(&h, true));
    }
}
