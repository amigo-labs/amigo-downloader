//! Integration tests for the REST API endpoints.
//!
//! Tests exercise the Axum router through actual HTTP requests using
//! a real in-memory SQLite backend. Each test gets a fresh server instance.

use std::net::SocketAddr;

use amigo_core::config::Config;

/// Spawn a test server on a random port. Returns the bound address.
async fn spawn_test_server() -> SocketAddr {
    // Prevent auto-start of downloads during tests
    let config = Config {
        max_concurrent_downloads: 0,
        ..Config::default()
    };

    let state = amigo_server::build_test_state(config);
    let app = amigo_server::build_test_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind test server");
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    addr
}

/// Build a test HTTP client.
fn test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("Failed to build test client")
}

fn base_url(addr: SocketAddr) -> String {
    format!("http://{addr}")
}

// =============================================================================
// Status & Stats
// =============================================================================

#[tokio::test]
async fn test_status_returns_ok() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/status", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    assert!(body["version"].is_string());
}

#[tokio::test]
async fn test_stats_returns_zeroes_initially() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/stats", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["active_downloads"], 0);
    assert_eq!(body["speed_bytes_per_sec"], 0);
    assert_eq!(body["queued"], 0);
    assert_eq!(body["completed"], 0);
}

// =============================================================================
// Downloads CRUD
// =============================================================================

#[tokio::test]
async fn test_add_download_returns_created() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .post(format!("{}/api/v1/downloads", base_url(addr)))
        .json(&serde_json::json!({
            "url": "https://example.com/file.zip"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["id"].is_string());
    assert!(!body["id"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_add_download_with_filename() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .post(format!("{}/api/v1/downloads", base_url(addr)))
        .json(&serde_json::json!({
            "url": "https://example.com/dl?id=123",
            "filename": "custom.zip"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    let id = body["id"].as_str().unwrap();

    // Verify the filename was set
    let resp = client
        .get(format!("{}/api/v1/downloads/{}", base_url(addr), id))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let dl: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(dl["filename"], "custom.zip");
}

#[tokio::test]
async fn test_list_downloads_empty() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/downloads", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_list_downloads_after_add() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    // Add a download
    client
        .post(format!("{base}/api/v1/downloads"))
        .json(&serde_json::json!({"url": "https://example.com/file1.zip"}))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!("{base}/api/v1/downloads"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["url"], "https://example.com/file1.zip");
    assert_eq!(arr[0]["status"], "queued");
}

#[tokio::test]
async fn test_get_download_by_id() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    let resp = client
        .post(format!("{base}/api/v1/downloads"))
        .json(&serde_json::json!({"url": "https://example.com/file.zip"}))
        .send()
        .await
        .unwrap();
    let id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = client
        .get(format!("{base}/api/v1/downloads/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["id"], id);
    assert_eq!(body["url"], "https://example.com/file.zip");
    assert_eq!(body["protocol"], "http");
    assert_eq!(body["status"], "queued");
}

#[tokio::test]
async fn test_get_nonexistent_download_returns_404() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!(
            "{}/api/v1/downloads/nonexistent-id",
            base_url(addr)
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_patch_download_pause_and_resume() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    let resp = client
        .post(format!("{base}/api/v1/downloads"))
        .json(&serde_json::json!({"url": "https://example.com/file.zip"}))
        .send()
        .await
        .unwrap();
    let id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Pause
    let resp = client
        .patch(format!("{base}/api/v1/downloads/{id}"))
        .json(&serde_json::json!({"action": "pause"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify paused
    let resp = client
        .get(format!("{base}/api/v1/downloads/{id}"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "paused");

    // Resume
    let resp = client
        .patch(format!("{base}/api/v1/downloads/{id}"))
        .json(&serde_json::json!({"action": "resume"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify resumed -> queued
    let resp = client
        .get(format!("{base}/api/v1/downloads/{id}"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "queued");
}

#[tokio::test]
async fn test_patch_download_invalid_action() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    let resp = client
        .post(format!("{base}/api/v1/downloads"))
        .json(&serde_json::json!({"url": "https://example.com/file.zip"}))
        .send()
        .await
        .unwrap();
    let id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = client
        .patch(format!("{base}/api/v1/downloads/{id}"))
        .json(&serde_json::json!({"action": "invalid"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_delete_download() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    let resp = client
        .post(format!("{base}/api/v1/downloads"))
        .json(&serde_json::json!({"url": "https://example.com/file.zip"}))
        .send()
        .await
        .unwrap();
    let id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Delete
    let resp = client
        .delete(format!("{base}/api/v1/downloads/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // Verify gone
    let resp = client
        .get(format!("{base}/api/v1/downloads/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// =============================================================================
// Batch
// =============================================================================

#[tokio::test]
async fn test_add_batch_downloads() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    let resp = client
        .post(format!("{base}/api/v1/downloads/batch"))
        .json(&serde_json::json!({
            "urls": [
                "https://example.com/file1.zip",
                "https://example.com/file2.zip",
                "https://example.com/file3.zip"
            ]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.unwrap();
    let ids = body["ids"].as_array().unwrap();
    assert_eq!(ids.len(), 3);
    assert!(body["errors"].as_array().unwrap().is_empty());

    // Verify all appear in list
    let resp = client
        .get(format!("{base}/api/v1/downloads"))
        .send()
        .await
        .unwrap();
    let list: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(list.as_array().unwrap().len(), 3);
}

// =============================================================================
// Queue
// =============================================================================

#[tokio::test]
async fn test_queue_empty_initially() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/queue", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_queue_shows_queued_downloads() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    client
        .post(format!("{base}/api/v1/downloads"))
        .json(&serde_json::json!({"url": "https://example.com/file.zip"}))
        .send()
        .await
        .unwrap();

    let resp = client
        .get(format!("{base}/api/v1/queue"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_queue_reorder() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    // Add 3 downloads
    let mut ids = Vec::new();
    for i in 1..=3 {
        let resp = client
            .post(format!("{base}/api/v1/downloads"))
            .json(&serde_json::json!({"url": format!("https://example.com/file{i}.zip")}))
            .send()
            .await
            .unwrap();
        let body: serde_json::Value = resp.json().await.unwrap();
        ids.push(body["id"].as_str().unwrap().to_string());
    }

    // Reorder: reverse
    let reordered = vec![ids[2].clone(), ids[1].clone(), ids[0].clone()];
    let resp = client
        .patch(format!("{base}/api/v1/queue/reorder"))
        .json(&serde_json::json!({"ids": reordered}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify priorities changed
    let resp = client
        .get(format!("{base}/api/v1/downloads/{}", ids[2]))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let prio_first = body["priority"].as_i64().unwrap();

    let resp = client
        .get(format!("{base}/api/v1/downloads/{}", ids[0]))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let prio_last = body["priority"].as_i64().unwrap();

    // First in reorder list should have higher priority
    assert!(prio_first > prio_last);
}

// =============================================================================
// History
// =============================================================================

#[tokio::test]
async fn test_history_empty_initially() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/history", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_delete_history() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .delete(format!("{}/api/v1/history", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 204);
}

// =============================================================================
// Config
// =============================================================================

#[tokio::test]
async fn test_get_config() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/config", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["download_dir"].is_string());
    assert!(body["max_concurrent_downloads"].is_number());
    assert!(body["http"].is_object());
}

#[tokio::test]
async fn test_put_config_updates_values() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    // Get current config
    let resp = client
        .get(format!("{base}/api/v1/config"))
        .send()
        .await
        .unwrap();
    let mut config: serde_json::Value = resp.json().await.unwrap();

    // Modify a value
    config["max_concurrent_downloads"] = serde_json::json!(5);

    let resp = client
        .put(format!("{base}/api/v1/config"))
        .json(&config)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Verify the change persisted
    let resp = client
        .get(format!("{base}/api/v1/config"))
        .send()
        .await
        .unwrap();
    let updated: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(updated["max_concurrent_downloads"], 5);
}

// =============================================================================
// Plugins
// =============================================================================

#[tokio::test]
async fn test_list_plugins() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/plugins", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.is_array());
}

// =============================================================================
// Captcha
// =============================================================================

#[tokio::test]
async fn test_captcha_pending_empty() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/captcha/pending", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_array().unwrap().is_empty());
}

// =============================================================================
// Webhooks
// =============================================================================

#[tokio::test]
async fn test_webhook_crud_lifecycle() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    // List — empty
    let resp = client
        .get(format!("{base}/api/v1/webhooks"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_array().unwrap().is_empty());

    // Create
    let resp = client
        .post(format!("{base}/api/v1/webhooks"))
        .json(&serde_json::json!({
            "name": "test-hook",
            "url": "https://example.com/webhook"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let hook: serde_json::Value = resp.json().await.unwrap();
    let hook_id = hook["id"].as_str().unwrap().to_string();

    // List — should have one
    let resp = client
        .get(format!("{base}/api/v1/webhooks"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body.as_array().unwrap().len(), 1);

    // Delete
    let resp = client
        .delete(format!("{base}/api/v1/webhooks/{hook_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 204);

    // List — empty again
    let resp = client
        .get(format!("{base}/api/v1/webhooks"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_delete_nonexistent_webhook_returns_404() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .delete(format!("{}/api/v1/webhooks/nonexistent", base_url(addr)))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

// =============================================================================
// RSS (feature-gated, disabled by default)
// =============================================================================

#[tokio::test]
async fn test_rss_feeds_empty_when_disabled() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/rss", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_array().unwrap().is_empty());
}

// =============================================================================
// Usenet (feature-gated, disabled by default)
// =============================================================================

#[tokio::test]
async fn test_usenet_servers_empty_when_disabled() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .get(format!("{}/api/v1/usenet/servers", base_url(addr)))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.as_array().unwrap().is_empty());
}

// =============================================================================
// Response structure validation
// =============================================================================

#[tokio::test]
async fn test_download_response_has_expected_fields() {
    let addr = spawn_test_server().await;
    let client = test_client();
    let base = base_url(addr);

    let resp = client
        .post(format!("{base}/api/v1/downloads"))
        .json(&serde_json::json!({"url": "https://example.com/file.zip"}))
        .send()
        .await
        .unwrap();
    let id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = client
        .get(format!("{base}/api/v1/downloads/{id}"))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();

    // Verify all expected fields are present
    assert!(body.get("id").is_some());
    assert!(body.get("url").is_some());
    assert!(body.get("protocol").is_some());
    assert!(body.get("status").is_some());
    assert!(body.get("priority").is_some());
    assert!(body.get("bytes_downloaded").is_some());
    assert!(body.get("speed").is_some());
    assert!(body.get("created_at").is_some());
}

// =============================================================================
// Error handling — malformed requests
// =============================================================================

#[tokio::test]
async fn test_add_download_without_url_returns_error() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .post(format!("{}/api/v1/downloads", base_url(addr)))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();

    // Should be 4xx (likely 422 from Axum deserialization failure)
    assert!(resp.status().is_client_error());
}

#[tokio::test]
async fn test_add_download_with_invalid_json_returns_error() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .post(format!("{}/api/v1/downloads", base_url(addr)))
        .header("content-type", "application/json")
        .body("not valid json")
        .send()
        .await
        .unwrap();

    assert!(resp.status().is_client_error());
}

// =============================================================================
// Body-size limits (audit finding #3 — DoS via unbounded uploads)
// =============================================================================

#[tokio::test]
async fn dlc_upload_rejects_oversize_body() {
    let addr = spawn_test_server().await;
    let client = test_client();

    // 2 MiB synthetic payload — over the 1 MiB DLC limit. We hand-roll the
    // multipart body so the test depends only on the JSON-enabled client
    // used elsewhere. The body-size guard triggers from DefaultBodyLimit
    // before the multipart parser even runs, so the exact framing does not
    // matter as long as the wire size exceeds the limit.
    let payload = vec![b'A'; 2 * 1024 * 1024];
    let boundary = "----amigotest";
    let mut body: Vec<u8> = Vec::with_capacity(payload.len() + 256);
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"evil.dlc\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(&payload);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let resp = client
        .post(format!("{}/api/v1/downloads/container", base_url(addr)))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body)
        .send()
        .await
        .unwrap();

    // The body-size guard rejects the request before the handler ever
    // sees it. The exact status varies (axum's DefaultBodyLimit returns
    // 413 for raw bodies, the Multipart extractor surfaces it as 400),
    // but it must not be a 2xx — the must-not is that an oversize body
    // gets buffered into the handler.
    let status = resp.status();
    assert!(
        status.is_client_error(),
        "expected 4xx for oversize DLC, got {status}"
    );
    assert!(
        matches!(status.as_u16(), 400 | 413),
        "expected 400 or 413 for oversize DLC, got {status}"
    );
}

#[tokio::test]
async fn nzb_upload_rejects_oversize_body() {
    let addr = spawn_test_server().await;
    let client = test_client();

    // 65 MiB JSON body — over the 64 MiB NZB limit. We stuff the bytes into
    // the nzb_data field as a long string.
    let big_string = "X".repeat(65 * 1024 * 1024);
    let body = serde_json::json!({ "nzb_data": big_string });

    let resp = client
        .post(format!("{}/api/v1/downloads/nzb", base_url(addr)))
        .json(&body)
        .send()
        .await
        .unwrap();

    // Either 413 Payload Too Large from DefaultBodyLimit, or a 4xx if the
    // server rejects the connection earlier — anything in the client-error
    // range is acceptable; the must-not is "200 OK".
    assert!(
        resp.status().is_client_error(),
        "expected 4xx, got {}",
        resp.status()
    );
}

// =============================================================================
// SSRF guard (audit finding #7)
// =============================================================================

#[tokio::test]
async fn create_webhook_rejects_private_ip_url() {
    let addr = spawn_test_server().await;
    let client = test_client();

    let resp = client
        .post(format!("{}/api/v1/webhooks", base_url(addr)))
        .json(&serde_json::json!({
            "name": "evil",
            "url": "http://127.0.0.1:22/",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 400, "loopback must be rejected");

    let resp = client
        .post(format!("{}/api/v1/webhooks", base_url(addr)))
        .json(&serde_json::json!({
            "name": "metadata",
            "url": "http://169.254.169.254/latest/meta-data/",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        400,
        "AWS metadata IP must be rejected"
    );

    let resp = client
        .post(format!("{}/api/v1/webhooks", base_url(addr)))
        .json(&serde_json::json!({
            "name": "scheme",
            "url": "file:///etc/passwd",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        400,
        "non-http(s) scheme must be rejected"
    );
}

#[tokio::test]
async fn add_rss_feed_rejects_private_ip_url() {
    // Build a server with rss_feeds enabled so we exercise the SSRF guard
    // rather than the feature flag short-circuit.
    let config = Config {
        max_concurrent_downloads: 0,
        features: amigo_core::config::FeatureFlags {
            rss_feeds: true,
            ..Default::default()
        },
        ..Config::default()
    };
    let state = amigo_server::build_test_state(config);
    let app = amigo_server::build_test_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = test_client();

    let resp = client
        .post(format!("{}/api/v1/rss", base_url(addr)))
        .json(&serde_json::json!({
            "name": "evil",
            "url": "http://10.0.0.1/feed.xml",
            "category": "",
            "interval_minutes": 60_u32,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        400,
        "RFC1918 RSS URL must be rejected"
    );
}
