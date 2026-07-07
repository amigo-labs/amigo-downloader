//! WebSocket handler for live progress updates.
//!
//! Events are scoped per authenticated principal: a browser login session only
//! receives events for downloads it created, while operator-level credentials
//! (the pre-shared token and API tokens) receive everything. Events not tied
//! to a specific download (queue/plugin notifications) go to every client.

use std::sync::Arc;

use axum::{
    Extension, Router,
    extract::{State, WebSocketUpgrade, ws::Message},
    response::Response,
    routing::get,
};
use tokio::sync::broadcast::Receiver;
use tracing::{debug, warn};

use crate::api::AppState;
use crate::auth::Principal;
use amigo_core::coordinator::{Coordinator, DownloadEvent};

pub fn ws_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/ws", get(ws_handler))
        .with_state(state)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    // Populated by the auth middleware in production. `Option` so the
    // auth-less test router (and any unauthenticated deployment) still works;
    // a missing principal is treated as admin — see `can_see`.
    principal: Option<Extension<Principal>>,
) -> Response {
    // Subscribe *before* the upgrade so the receiver is live by the time the
    // client sees the 101 response. If we subscribed inside `on_upgrade`,
    // the on_upgrade future could still be scheduling when the first event
    // is broadcast, causing the client to silently miss it.
    let rx = state.coordinator.subscribe();
    let coordinator = state.coordinator.clone();
    let principal = principal.map(|Extension(p)| p);
    ws.on_upgrade(move |socket| handle_socket(socket, rx, coordinator, principal))
}

/// Whether `principal` is allowed to see events for a download owned by
/// `owner`. Operator credentials (pre-shared / API token) and — for
/// backward compatibility with auth-less deployments — a missing principal
/// see everything; a login session sees only downloads it owns.
fn can_see(principal: &Option<Principal>, owner: Option<&str>) -> bool {
    match principal {
        None | Some(Principal::Preshared) | Some(Principal::ApiToken { .. }) => true,
        Some(Principal::Session { username, .. }) => owner == Some(username.as_str()),
    }
}

async fn handle_socket(
    mut socket: axum::extract::ws::WebSocket,
    mut rx: Receiver<DownloadEvent>,
    coordinator: Arc<Coordinator>,
    principal: Option<Principal>,
) {
    debug!("WebSocket client connected");

    loop {
        match rx.recv().await {
            Ok(event) => {
                // Filter per principal. Events not tied to a download are
                // global; download-scoped events are gated by ownership. The
                // owner lookup is skipped for admin principals (fast path).
                let deliver = match event.subject_download_id() {
                    None => true,
                    Some(id) => match &principal {
                        None | Some(Principal::Preshared) | Some(Principal::ApiToken { .. }) => {
                            true
                        }
                        Some(Principal::Session { .. }) => {
                            let owner = coordinator.download_owner(id).await;
                            can_see(&principal, owner.as_deref())
                        }
                    },
                };
                if !deliver {
                    continue;
                }

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
