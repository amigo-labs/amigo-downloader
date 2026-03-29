//! WebSocket handler for live progress updates.

use axum::{
    Router,
    extract::{State, WebSocketUpgrade, ws::Message},
    response::Response,
    routing::get,
};
use tracing::{debug, warn};

use crate::api::AppState;

pub fn ws_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/ws", get(ws_handler))
        .with_state(state)
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: axum::extract::ws::WebSocket, state: AppState) {
    debug!("WebSocket client connected");
    let mut rx = state.coordinator.subscribe();

    loop {
        match rx.recv().await {
            Ok(event) => {
                // DownloadEvent derives Serialize with tag="type", so we can serialize directly
                if let Ok(json) = serde_json::to_string(&event)
                    && socket.send(Message::Text(json.into())).await.is_err() {
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
