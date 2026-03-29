//! Webhook dispatcher — sends download events to configured HTTP endpoints.
//!
//! Subscribes to `DownloadEvent`s and POSTs JSON payloads to configured URLs.
//! Supports HMAC-SHA256 signing, event filtering, and retry with backoff.

use std::sync::Arc;
use std::time::Duration;

use amigo_core::config::WebhookEndpoint;
use amigo_core::coordinator::DownloadEvent;
use serde::Serialize;
use tokio::sync::{RwLock, broadcast};
use tracing::{debug, error, info, warn};

/// Webhook delivery payload.
#[derive(Debug, Serialize)]
struct WebhookPayload {
    event: String,
    timestamp: String,
    data: serde_json::Value,
}

/// Dispatches download events to configured webhook endpoints.
pub struct WebhookDispatcher {
    client: reqwest::Client,
    endpoints: Arc<RwLock<Vec<WebhookEndpoint>>>,
}

impl WebhookDispatcher {
    pub fn new(endpoints: Vec<WebhookEndpoint>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("amigo-downloader/0.1.0")
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to build webhook HTTP client"),
            endpoints: Arc::new(RwLock::new(endpoints)),
        }
    }

    /// Run the dispatcher loop — subscribes to events and dispatches webhooks.
    pub async fn run(&self, mut event_rx: broadcast::Receiver<DownloadEvent>) {
        info!("Webhook dispatcher started");
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    let event_type = event_type_string(&event);

                    // Skip progress events (too frequent for webhooks)
                    if event_type == "progress" {
                        continue;
                    }

                    let endpoints = self.endpoints.read().await;
                    let matching: Vec<_> = endpoints
                        .iter()
                        .filter(|ep| {
                            ep.enabled && ep.events.iter().any(|e| e == "*" || e == &event_type)
                        })
                        .cloned()
                        .collect();
                    drop(endpoints);

                    if matching.is_empty() {
                        continue;
                    }

                    let payload = WebhookPayload {
                        event: event_type.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        data: serde_json::to_value(&event).unwrap_or_default(),
                    };

                    let payload_json = match serde_json::to_string(&payload) {
                        Ok(j) => j,
                        Err(e) => {
                            error!("Failed to serialize webhook payload: {e}");
                            continue;
                        }
                    };

                    for endpoint in matching {
                        let client = self.client.clone();
                        let json = payload_json.clone();
                        let event_type = event_type.clone();

                        tokio::spawn(async move {
                            dispatch_with_retry(&client, &endpoint, &json, &event_type).await;
                        });
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("Webhook dispatcher lagged, skipped {n} events");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    info!("Event channel closed, webhook dispatcher stopping");
                    break;
                }
            }
        }
    }

    /// Get current webhook endpoints.
    pub async fn list_endpoints(&self) -> Vec<WebhookEndpoint> {
        self.endpoints.read().await.clone()
    }

    /// Add a new webhook endpoint.
    pub async fn add_endpoint(&self, endpoint: WebhookEndpoint) {
        self.endpoints.write().await.push(endpoint);
    }

    /// Remove a webhook endpoint by ID.
    pub async fn remove_endpoint(&self, id: &str) -> bool {
        let mut endpoints = self.endpoints.write().await;
        let before = endpoints.len();
        endpoints.retain(|ep| ep.id != id);
        endpoints.len() < before
    }

    /// Update a webhook endpoint.
    pub async fn update_endpoint(&self, id: &str, updated: WebhookEndpoint) -> bool {
        let mut endpoints = self.endpoints.write().await;
        if let Some(ep) = endpoints.iter_mut().find(|ep| ep.id == id) {
            *ep = updated;
            true
        } else {
            false
        }
    }

    /// Send a test event to a specific webhook.
    pub async fn send_test(&self, id: &str) -> Result<u16, String> {
        let endpoints = self.endpoints.read().await;
        let endpoint = endpoints
            .iter()
            .find(|ep| ep.id == id)
            .ok_or_else(|| format!("Webhook not found: {id}"))?
            .clone();
        drop(endpoints);

        let payload = WebhookPayload {
            event: "test".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            data: serde_json::json!({
                "message": "This is a test webhook from amigo-downloader"
            }),
        };

        let json = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
        dispatch_once(&self.client, &endpoint, &json, "test").await
    }
}

async fn dispatch_with_retry(
    client: &reqwest::Client,
    endpoint: &WebhookEndpoint,
    payload_json: &str,
    event_type: &str,
) {
    for attempt in 0..=endpoint.retry_count {
        match dispatch_once(client, endpoint, payload_json, event_type).await {
            Ok(status) if (200..300).contains(&status) => {
                debug!(
                    "Webhook delivered: {} → {} (status {})",
                    event_type, endpoint.name, status
                );
                return;
            }
            Ok(status) => {
                warn!(
                    "Webhook {} returned status {}, attempt {}/{}",
                    endpoint.name,
                    status,
                    attempt + 1,
                    endpoint.retry_count + 1
                );
            }
            Err(e) => {
                warn!(
                    "Webhook {} failed: {}, attempt {}/{}",
                    endpoint.name,
                    e,
                    attempt + 1,
                    endpoint.retry_count + 1
                );
            }
        }

        if attempt < endpoint.retry_count {
            let delay = Duration::from_secs(endpoint.retry_delay_secs as u64 * (attempt as u64 + 1));
            tokio::time::sleep(delay).await;
        }
    }
    error!(
        "Webhook {} exhausted all retries for event {}",
        endpoint.name, event_type
    );
}

async fn dispatch_once(
    client: &reqwest::Client,
    endpoint: &WebhookEndpoint,
    payload_json: &str,
    event_type: &str,
) -> Result<u16, String> {
    let delivery_id = uuid::Uuid::new_v4().to_string();

    let mut req = client
        .post(&endpoint.url)
        .header("Content-Type", "application/json")
        .header("X-Amigo-Event", event_type)
        .header("X-Amigo-Delivery", &delivery_id);

    // HMAC-SHA256 signing
    if let Some(secret) = &endpoint.secret {
        use hmac::{Hmac, Mac};
        type HmacSha256 = Hmac<sha2::Sha256>;
        if let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) {
            mac.update(payload_json.as_bytes());
            let signature = hex::encode(mac.finalize().into_bytes());
            req = req.header("X-Amigo-Signature", format!("sha256={signature}"));
        }
    }

    let resp = req
        .body(payload_json.to_string())
        .send()
        .await
        .map_err(|e| e.to_string())?;

    Ok(resp.status().as_u16())
}

/// Map a DownloadEvent to its webhook event type string.
fn event_type_string(event: &DownloadEvent) -> String {
    match event {
        DownloadEvent::Added { .. } => "download.added",
        DownloadEvent::Progress { .. } => "progress",
        DownloadEvent::StatusChanged { .. } => "download.status_changed",
        DownloadEvent::Completed { .. } => "download.completed",
        DownloadEvent::Failed { .. } => "download.failed",
        DownloadEvent::Removed { .. } => "download.removed",
        DownloadEvent::PluginNotification { .. } => "plugin.notification",
        DownloadEvent::CaptchaChallenge { .. } => "captcha.required",
        DownloadEvent::CaptchaSolved { .. } => "captcha.solved",
        DownloadEvent::CaptchaTimeout { .. } => "captcha.timeout",
        DownloadEvent::QueueEmpty => "queue.empty",
    }
    .to_string()
}
