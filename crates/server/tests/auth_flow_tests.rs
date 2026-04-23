//! End-to-end tests for the setup → login → pairing flow.
//!
//! Each test spins up a real Axum server on a random loopback port. We
//! mark the auth state as `bind_is_loopback = false` so the setup guard
//! actually fires — otherwise loopback bypasses the whole wizard.

use std::net::SocketAddr;
use std::path::PathBuf;

use amigo_core::config::Config;

fn init_tracing_once() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("amigo_server=debug")
            .with_test_writer()
            .try_init();
    });
}

async fn spawn(setup_pin: Option<&str>) -> (SocketAddr, PathBuf) {
    init_tracing_once();
    let mut config = Config::default();
    // Force non-loopback semantics so setup-mode is exercised even though
    // we actually listen on 127.0.0.1 during the test.
    config.server.bind = "0.0.0.0:0".into();

    let state = amigo_server::build_test_state(config);
    let config_path = state.config_path.clone();
    let app =
        amigo_server::build_full_test_router(state, setup_pin.map(|s| s.to_string()), false).await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    // Give the listener a moment to come up.
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    (addr, config_path)
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
}

fn url(addr: SocketAddr, path: &str) -> String {
    format!("http://{addr}{path}")
}

#[tokio::test]
async fn setup_guard_blocks_api_before_wizard() {
    let (addr, _) = spawn(None).await;
    let c = client();
    let res = c
        .get(url(addr, "/api/v1/downloads"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 503);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["error"], "setup_required");
}

#[tokio::test]
async fn setup_status_reports_needs_setup() {
    let (addr, _) = spawn(None).await;
    let c = client();
    let status: serde_json::Value = c
        .get(url(addr, "/api/v1/setup/status"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(status["needs_setup"], true);
    assert_eq!(status["needs_pin"], false);
}

#[tokio::test]
async fn setup_complete_then_login_flow() {
    let (addr, _config_path) = spawn(None).await;
    let c = client();

    // Create admin account.
    let res = c
        .post(url(addr, "/api/v1/setup/complete"))
        .json(&serde_json::json!({
            "username": "admin",
            "password": "hunter22hunter",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "{:?}", res.text().await);

    // The setup wizard must be one-shot afterwards.
    let res2 = c
        .post(url(addr, "/api/v1/setup/complete"))
        .json(&serde_json::json!({ "username": "admin", "password": "ignored22" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res2.status(), 410);

    // Login with the fresh admin credentials — a brand-new client to make
    // sure the cookie-on-setup path isn't what's authenticating us.
    let fresh = client();
    let login_res = fresh
        .post(url(addr, "/api/v1/login"))
        .json(&serde_json::json!({
            "username": "admin",
            "password": "hunter22hunter",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(login_res.status(), 200);
    assert!(login_res.headers().contains_key("set-cookie"));

    // /me returns the session principal.
    let me: serde_json::Value = fresh
        .get(url(addr, "/api/v1/me"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(me["kind"], "session");
    assert_eq!(me["username"], "admin");
}

#[tokio::test]
async fn setup_pin_required_when_env_set() {
    let (addr, _) = spawn(Some("topsecret")).await;
    let c = client();

    // Missing PIN → 403.
    let res = c
        .post(url(addr, "/api/v1/setup/complete"))
        .json(&serde_json::json!({ "username": "admin", "password": "abcdefgh" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);

    // Wrong PIN → 403.
    let res = c
        .post(url(addr, "/api/v1/setup/complete"))
        .header("X-Setup-Pin", "nope")
        .json(&serde_json::json!({ "username": "admin", "password": "abcdefgh" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 403);

    // Correct PIN → 200.
    let res = c
        .post(url(addr, "/api/v1/setup/complete"))
        .header("X-Setup-Pin", "topsecret")
        .json(&serde_json::json!({ "username": "admin", "password": "abcdefgh" }))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
}

#[tokio::test]
async fn pairing_happy_path_issues_bearer_token() {
    let (addr, _) = spawn(None).await;
    let c = client();

    // Complete setup first so we have an authenticated admin to approve with.
    c.post(url(addr, "/api/v1/setup/complete"))
        .json(&serde_json::json!({ "username": "admin", "password": "abcdefgh" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    // CLI: start pairing.
    let start: serde_json::Value = c
        .post(url(addr, "/api/v1/pairing/start"))
        .json(&serde_json::json!({ "device_name": "laptop" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let poll_token = start["poll_token"].as_str().unwrap().to_string();
    let pairing_id = start["id"].as_str().unwrap().to_string();
    let fp_start = start["fingerprint"].as_str().unwrap().to_string();

    // Admin: sees the pending row with the *same* fingerprint.
    let pending: serde_json::Value = c
        .get(url(addr, "/api/v1/pairing/pending"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let list = pending.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["id"], pairing_id);
    assert_eq!(list[0]["fingerprint"], fp_start);
    assert_eq!(list[0]["device_name"], "laptop");

    // Admin: approve.
    c.post(url(addr, "/api/v1/pairing/approve"))
        .json(&serde_json::json!({ "id": pairing_id }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    // CLI: poll for status — first hit consumes the token.
    let status1: serde_json::Value = c
        .get(url(
            addr,
            &format!("/api/v1/pairing/status?poll_token={poll_token}"),
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(status1["status"], "approved");
    let bearer = status1["token"].as_str().unwrap().to_string();

    // Second poll: row has been consumed / deleted.
    let status2: serde_json::Value = c
        .get(url(
            addr,
            &format!("/api/v1/pairing/status?poll_token={poll_token}"),
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(status2["status"], "not_found");

    // Bearer token actually authenticates a protected request.
    let bearer_client = client();
    let res = bearer_client
        .get(url(addr, "/api/v1/downloads"))
        .bearer_auth(&bearer)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200, "{:?}", res.text().await);
}

#[tokio::test]
async fn pairing_deny_yields_no_token() {
    let (addr, _) = spawn(None).await;
    let c = client();

    c.post(url(addr, "/api/v1/setup/complete"))
        .json(&serde_json::json!({ "username": "admin", "password": "abcdefgh" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let start: serde_json::Value = c
        .post(url(addr, "/api/v1/pairing/start"))
        .json(&serde_json::json!({ "device_name": "evil" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let poll_token = start["poll_token"].as_str().unwrap().to_string();
    let pairing_id = start["id"].as_str().unwrap().to_string();

    c.post(url(addr, "/api/v1/pairing/deny"))
        .json(&serde_json::json!({ "id": pairing_id }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let status: serde_json::Value = c
        .get(url(
            addr,
            &format!("/api/v1/pairing/status?poll_token={poll_token}"),
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(status["status"], "denied");
    assert!(status.get("token").is_none() || status["token"].is_null());
}
