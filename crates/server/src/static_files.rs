//! Embedded Web-UI static assets via rust-embed.
//!
//! In release builds, the web-ui dist/ folder is compiled into the binary.
//! In dev, it falls through to a 404 (use vite dev server with proxy instead).

use axum::{
    Router,
    body::Body,
    extract::Request,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../web-ui/dist/"]
#[prefix = ""]
struct WebUiAssets;

pub fn static_router() -> Router {
    Router::new().fallback(get(serve_static))
}

async fn serve_static(req: Request) -> Response {
    let path = req.uri().path().trim_start_matches('/');

    // Try exact file match first
    if let Some(content) = WebUiAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, mime.as_ref().to_string()),
                (header::CACHE_CONTROL, cache_control(path).to_string()),
            ],
            content.data.to_vec(),
        )
            .into_response();
    }

    // SPA fallback: serve index.html for all non-file paths
    if !path.contains('.') || path.is_empty() {
        if let Some(index) = WebUiAssets::get("index.html") {
            return (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, "text/html".to_string()),
                    (header::CACHE_CONTROL, "no-cache".to_string()),
                ],
                index.data.to_vec(),
            )
                .into_response();
        }
    }

    StatusCode::NOT_FOUND.into_response()
}

/// Aggressive caching for hashed assets, no-cache for HTML/SW.
fn cache_control(path: &str) -> &'static str {
    if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else if path == "sw.js" || path == "manifest.json" {
        "no-cache"
    } else {
        "public, max-age=3600"
    }
}
