//! Global and per-download bandwidth scheduling using a token bucket algorithm.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthConfig {
    /// Global limit in bytes/s, 0 = unlimited.
    pub global_limit: u64,
    /// Per-protocol limits (0 = unlimited).
    pub http_limit: u64,
    pub usenet_limit: u64,
    pub torrent_limit: u64,
    /// Time-based scheduling enabled.
    pub schedule_enabled: bool,
}

impl Default for BandwidthConfig {
    fn default() -> Self {
        Self {
            global_limit: 0,
            http_limit: 0,
            usenet_limit: 0,
            torrent_limit: 0,
            schedule_enabled: false,
        }
    }
}

/// Token-bucket based bandwidth limiter.
#[derive(Clone)]
pub struct BandwidthLimiter {
    inner: Arc<Mutex<BucketState>>,
    config: BandwidthConfig,
}

struct BucketState {
    tokens: u64,
    last_refill: tokio::time::Instant,
}

impl BandwidthLimiter {
    pub fn new(config: BandwidthConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BucketState {
                tokens: config.global_limit.max(1024 * 1024), // Start with a full bucket
                last_refill: tokio::time::Instant::now(),
            })),
            config,
        }
    }

    /// Check if bandwidth limiting is active.
    pub fn is_limited(&self) -> bool {
        self.config.global_limit > 0
    }

    /// Request `bytes` worth of bandwidth. Returns when the tokens are available.
    /// If unlimited, returns immediately.
    pub async fn acquire(&self, bytes: u64) {
        if !self.is_limited() {
            return;
        }

        let limit = self.config.global_limit;
        loop {
            {
                let mut state = self.inner.lock().await;
                // Refill tokens based on elapsed time
                let now = tokio::time::Instant::now();
                let elapsed = now.duration_since(state.last_refill);
                let new_tokens = (elapsed.as_secs_f64() * limit as f64) as u64;
                if new_tokens > 0 {
                    state.tokens = (state.tokens + new_tokens).min(limit * 2); // Max burst = 2x limit
                    state.last_refill = now;
                }

                if state.tokens >= bytes {
                    state.tokens -= bytes;
                    return;
                }
            }
            // Not enough tokens, wait a bit
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }

    /// Update the bandwidth config at runtime.
    pub async fn update_config(&mut self, config: BandwidthConfig) {
        self.config = config;
    }
}
