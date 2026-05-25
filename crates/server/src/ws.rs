//! WebSocket handler for live progress updates.

use axum::{
    Router,
    extract::{State, WebSocketUpgrade, ws::Message},
    response::Response,
    routing::get,
};
use tokio::sync::broadcast::Receiver;
use tracing::{debug, warn};

use crate::api::AppState;
use amigo_core::coordinator::DownloadEvent;

pub fn ws_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/ws", get(ws_handler))
        .with_state(state)
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    // Subscribe *before* the upgrade so the receiver is live by the time the
    // client sees the 101 response. If we subscribed inside `on_upgrade`,
    // the on_upgrade future could still be scheduling when the first event
    // is broadcast, causing the client to silently miss it.
    let rx = state.coordinator.subscribe();
    ws.on_upgrade(move |socket| handle_socket(socket, rx))
}

async fn handle_socket(mut socket: axum::extract::ws::WebSocket, mut rx: Receiver<DownloadEvent>) {
    debug!("WebSocket client connected");

    loop {
        match rx.recv().await {
            Ok(event) => {
                // DownloadEvent derives Serialize with tag="type", so we can serialize directly
                if let Ok(json) = serde_json::to_string(&event)
                    && socket.send(Message::Text(json.into())).await.is_err()
                {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                warn!("WebSocket client lagged, skipped {n} events");
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }

    debug!("WebSocket client disconnected");
}
