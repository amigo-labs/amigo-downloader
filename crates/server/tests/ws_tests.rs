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
    // Brief wait so the server-side handler runs `coordinator.subscribe()`
    // before we send a request that would otherwise fire its event first.
    tokio::time::sleep(Duration::from_millis(50)).await;
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
