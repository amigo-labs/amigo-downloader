//! Manual captcha solving — broadcasts challenges to the UI and waits for user input.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, broadcast, oneshot};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::coordinator::DownloadEvent;

/// Error type for captcha operations.
#[derive(Debug, thiserror::Error)]
pub enum CaptchaError {
    #[error("Captcha timed out after {0}s")]
    Timeout(u64),
    #[error("Captcha cancelled by user")]
    Cancelled,
    #[error("Captcha not found: {0}")]
    NotFound(String),
    #[error("Captcha already solved: {0}")]
    AlreadySolved(String),
}

/// Configuration for the captcha system.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CaptchaConfig {
    /// Timeout in seconds for manual solving (default: 300 = 5 minutes).
    pub timeout_secs: u64,
}

impl Default for CaptchaConfig {
    fn default() -> Self {
        Self { timeout_secs: 300 }
    }
}

/// A pending captcha waiting for the user to solve it.
struct PendingCaptcha {
    answer_tx: oneshot::Sender<CaptchaAnswer>,
    created_at: Instant,
    plugin_id: String,
    download_id: String,
    image_url: String,
    captcha_type: String,
}

enum CaptchaAnswer {
    Solved(String),
    Cancelled,
}

/// Info about a pending captcha (for the REST API).
#[derive(Debug, Clone, serde::Serialize)]
pub struct CaptchaInfo {
    pub id: String,
    pub plugin_id: String,
    pub download_id: String,
    pub image_url: String,
    pub captcha_type: String,
    pub elapsed_secs: u64,
    pub timeout_secs: u64,
}

/// Manages manual captcha solving via the Web UI.
///
/// Flow:
/// 1. Plugin calls `amigo.solveCaptcha(imageUrl)` → Host API calls `request_solve()`
/// 2. CaptchaManager broadcasts a `CaptchaChallenge` event via the event channel
/// 3. WebSocket sends the challenge to the Web UI
/// 4. User sees captcha image, types solution, POSTs to `/api/v1/captcha/{id}/solve`
/// 5. REST handler calls `submit_solution()` → oneshot channel delivers answer to plugin
#[derive(Clone)]
pub struct CaptchaManager {
    pending: Arc<Mutex<HashMap<String, PendingCaptcha>>>,
    event_tx: broadcast::Sender<DownloadEvent>,
    timeout: Duration,
}

impl CaptchaManager {
    pub fn new(event_tx: broadcast::Sender<DownloadEvent>, config: &CaptchaConfig) -> Self {
        Self {
            pending: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
            timeout: Duration::from_secs(config.timeout_secs),
        }
    }

    /// Request a captcha solution from the user. Blocks until solved or timeout.
    pub async fn request_solve(
        &self,
        plugin_id: &str,
        download_id: &str,
        image_url: &str,
        captcha_type: &str,
    ) -> Result<String, CaptchaError> {
        let captcha_id = Uuid::new_v4().to_string();
        let (answer_tx, answer_rx) = oneshot::channel();

        info!(
            "Captcha requested: {} (plugin={}, type={})",
            captcha_id, plugin_id, captcha_type
        );

        // Store pending captcha
        {
            let mut pending = self.pending.lock().await;
            pending.insert(
                captcha_id.clone(),
                PendingCaptcha {
                    answer_tx,
                    created_at: Instant::now(),
                    plugin_id: plugin_id.to_string(),
                    download_id: download_id.to_string(),
                    image_url: image_url.to_string(),
                    captcha_type: captcha_type.to_string(),
                },
            );
        }

        // Broadcast challenge to UI
        let _ = self.event_tx.send(DownloadEvent::CaptchaChallenge {
            id: captcha_id.clone(),
            plugin_id: plugin_id.to_string(),
            download_id: download_id.to_string(),
            image_url: image_url.to_string(),
            captcha_type: captcha_type.to_string(),
        });

        // Wait for answer with timeout
        let result = tokio::time::timeout(self.timeout, answer_rx).await;

        // Clean up
        self.pending.lock().await.remove(&captcha_id);

        match result {
            Ok(Ok(CaptchaAnswer::Solved(answer))) => {
                debug!("Captcha solved: {captcha_id}");
                let _ = self.event_tx.send(DownloadEvent::CaptchaSolved {
                    id: captcha_id,
                });
                Ok(answer)
            }
            Ok(Ok(CaptchaAnswer::Cancelled)) => {
                debug!("Captcha cancelled: {captcha_id}");
                Err(CaptchaError::Cancelled)
            }
            Ok(Err(_)) => {
                // Channel closed — shouldn't happen normally
                warn!("Captcha channel closed unexpectedly: {captcha_id}");
                Err(CaptchaError::Cancelled)
            }
            Err(_) => {
                warn!("Captcha timed out: {captcha_id}");
                let _ = self.event_tx.send(DownloadEvent::CaptchaTimeout {
                    id: captcha_id,
                });
                Err(CaptchaError::Timeout(self.timeout.as_secs()))
            }
        }
    }

    /// Submit a captcha solution (called from REST API).
    pub async fn submit_solution(
        &self,
        captcha_id: &str,
        answer: &str,
    ) -> Result<(), CaptchaError> {
        let mut pending = self.pending.lock().await;
        let captcha = pending
            .remove(captcha_id)
            .ok_or_else(|| CaptchaError::NotFound(captcha_id.to_string()))?;

        captcha
            .answer_tx
            .send(CaptchaAnswer::Solved(answer.to_string()))
            .map_err(|_| CaptchaError::AlreadySolved(captcha_id.to_string()))?;

        Ok(())
    }

    /// Cancel a captcha challenge (called from REST API).
    pub async fn cancel(&self, captcha_id: &str) -> Result<(), CaptchaError> {
        let mut pending = self.pending.lock().await;
        let captcha = pending
            .remove(captcha_id)
            .ok_or_else(|| CaptchaError::NotFound(captcha_id.to_string()))?;

        let _ = captcha.answer_tx.send(CaptchaAnswer::Cancelled);
        Ok(())
    }

    /// List all pending captchas (for REST API).
    pub async fn list_pending(&self) -> Vec<CaptchaInfo> {
        let pending = self.pending.lock().await;
        pending
            .iter()
            .map(|(id, c)| CaptchaInfo {
                id: id.clone(),
                plugin_id: c.plugin_id.clone(),
                download_id: c.download_id.clone(),
                image_url: c.image_url.clone(),
                captcha_type: c.captcha_type.clone(),
                elapsed_secs: c.created_at.elapsed().as_secs(),
                timeout_secs: self.timeout.as_secs(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_captcha_solve_flow() {
        let (event_tx, _event_rx) = broadcast::channel(16);
        let manager = CaptchaManager::new(event_tx, &CaptchaConfig { timeout_secs: 5 });

        let mgr = manager.clone();
        let solve_task = tokio::spawn(async move {
            mgr.request_solve("test-plugin", "dl-1", "https://img/captcha.png", "image")
                .await
        });

        // Give the request time to register
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Get the pending captcha
        let pending = manager.list_pending().await;
        assert_eq!(pending.len(), 1);
        let captcha_id = &pending[0].id;

        // Submit solution
        manager.submit_solution(captcha_id, "ABC123").await.unwrap();

        let result = solve_task.await.unwrap().unwrap();
        assert_eq!(result, "ABC123");
    }

    #[tokio::test]
    async fn test_captcha_cancel() {
        let (event_tx, _event_rx) = broadcast::channel(16);
        let manager = CaptchaManager::new(event_tx, &CaptchaConfig { timeout_secs: 5 });

        let mgr = manager.clone();
        let solve_task = tokio::spawn(async move {
            mgr.request_solve("test-plugin", "dl-1", "https://img/captcha.png", "image")
                .await
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let pending = manager.list_pending().await;
        manager.cancel(&pending[0].id).await.unwrap();

        let result = solve_task.await.unwrap();
        assert!(matches!(result, Err(CaptchaError::Cancelled)));
    }

    #[tokio::test]
    async fn test_captcha_timeout() {
        let (event_tx, _event_rx) = broadcast::channel(16);
        let manager = CaptchaManager::new(
            event_tx,
            &CaptchaConfig { timeout_secs: 1 }, // 1 second timeout for test
        );

        let result = manager
            .request_solve("test-plugin", "dl-1", "https://img/captcha.png", "image")
            .await;

        assert!(matches!(result, Err(CaptchaError::Timeout(1))));
    }
}
