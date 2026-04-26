//! Security-headers middleware.
//!
//! Wraps the Axum response stack so every response carries the standard
//! defence-in-depth headers (clickjacking protection, MIME sniffing, referrer
//! policy, baseline CSP). Without these, a stored XSS in plugin metadata or
//! a misconfigured CDN can be exploited far more easily.
//!
//! The Strict-Transport-Security header is *not* set unconditionally — it
//! would break local HTTP setups. Operators behind a TLS-terminating proxy
//! can opt in by trusting `X-Forwarded-Proto: https` (see `config.server.trust_proxy`).

use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue, header},
    middleware::Next,
    response::Response,
};

/// `Content-Security-Policy` for the bundled web UI.
///
/// `connect-src` allows same-origin XHR, the in-process WebSocket, and any
/// HTTP/HTTPS URL the UI may need (the API surfaces external thumbnails,
/// captcha images, etc.). `style-src 'unsafe-inline'` is unfortunately
/// required by Svelte's scoped styles; the rest of the policy keeps script
/// execution to first-party resources only.
const DEFAULT_CSP: &str = "default-src 'self'; \
     script-src 'self'; \
     style-src 'self' 'unsafe-inline'; \
     img-src 'self' data: https:; \
     font-src 'self' data:; \
     connect-src 'self' ws: wss: http: https:; \
     frame-ancestors 'none'; \
     base-uri 'self'; \
     form-action 'self'";

pub async fn security_headers(req: Request, next: Next) -> Response {
    let mut resp = next.run(req).await;
    let headers = resp.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static("DENY"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("interest-cohort=(), browsing-topics=()"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(DEFAULT_CSP),
    );
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, routing::get};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn middleware_sets_baseline_headers() {
        let app = Router::new()
            .route("/x", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(security_headers));

        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/x")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let h = resp.headers().clone();
        assert_eq!(h["x-content-type-options"], "nosniff");
        assert_eq!(h["x-frame-options"], "DENY");
        assert_eq!(h["referrer-policy"], "no-referrer");
        assert!(h.get("content-security-policy").is_some());
        assert!(h.get("permissions-policy").is_some());
        // Drain body to keep clippy happy.
        let _ = resp.into_body().collect().await.unwrap();
    }
}
