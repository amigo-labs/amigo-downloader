//! Bearer-token authentication middleware for the REST API and WebSocket.
//!
//! When `api_token` is set in `ServerConfig`, every request to a protected
//! router must carry `Authorization: Bearer <token>` (or, for WebSocket
//! upgrade handshakes, a `?token=<token>` query parameter).
//!
//! Constant-time comparison is used to defeat timing side channels.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, Uri},
    middleware::Next,
    response::Response,
};

/// Shared auth state — the configured token or `None` for unauthenticated.
#[derive(Clone)]
pub struct AuthState {
    token: Option<Arc<String>>,
}

impl AuthState {
    pub fn new(token: Option<String>) -> Self {
        Self {
            token: token.filter(|t| !t.is_empty()).map(Arc::new),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.token.is_some()
    }
}

/// Constant-time byte comparison.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn extract_token<'a>(headers: &'a HeaderMap, uri: &'a Uri) -> Option<&'a str> {
    if let Some(h) = headers.get(axum::http::header::AUTHORIZATION)
        && let Ok(s) = h.to_str()
        && let Some(t) = s.strip_prefix("Bearer ")
    {
        return Some(t);
    }
    // WebSocket handshake fallback: browsers can't set Authorization header on
    // `new WebSocket(...)`, so we accept the token as a query parameter.
    if let Some(q) = uri.query() {
        for pair in q.split('&') {
            if let Some(v) = pair.strip_prefix("token=") {
                return Some(v);
            }
        }
    }
    None
}

pub async fn require_token(
    State(auth): State<AuthState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let Some(expected) = auth.token.as_ref() else {
        // Auth disabled → pass through. Startup-time validation ensures this
        // only happens on loopback binds.
        return Ok(next.run(req).await);
    };

    let provided = extract_token(req.headers(), req.uri());
    match provided {
        Some(tok) if ct_eq(tok.as_bytes(), expected.as_bytes()) => Ok(next.run(req).await),
        _ => Err(StatusCode::UNAUTHORIZED),
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
    fn auth_disabled_when_empty() {
        assert!(!AuthState::new(None).is_enabled());
        assert!(!AuthState::new(Some(String::new())).is_enabled());
        assert!(AuthState::new(Some("x".into())).is_enabled());
    }
}
