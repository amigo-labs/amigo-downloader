//! Global and per-download bandwidth scheduling.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthConfig {
    /// Global limit in bytes/s, 0 = unlimited
    pub global_limit: u64,
    /// Per-protocol limits
    pub http_limit: u64,
    pub usenet_limit: u64,
    pub torrent_limit: u64,
    /// Time-based scheduling enabled
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

pub struct BandwidthLimiter {
    config: BandwidthConfig,
}

impl BandwidthLimiter {
    pub fn new(config: BandwidthConfig) -> Self {
        Self { config }
    }

    /// Returns the allowed bytes/s for a given protocol right now.
    pub fn allowed_rate(&self, _protocol: &str) -> u64 {
        if self.config.global_limit == 0 {
            return u64::MAX;
        }
        self.config.global_limit
    }
}
