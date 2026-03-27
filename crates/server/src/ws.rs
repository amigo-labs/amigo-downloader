//! WebSocket handler for live progress updates.

use axum::{
    Router,
    extract::{State, WebSocketUpgrade, ws::Message},
    response::Response,
    routing::get,
};
use serde::Serialize;
use tracing::{debug, warn};

use amigo_core::coordinator::DownloadEvent;

use crate::api::AppState;

pub fn ws_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/ws", get(ws_handler))
        .with_state(state)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(
    mut socket: axum::extract::ws::WebSocket,
    state: AppState,
) {
    debug!("WebSocket client connected");
    let mut rx = state.coordinator.subscribe();

    loop {
        match rx.recv().await {
            Ok(event) => {
                let msg = match &event {
                    DownloadEvent::Added { id, url } => {
                        serde_json::to_string(&WsMessage {
                            event_type: "added",
                            id,
                            data: serde_json::json!({ "url": url }),
                        })
                    }
                    DownloadEvent::Progress { id, progress } => {
                        serde_json::to_string(&WsMessage {
                            event_type: "progress",
                            id,
                            data: serde_json::json!({
                                "bytes_downloaded": progress.bytes_downloaded,
                                "total_bytes": progress.total_bytes,
                                "speed": progress.speed_bytes_per_sec,
                            }),
                        })
                    }
                    DownloadEvent::StatusChanged { id, status } => {
                        serde_json::to_string(&WsMessage {
                            event_type: "status",
                            id,
                            data: serde_json::json!({ "status": status }),
                        })
                    }
                    DownloadEvent::Completed { id } => {
                        serde_json::to_string(&WsMessage {
                            event_type: "completed",
                            id,
                            data: serde_json::json!({}),
                        })
                    }
                    DownloadEvent::Failed { id, error } => {
                        serde_json::to_string(&WsMessage {
                            event_type: "failed",
                            id,
                            data: serde_json::json!({ "error": error }),
                        })
                    }
                    DownloadEvent::Removed { id } => {
                        serde_json::to_string(&WsMessage {
                            event_type: "removed",
                            id,
                            data: serde_json::json!({}),
                        })
                    }
                };

                if let Ok(json) = msg {
                    if socket.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
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

#[derive(Serialize)]
struct WsMessage<'a> {
    #[serde(rename = "type")]
    event_type: &'a str,
    id: &'a str,
    data: serde_json::Value,
}
