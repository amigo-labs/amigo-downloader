//! WebSocket integration tests.
//!
//! Verify the `/api/v1/ws` endpoint correctly broadcasts `DownloadEvent`s
//! to connected clients and tolerates concurrent subscribers.

use std::net::SocketAddr;
use std::time::Duration;

use amigo_core::config::Config;
use futures_util::StreamExt;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;

async fn spawn_test_server() -> SocketAddr {
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

fn test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("Failed to build test client")
}

async fn connect_ws(
    addr: SocketAddr,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let url = format!("ws://{addr}/api/v1/ws");
    let req = url.into_client_request().expect("ws request build");
    let (stream, response) = tokio_tungstenite::connect_async(req)
        .await
        .expect("ws connect");
    assert_eq!(
        response.status(),
        101,
        "WebSocket upgrade must return 101 Switching Protocols"
    );
    // No sleep required: `ws_handler` subscribes to the coordinator's event
    // broadcast before calling `on_upgrade`, so the receiver is live by the
    // time this 101 response reaches the client.
    stream
}

async fn next_event(
    stream: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> serde_json::Value {
    let msg = timeout(Duration::from_secs(2), stream.next())
        .await
        .expect("timed out waiting for ws event")
        .expect("ws stream closed")
        .expect("ws message error");
    match msg {
        Message::Text(t) => serde_json::from_str(&t).expect("ws text not JSON"),
        other => panic!("unexpected ws message: {other:?}"),
    }
}

#[tokio::test]
async fn test_ws_upgrade_succeeds() {
    let addr = spawn_test_server().await;
    let mut ws = connect_ws(addr).await;
    // Close cleanly to exercise the disconnect path.
    ws.close(None).await.ok();
}

#[tokio::test]
async fn test_ws_receives_added_event_on_new_download() {
    let addr = spawn_test_server().await;
    let mut ws = connect_ws(addr).await;
    let client = test_client();

    client
        .post(format!("http://{addr}/api/v1/downloads"))
        .json(&serde_json::json!({"url": "https://example.com/file.zip"}))
        .send()
        .await
        .unwrap();

    let event = next_event(&mut ws).await;
    assert_eq!(event["type"], "added");
    assert_eq!(event["url"], "https://example.com/file.zip");
    assert!(event["id"].is_string());
}

#[tokio::test]
async fn test_ws_receives_status_changed_on_pause() {
    let addr = spawn_test_server().await;
    let mut ws = connect_ws(addr).await;
    let client = test_client();

    let resp = client
        .post(format!("http://{addr}/api/v1/downloads"))
        .json(&serde_json::json!({"url": "https://example.com/file.zip"}))
        .send()
        .await
        .unwrap();
    let id = resp.json::<serde_json::Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Drain the "added" event.
    let added = next_event(&mut ws).await;
    assert_eq!(added["type"], "added");

    client
        .patch(format!("http://{addr}/api/v1/downloads/{id}"))
        .json(&serde_json::json!({"action": "pause"}))
        .send()
        .await
        .unwrap();

    let event = next_event(&mut ws).await;
    assert_eq!(event["type"], "status_changed");
    assert_eq!(event["id"], id);
    assert_eq!(event["status"], "paused");
}

#[tokio::test]
async fn test_ws_two_clients_both_receive_event() {
    let addr = spawn_test_server().await;
    let mut ws1 = connect_ws(addr).await;
    let mut ws2 = connect_ws(addr).await;
    let client = test_client();

    client
        .post(format!("http://{addr}/api/v1/downloads"))
        .json(&serde_json::json!({"url": "https://example.com/parallel.zip"}))
        .send()
        .await
        .unwrap();

    let e1 = next_event(&mut ws1).await;
    let e2 = next_event(&mut ws2).await;
    assert_eq!(e1["type"], "added");
    assert_eq!(e2["type"], "added");
    assert_eq!(e1["url"], "https://example.com/parallel.zip");
    assert_eq!(e2["url"], "https://example.com/parallel.zip");
    assert_eq!(e1["id"], e2["id"]);
}

/// Spawn a server behind real auth, returning the address plus a state clone
/// so the test can seed sessions and read the pre-shared admin token.
async fn spawn_auth_server() -> (SocketAddr, amigo_server::api::AppState) {
    let mut config = Config {
        max_concurrent_downloads: 0,
        ..Config::default()
    };
    config.server.api_token = Some("admin-secret".into());
    config.server.setup_complete = true;

    let state = amigo_server::build_test_state(config);
    let state_clone = state.clone();
    // bind_is_loopback = true suppresses the setup wizard but leaves the auth
    // middleware active, so every WS client must present a credential.
    let app = amigo_server::build_full_test_router(state, None, true).await;

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
    tokio::time::sleep(Duration::from_millis(20)).await;
    (addr, state_clone)
}

async fn connect_ws_with_header(
    addr: SocketAddr,
    name: &'static str,
    value: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let url = format!("ws://{addr}/api/v1/ws");
    let mut req = url.into_client_request().expect("ws request build");
    req.headers_mut()
        .insert(name, HeaderValue::from_str(value).unwrap());
    let (stream, response) = tokio_tungstenite::connect_async(req)
        .await
        .expect("ws connect");
    assert_eq!(response.status(), 101);
    stream
}

/// Assert no event arrives within a short window (used to prove isolation).
async fn assert_no_event(
    stream: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) {
    match timeout(Duration::from_millis(600), stream.next()).await {
        Err(_) => {} // timed out — good, nothing delivered
        Ok(Some(Ok(Message::Text(t)))) => {
            panic!("client received an event it should not see: {t}")
        }
        Ok(other) => panic!("unexpected ws state: {other:?}"),
    }
}

#[tokio::test]
async fn test_ws_events_are_scoped_per_session_owner() {
    // Issue #32: a login session must only see events for its own downloads,
    // while operator credentials (pre-shared token) see everything.
    let (addr, state) = spawn_auth_server().await;

    let alice_sid = amigo_server::login::create_session(&state, "alice", 3600)
        .await
        .expect("seed alice session");
    let bob_sid = amigo_server::login::create_session(&state, "bob", 3600)
        .await
        .expect("seed bob session");

    let mut ws_alice =
        connect_ws_with_header(addr, "cookie", &format!("amigo_session={alice_sid}")).await;
    let mut ws_bob =
        connect_ws_with_header(addr, "cookie", &format!("amigo_session={bob_sid}")).await;
    let mut ws_admin = connect_ws_with_header(addr, "authorization", "Bearer admin-secret").await;

    // Alice creates a download (owner = "alice").
    let client = test_client();
    client
        .post(format!("http://{addr}/api/v1/downloads"))
        .header("cookie", format!("amigo_session={alice_sid}"))
        .json(&serde_json::json!({"url": "https://example.com/alice.zip"}))
        .send()
        .await
        .unwrap();

    // Alice sees her own "added" event...
    let e_alice = next_event(&mut ws_alice).await;
    assert_eq!(e_alice["type"], "added");
    assert_eq!(e_alice["url"], "https://example.com/alice.zip");

    // ...the admin (pre-shared token) sees it too...
    let e_admin = next_event(&mut ws_admin).await;
    assert_eq!(e_admin["type"], "added");
    assert_eq!(e_admin["url"], "https://example.com/alice.zip");

    // ...but Bob must not see another user's download.
    assert_no_event(&mut ws_bob).await;
}

#[tokio::test]
async fn test_ws_client_disconnect_does_not_block_others() {
    let addr = spawn_test_server().await;
    let mut ws_a = connect_ws(addr).await;
    let mut ws_b = connect_ws(addr).await;
    let client = test_client();

    // Drop client A by closing the socket.
    ws_a.close(None).await.ok();
    drop(ws_a);

    // Server should keep delivering to client B.
    client
        .post(format!("http://{addr}/api/v1/downloads"))
        .json(&serde_json::json!({"url": "https://example.com/solo.zip"}))
        .send()
        .await
        .unwrap();

    let event = next_event(&mut ws_b).await;
    assert_eq!(event["type"], "added");
    assert_eq!(event["url"], "https://example.com/solo.zip");
}
