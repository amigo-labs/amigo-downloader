//! Global and per-download bandwidth scheduling using a token bucket algorithm.
//!
//! Supports time-based schedules (e.g. full speed at night, limited during day).

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct BandwidthConfig {
    /// Global limit in bytes/s, 0 = unlimited.
    pub global_limit: u64,
    /// Per-protocol limits (0 = unlimited).
    pub http_limit: u64,
    pub usenet_limit: u64,
    /// Time-based scheduling enabled.
    pub schedule_enabled: bool,
    /// Schedule rules (checked in order, first match wins).
    #[serde(default)]
    pub schedules: Vec<BandwidthSchedule>,
}

/// A time-based bandwidth schedule rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthSchedule {
    /// Human-readable name (e.g. "Night", "Work hours").
    pub name: String,
    /// Start time in HH:MM format (24h).
    pub start: String,
    /// End time in HH:MM format (24h).
    pub end: String,
    /// Bandwidth limit during this period (bytes/s, 0 = unlimited).
    pub limit: u64,
}


/// Token-bucket based bandwidth limiter with time-based scheduling.
#[derive(Clone)]
pub struct BandwidthLimiter {
    inner: Arc<Mutex<BucketState>>,
    config: Arc<Mutex<BandwidthConfig>>,
}

struct BucketState {
    tokens: u64,
    last_refill: tokio::time::Instant,
}

impl BandwidthLimiter {
    pub fn new(config: BandwidthConfig) -> Self {
        let initial_tokens = config.global_limit.max(1024 * 1024);
        Self {
            inner: Arc::new(Mutex::new(BucketState {
                tokens: initial_tokens,
                last_refill: tokio::time::Instant::now(),
            })),
            config: Arc::new(Mutex::new(config)),
        }
    }

    /// Get the current effective limit, considering time-based schedules.
    pub async fn effective_limit(&self) -> u64 {
        let config = self.config.lock().await;

        if config.schedule_enabled && !config.schedules.is_empty() {
            let now = chrono::Local::now();
            let current_minutes = now.hour() * 60 + now.minute();

            for schedule in &config.schedules {
                if let (Some(start), Some(end)) =
                    (parse_time(&schedule.start), parse_time(&schedule.end))
                {
                    let in_range = if start <= end {
                        current_minutes >= start && current_minutes < end
                    } else {
                        // Wraps midnight (e.g. 23:00 - 07:00)
                        current_minutes >= start || current_minutes < end
                    };

                    if in_range {
                        return schedule.limit;
                    }
                }
            }
        }

        config.global_limit
    }

    /// Check if bandwidth limiting is active right now.
    pub async fn is_limited(&self) -> bool {
        self.effective_limit().await > 0
    }

    /// Request `bytes` worth of bandwidth. Returns when the tokens are available.
    pub async fn acquire(&self, bytes: u64) {
        let limit = self.effective_limit().await;
        if limit == 0 {
            return; // Unlimited
        }

        loop {
            {
                let mut state = self.inner.lock().await;
                let now = tokio::time::Instant::now();
                let elapsed = now.duration_since(state.last_refill);
                let new_tokens = (elapsed.as_secs_f64() * limit as f64) as u64;
                if new_tokens > 0 {
                    state.tokens = (state.tokens + new_tokens).min(limit * 2);
                    state.last_refill = now;
                }

                if state.tokens >= bytes {
                    state.tokens -= bytes;
                    return;
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }

    /// Update the bandwidth config at runtime.
    pub async fn update_config(&self, config: BandwidthConfig) {
        *self.config.lock().await = config;
    }
}

/// Parse "HH:MM" to minutes since midnight.
fn parse_time(s: &str) -> Option<u32> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let h = parts[0].parse::<u32>().ok()?;
    let m = parts[1].parse::<u32>().ok()?;
    if h < 24 && m < 60 {
        Some(h * 60 + m)
    } else {
        None
    }
}

use chrono::Timelike;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_time() {
        assert_eq!(parse_time("00:00"), Some(0));
        assert_eq!(parse_time("07:30"), Some(450));
        assert_eq!(parse_time("23:59"), Some(1439));
        assert_eq!(parse_time("24:00"), None);
        assert_eq!(parse_time("invalid"), None);
    }

    #[test]
    fn test_default_config_unlimited() {
        let config = BandwidthConfig::default();
        assert_eq!(config.global_limit, 0);
        assert!(!config.schedule_enabled);
        assert!(config.schedules.is_empty());
    }

    #[tokio::test]
    async fn test_limiter_unlimited_returns_immediately() {
        let limiter = BandwidthLimiter::new(BandwidthConfig::default());
        // Should return instantly (no blocking)
        limiter.acquire(1_000_000).await;
    }

    #[test]
    fn test_schedule_config_serde() {
        let config = BandwidthConfig {
            global_limit: 5_000_000,
            http_limit: 0,
            usenet_limit: 0,
            schedule_enabled: true,
            schedules: vec![
                BandwidthSchedule {
                    name: "Night".into(),
                    start: "01:00".into(),
                    end: "07:00".into(),
                    limit: 0, // unlimited at night
                },
                BandwidthSchedule {
                    name: "Day".into(),
                    start: "07:00".into(),
                    end: "01:00".into(),
                    limit: 5_000_000, // 5 MB/s during day
                },
            ],
        };

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: BandwidthConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.schedules.len(), 2);
        assert_eq!(parsed.schedules[0].name, "Night");
        assert_eq!(parsed.schedules[1].limit, 5_000_000);
    }
}
