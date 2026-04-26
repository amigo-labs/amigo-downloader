//! Click'n'Load listener on port 9666.
//!
//! Implements the Click'n'Load v2 protocol used by browser extensions
//! to send download links to the download manager.
//! Compatible with JDownloader, pyLoad, and other tools.
//!
//! Security: the listener binds to `127.0.0.1` only; external clients can't
//! reach it. Even so, every URL submitted via the protocol is run through
//! [`crate::net_guard::validate_outbound_url`] before queueing, so a
//! malicious local process / browser extension can't sneak in `file://`,
//! `gopher://`, RFC1918, or cloud-metadata targets. Bodies are also capped
//! (see `MAX_*_BYTES`) to stop DLC-bomb DoS.

use std::sync::Arc;

use axum::{
    Router,
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    routing::{get, post},
};
use tracing::{info, warn};

use amigo_core::coordinator::Coordinator;

/// Plain `flash/add` payload is one URL per line. 256 KiB is plenty for any
/// realistic batch and refuses obvious DoS payloads.
const MAX_FLASH_ADD_BYTES: usize = 256 * 1024;
/// Encrypted DLC bodies wrap a base64-encoded link list; legitimate ones are
/// kilobytes. 1 MiB matches the REST `/api/v1/downloads/container` limit.
const MAX_FLASH_ADDCRYPTED_BYTES: usize = 1024 * 1024;

/// Start the Click'n'Load listener on port 9666.
pub async fn start_clicknload(coordinator: Arc<Coordinator>) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/jdcheck.js", get(jdcheck))
        .route(
            "/flash/add",
            post(flash_add).layer(DefaultBodyLimit::max(MAX_FLASH_ADD_BYTES)),
        )
        .route(
            "/flash/addcrypted2",
            post(flash_addcrypted2)
                .layer(DefaultBodyLimit::max(MAX_FLASH_ADDCRYPTED_BYTES)),
        )
        .with_state(coordinator);

    // Bind explicitly to loopback. Anything else would expose the
    // unauthenticated download-injection endpoint to the LAN.
    let bind = "127.0.0.1:9666";
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
        // Even though the listener is loopback-only, a malicious browser
        // extension or local process could submit file://, gopher://, or
        // RFC1918 URLs and turn the queue into a probe. Filter through the
        // server's outbound URL guard.
        if let Err(e) = crate::net_guard::validate_outbound_url(url, false).await {
            warn!("Click'n'Load: rejecting URL {url}: {e}");
            continue;
        }
        if let Err(e) = coord.add_download(url, None).await {
            warn!("Click'n'Load: failed to add {url}: {e}");
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
                        if let Err(e) =
                            crate::net_guard::validate_outbound_url(&link.url, false).await
                        {
                            warn!("Click'n'Load: rejecting DLC URL {}: {e}", link.url);
                            continue;
                        }
                        let _ = coord.add_download(&link.url, link.filename.clone()).await;
                    }
                }
            }
            Err(e) => {
                warn!("Click'n'Load: failed to parse encrypted data: {e}");
                // Fallback: treat as plain URL list
                for line in decoded.lines() {
                    let url = line.trim();
                    if url.is_empty() {
                        continue;
                    }
                    if let Err(e) = crate::net_guard::validate_outbound_url(url, false).await {
                        warn!("Click'n'Load: rejecting fallback URL {url}: {e}");
                        continue;
                    }
                    let _ = coord.add_download(url, None).await;
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

#[cfg(test)]
mod tests {
    use crate::net_guard;

    #[tokio::test]
    async fn cnl_url_filter_rejects_dangerous_schemes() {
        // The flash_add handler runs each line through validate_outbound_url
        // before queueing. We exercise the same predicate directly so the
        // test does not need a full Coordinator + reqwest harness.
        for url in [
            "file:///etc/passwd",
            "gopher://attacker.example/x",
            "ftp://intranet/secret",
        ] {
            let err = net_guard::validate_outbound_url(url, false)
                .await
                .expect_err(&format!("scheme must be blocked: {url}"));
            assert!(
                matches!(err, net_guard::GuardError::BadScheme(_)),
                "{url} → {err}"
            );
        }
    }

    #[tokio::test]
    async fn cnl_url_filter_rejects_private_ips() {
        for url in [
            "http://127.0.0.1:22/",
            "http://10.0.0.1/",
            "http://169.254.169.254/latest/",
        ] {
            let err = net_guard::validate_outbound_url(url, false)
                .await
                .expect_err(&format!("private IP must be blocked: {url}"));
            assert!(
                matches!(err, net_guard::GuardError::BlockedAddress { .. }),
                "{url} → {err}"
            );
        }
    }

    #[tokio::test]
    async fn cnl_url_filter_accepts_public_https() {
        // example.com / www.example.com is reserved for documentation but
        // resolves to public IPs, so the guard should let it through.
        net_guard::validate_outbound_url("https://www.example.com/file.zip", false)
            .await
            .expect("public HTTPS must be accepted");
    }
}
