//! Browser login + logout + /me.
//!
//! - `POST /api/v1/login`   — body `{username, password}`; sets session cookie.
//! - `POST /api/v1/logout`  — requires auth; deletes the session row + clears the cookie.
//! - `GET  /api/v1/me`      — requires auth; returns `{principal}`.

use std::sync::Arc;

use amigo_core::storage::SessionRow;
use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::api::AppState;
use crate::auth::{AuthState, Principal, SESSION_COOKIE};
use crate::password;

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct MeResponse {
    kind: &'static str,
    username: Option<String>,
    token_name: Option<String>,
}

pub fn login_router(state: AppState, auth: AuthState) -> Router {
    let auth_layer = axum::middleware::from_fn_with_state(auth.clone(), crate::auth::require_auth);
    Router::new()
        .route(
            "/api/v1/logout",
            post(logout).route_layer(auth_layer.clone()),
        )
        .route("/api/v1/me", get(me).route_layer(auth_layer))
        .route("/api/v1/login", post(login))
        .with_state(Arc::new((state, auth)))
}

/// Generate a URL-safe opaque session id (32 bytes of randomness, hex-encoded).
pub fn new_session_id() -> String {
    let mut buf = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    hex::encode(buf)
}

/// Insert a new session row and return its id. Called by both the setup
/// handler (right after account creation) and the login handler.
pub async fn create_session(
    state: &AppState,
    username: &str,
    ttl_secs: i64,
) -> Result<String, String> {
    let now = chrono::Utc::now().timestamp();
    let id = new_session_id();
    let row = SessionRow {
        id: id.clone(),
        username: username.to_string(),
        created_at: now,
        expires_at: now + ttl_secs,
        last_seen_at: now,
    };
    state
        .coordinator
        .storage()
        .create_session(&row)
        .await
        .map_err(|e| e.to_string())?;
    Ok(id)
}

async fn login(
    State(s): State<Arc<(AppState, AuthState)>>,
    Json(req): Json<LoginRequest>,
) -> Response {
    let (app, _auth) = &*s;
    let cfg = app.coordinator.config().await;
    let expected_user = cfg.server.admin_username.as_deref();
    let expected_hash = cfg.server.admin_password_hash.as_deref();
    let (Some(user), Some(hash)) = (expected_user, expected_hash) else {
        return (StatusCode::SERVICE_UNAVAILABLE, "setup not complete").into_response();
    };
    if req.username != user {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let ok = password::verify_password(&req.password, hash).unwrap_or(false);
    if !ok {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let session_id = match create_session(app, user, cfg.server.session_ttl_secs).await {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("login: session create failed: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let cookie = Cookie::build((SESSION_COOKIE, session_id))
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        .max_age(cookie::time::Duration::seconds(cfg.server.session_ttl_secs))
        .build();

    (
        StatusCode::OK,
        [(header::SET_COOKIE, cookie.to_string())],
        Json(serde_json::json!({ "ok": true, "username": user })),
    )
        .into_response()
}

async fn logout(
    State(s): State<Arc<(AppState, AuthState)>>,
    req: axum::extract::Request,
) -> Response {
    let (app, _) = &*s;
    if let Some(Principal::Session { session_id, .. }) = req.extensions().get::<Principal>() {
        let _ = app.coordinator.storage().delete_session(session_id).await;
    }
    // Clear the cookie regardless; the client may be a stale bearer user.
    let clear = Cookie::build((SESSION_COOKIE, ""))
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        .max_age(cookie::time::Duration::seconds(0))
        .build();
    (
        StatusCode::OK,
        [(header::SET_COOKIE, clear.to_string())],
        Json(serde_json::json!({ "ok": true })),
    )
        .into_response()
}

async fn me(req: axum::extract::Request) -> Response {
    let principal = req.extensions().get::<Principal>().cloned();
    let resp = match principal {
        Some(Principal::Session { username, .. }) => MeResponse {
            kind: "session",
            username: Some(username),
            token_name: None,
        },
        Some(Principal::ApiToken { name, .. }) => MeResponse {
            kind: "api_token",
            username: None,
            token_name: Some(name),
        },
        Some(Principal::Preshared) => MeResponse {
            kind: "preshared",
            username: None,
            token_name: None,
        },
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };
    Json(resp).into_response()
}
