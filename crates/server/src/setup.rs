//! First-run setup wizard endpoints.
//!
//! Flow:
//!
//! - `GET  /api/v1/setup/status` — public; returns `{needs_setup, needs_pin}`.
//! - `POST /api/v1/setup/complete` — body `{username, password, pin?}`.
//!   Protected by `require_setup_pin` when `AMIGO_SETUP_PIN` is set; TOFU
//!   otherwise. Hashes the password with Argon2id, persists to `config.toml`,
//!   marks `setup_complete = true`, and issues a session cookie so the
//!   wizard can redirect the browser straight into the app.

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::{Deserialize, Serialize};

use crate::api::AppState;
use crate::auth::{AuthState, SESSION_COOKIE};
use crate::login;
use crate::password;

#[derive(Serialize)]
struct SetupStatus {
    needs_setup: bool,
    needs_pin: bool,
}

#[derive(Deserialize)]
struct CompleteRequest {
    username: String,
    password: String,
}

pub fn setup_router(state: AppState, auth: AuthState) -> Router {
    let pin_layer =
        axum::middleware::from_fn_with_state(auth.clone(), crate::auth::require_setup_pin);

    Router::new()
        .route(
            "/api/v1/setup/complete",
            post(complete).route_layer(pin_layer),
        )
        .route("/api/v1/setup/status", get(status))
        .with_state((state, auth))
}

async fn status(State((_app, auth)): State<(AppState, AuthState)>) -> Json<SetupStatus> {
    Json(SetupStatus {
        needs_setup: !auth.setup_complete().await,
        needs_pin: auth.setup_pin.is_some(),
    })
}

async fn complete(
    State((app, auth)): State<(AppState, AuthState)>,
    Json(req): Json<CompleteRequest>,
) -> Response {
    if auth.setup_complete().await {
        return (StatusCode::GONE, "setup already complete").into_response();
    }
    if req.username.trim().is_empty() || req.password.len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            "username required and password must be >= 8 chars",
        )
            .into_response();
    }

    // Hash the password and persist to config.toml.
    let hash = match password::hash_password(&req.password) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("setup: hash failed: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let mut cfg = app.coordinator.config().await;
    cfg.server.admin_username = Some(req.username.trim().to_string());
    cfg.server.admin_password_hash = Some(hash);
    cfg.server.setup_complete = true;
    if let Err(e) = cfg.save(&app.config_path) {
        tracing::error!("setup: config save failed: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    app.coordinator.update_config(cfg.clone()).await;

    // Issue a session cookie so the wizard can drop the user straight into
    // the app without a separate login round-trip.
    let session_id = match login::create_session(
        &app,
        cfg.server.admin_username.as_deref().unwrap_or(""),
        cfg.server.session_ttl_secs,
    )
    .await
    {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("setup: session create failed: {e}");
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
        [(axum::http::header::SET_COOKIE, cookie.to_string())],
        Json(serde_json::json!({ "ok": true })),
    )
        .into_response()
}
