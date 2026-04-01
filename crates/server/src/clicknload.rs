//! Click'n'Load listener on port 9666.
//!
//! Implements the Click'n'Load v2 protocol used by browser extensions
//! to send download links to the download manager.
//! Compatible with JDownloader, pyLoad, and other tools.

use std::sync::Arc;

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use tracing::info;

use amigo_core::coordinator::Coordinator;

/// Start the Click'n'Load listener on port 9666.
pub async fn start_clicknload(coordinator: Arc<Coordinator>) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/jdcheck.js", get(jdcheck))
        .route("/flash/add", post(flash_add))
        .route("/flash/addcrypted2", post(flash_addcrypted2))
        .with_state(coordinator);

    let bind = "0.0.0.0:9666";
    info!("Click'n'Load listener on {bind}");

    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Browser extensions check this endpoint to see if a download manager is running.
async fn jdcheck() -> &'static str {
    "jdownloader=true;"
}

/// Receive plain-text links (one per line).
async fn flash_add(
    State(coord): State<Arc<Coordinator>>,
    body: String,
) -> Result<StatusCode, StatusCode> {
    let urls: Vec<&str> = body
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    info!("Click'n'Load: received {} links", urls.len());

    for url in urls {
        if let Err(e) = coord.add_download(url, None).await {
            tracing::warn!("Click'n'Load: failed to add {url}: {e}");
        }
    }

    Ok(StatusCode::OK)
}

/// Receive encrypted links (DLC-style). The `crypted` field contains Base64 data.
async fn flash_addcrypted2(
    State(coord): State<Arc<Coordinator>>,
    body: String,
) -> Result<StatusCode, StatusCode> {
    // Parse form data: crypted=...&jk=...&passwords=...
    let mut crypted = None;
    for pair in body.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or("");
        let value = parts.next().unwrap_or("");
        if key == "crypted" {
            crypted = Some(value.to_string());
        }
    }

    if let Some(data) = crypted {
        // Try to parse as DLC
        let decoded = urlencoding_decode(&data);
        match amigo_core::container::import_dlc(decoded.as_bytes()) {
            Ok(packages) => {
                let link_count: usize = packages.iter().map(|p| p.links.len()).sum();
                info!("Click'n'Load: DLC with {} links", link_count);
                for pkg in &packages {
                    for link in &pkg.links {
                        let _ = coord.add_download(&link.url, link.filename.clone()).await;
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Click'n'Load: failed to parse encrypted data: {e}");
                // Fallback: treat as plain URL list
                for line in decoded.lines() {
                    let url = line.trim();
                    if !url.is_empty() {
                        let _ = coord.add_download(url, None).await;
                    }
                }
            }
        }
    }

    Ok(StatusCode::OK)
}

/// Simple URL decoding (percent-encoded).
fn urlencoding_decode(input: &str) -> String {
    let mut bytes = Vec::with_capacity(input.len());
    let mut chars = input.chars();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                bytes.push(byte);
            }
        } else if c == '+' {
            bytes.push(b' ');
        } else {
            // Safe for ASCII; multi-byte chars get their UTF-8 bytes
            let mut buf = [0u8; 4];
            bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        }
    }

    String::from_utf8(bytes).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}
