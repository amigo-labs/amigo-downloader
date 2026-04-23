//! Device-pairing endpoints — lets a CLI/script obtain an API token by
//! asking the admin to click "Approve" in the Web UI.
//!
//! Endpoints:
//! - `POST /api/v1/pairing/start` (public, rate-limited) —
//!   body: `{ device_name, user_agent? }`.
//!   returns: `{ id, poll_token, fingerprint, expires_at }`.
//! - `GET /api/v1/pairing/status?poll_token=...` (public) —
//!   `{ status }` for pending/denied/expired, or
//!   `{ status: "approved", token, token_name }` exactly once.
//! - `GET /api/v1/pairing/pending` (auth) — admin view of all pending rows.
//! - `POST /api/v1/pairing/approve { id }` (auth) — admin approves.
//! - `POST /api/v1/pairing/deny { id }` (auth) — admin denies.

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use amigo_core::storage::{ApiTokenRow, PairingRequestRow};
use axum::{
    Json, Router,
    extract::{ConnectInfo, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::api::AppState;
use crate::auth::{self, AuthState};

/// Pairing-request lifetime. Long enough for the admin to walk to their
/// browser; short enough that stale rows don't pile up.
const PAIRING_TTL_SECS: i64 = 5 * 60;

/// Max `/pairing/start` calls per source-IP per minute.
const RL_MAX: usize = 10;
const RL_WINDOW: std::time::Duration = std::time::Duration::from_secs(60);

type RateLimiter = Arc<Mutex<HashMap<String, VecDeque<Instant>>>>;

/// Helper: check-and-record an IP request, returning `false` when the
/// limit is exceeded for the current window.
async fn rl_allow(rl: &RateLimiter, ip: &str) -> bool {
    let now = Instant::now();
    let mut map = rl.lock().await;
    let q = map.entry(ip.to_string()).or_default();
    while let Some(&ts) = q.front() {
        if now.duration_since(ts) > RL_WINDOW {
            q.pop_front();
        } else {
            break;
        }
    }
    if q.len() >= RL_MAX {
        return false;
    }
    q.push_back(now);
    true
}

#[derive(Clone)]
struct PairingState {
    app: AppState,
    auth: AuthState,
    rl: RateLimiter,
}

pub fn pairing_router(state: AppState, auth: AuthState) -> Router {
    let ps = PairingState {
        app: state,
        auth: auth.clone(),
        rl: Arc::new(Mutex::new(HashMap::new())),
    };
    let auth_layer =
        axum::middleware::from_fn_with_state(auth.clone(), crate::auth::require_auth);

    Router::new()
        .route(
            "/api/v1/pairing/pending",
            get(list_pending).route_layer(auth_layer.clone()),
        )
        .route(
            "/api/v1/pairing/approve",
            post(approve).route_layer(auth_layer.clone()),
        )
        .route(
            "/api/v1/pairing/deny",
            post(deny).route_layer(auth_layer),
        )
        .route("/api/v1/pairing/start", post(start))
        .route("/api/v1/pairing/status", get(status))
        .with_state(ps)
}

// ---- request / response shapes ----

#[derive(Deserialize)]
struct StartRequest {
    device_name: Option<String>,
}

#[derive(Serialize)]
struct StartResponse {
    id: String,
    poll_token: String,
    fingerprint: String,
    expires_at: i64,
}

#[derive(Deserialize)]
struct StatusQuery {
    poll_token: String,
}

#[derive(Serialize)]
struct StatusResponse {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    token_name: Option<String>,
}

#[derive(Serialize)]
struct PendingRow {
    id: String,
    device_name: String,
    source_ip: String,
    user_agent: String,
    fingerprint: String,
    created_at: i64,
}

#[derive(Deserialize)]
struct IdRequest {
    id: String,
}

// ---- handlers ----

fn rand_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    hex::encode(buf)
}

/// 6-digit fingerprint formatted as `NNN-NNN` for easy verbal comparison.
fn fingerprint() -> String {
    let mut buf = [0u8; 4];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    let n = u32::from_le_bytes(buf) % 1_000_000;
    let s = format!("{n:06}");
    format!("{}-{}", &s[..3], &s[3..])
}

async fn start(
    State(ps): State<PairingState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<StartRequest>,
) -> Response {
    let cfg = ps.app.coordinator.config().await;
    let ip = auth::client_ip(&headers, Some(peer), cfg.server.trust_proxy);

    if !rl_allow(&ps.rl, &ip).await {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({ "error": "rate_limited" })),
        )
            .into_response();
    }

    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let device_name = req
        .device_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("unnamed device")
        .to_string();

    let now = chrono::Utc::now().timestamp();
    let id = Uuid::new_v4().to_string();
    let poll_token = rand_hex(32);
    let poll_token_hash = auth::hash_api_token(&poll_token);
    let row = PairingRequestRow {
        id: id.clone(),
        poll_token_hash,
        device_name,
        source_ip: ip,
        user_agent,
        fingerprint: fingerprint(),
        status: "pending".into(),
        api_token_plain: None,
        api_token_id: None,
        created_at: now,
        expires_at: now + PAIRING_TTL_SECS,
    };
    if let Err(e) = ps
        .app
        .coordinator
        .storage()
        .create_pairing_request(&row)
        .await
    {
        tracing::error!("pairing start: storage error: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    Json(StartResponse {
        id: row.id,
        poll_token,
        fingerprint: row.fingerprint,
        expires_at: row.expires_at,
    })
    .into_response()
}

async fn status(
    State(ps): State<PairingState>,
    Query(q): Query<StatusQuery>,
) -> Response {
    let hash = auth::hash_api_token(&q.poll_token);

    // Housekeeping: flip stale pending rows to 'expired' before we look.
    let now = chrono::Utc::now().timestamp();
    let _ = ps.app.coordinator.storage().expire_pairings(now).await;

    // Approved rows are consumed atomically — one-shot delivery of the token.
    if let Ok(Some(row)) = ps
        .app
        .coordinator
        .storage()
        .consume_pairing_by_poll_hash(&hash)
        .await
    {
        return Json(StatusResponse {
            status: "approved".into(),
            token: row.api_token_plain,
            token_name: Some(row.device_name),
        })
        .into_response();
    }

    match ps
        .app
        .coordinator
        .storage()
        .get_pairing_by_poll_hash(&hash)
        .await
    {
        Ok(Some(row)) => Json(StatusResponse {
            status: row.status,
            token: None,
            token_name: None,
        })
        .into_response(),
        Ok(None) => Json(StatusResponse {
            status: "not_found".into(),
            token: None,
            token_name: None,
        })
        .into_response(),
        Err(e) => {
            tracing::error!("pairing status: storage error: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn list_pending(State(ps): State<PairingState>) -> Response {
    let now = chrono::Utc::now().timestamp();
    let _ = ps.app.coordinator.storage().expire_pairings(now).await;
    match ps.app.coordinator.storage().list_pending_pairings().await {
        Ok(rows) => Json(
            rows.into_iter()
                .map(|r| PendingRow {
                    id: r.id,
                    device_name: r.device_name,
                    source_ip: r.source_ip,
                    user_agent: r.user_agent,
                    fingerprint: r.fingerprint,
                    created_at: r.created_at,
                })
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(e) => {
            tracing::error!("list_pending: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn approve(
    State(ps): State<PairingState>,
    Json(req): Json<IdRequest>,
) -> Response {
    // Generate a fresh bearer token; store hashed on api_tokens, plain on
    // the pairing row (consumed exactly once by `/status`).
    let plain = rand_hex(32);
    let hash = auth::hash_api_token(&plain);
    let token_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();

    // Look up the row to use its device_name for the token label.
    let pending = match ps.app.coordinator.storage().list_pending_pairings().await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("approve: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let Some(row) = pending.into_iter().find(|r| r.id == req.id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let token_row = ApiTokenRow {
        id: token_id.clone(),
        token_hash: hash,
        name: row.device_name.clone(),
        created_at: now,
        last_used_at: None,
        last_used_ip: None,
        expires_at: None,
        revoked: false,
    };
    if let Err(e) = ps
        .app
        .coordinator
        .storage()
        .create_api_token(&token_row)
        .await
    {
        tracing::error!("approve: token create: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    match ps
        .app
        .coordinator
        .storage()
        .approve_pairing(&req.id, &plain, &token_id)
        .await
    {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Ok(false) => {
            // Raced — pairing already moved out of pending. Revoke the just-
            // created token so we don't leak a credential.
            let _ = ps
                .app
                .coordinator
                .storage()
                .revoke_api_token(&token_id)
                .await;
            StatusCode::CONFLICT.into_response()
        }
        Err(e) => {
            tracing::error!("approve: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn deny(State(ps): State<PairingState>, Json(req): Json<IdRequest>) -> Response {
    match ps.app.coordinator.storage().deny_pairing(&req.id).await {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!("deny: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_has_six_digits_with_dash() {
        let fp = fingerprint();
        assert_eq!(fp.len(), 7, "{fp}");
        assert_eq!(fp.chars().nth(3), Some('-'));
        let digits: String = fp.chars().filter(|c| c.is_ascii_digit()).collect();
        assert_eq!(digits.len(), 6);
    }

    #[tokio::test]
    async fn rate_limiter_caps_requests_per_window() {
        let rl: RateLimiter = Arc::new(Mutex::new(HashMap::new()));
        for _ in 0..RL_MAX {
            assert!(rl_allow(&rl, "1.2.3.4").await);
        }
        assert!(!rl_allow(&rl, "1.2.3.4").await);
        // Different IP has its own bucket.
        assert!(rl_allow(&rl, "5.6.7.8").await);
    }
}
