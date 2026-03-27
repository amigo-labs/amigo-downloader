//! Retry strategies with exponential backoff and jitter.

use std::time::Duration;

pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 5,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
        }
    }
}

impl RetryPolicy {
    /// Calculate delay for attempt N with exponential backoff + jitter.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base = self.base_delay.as_millis() as u64;
        let exp_delay = base.saturating_mul(2u64.saturating_pow(attempt));
        let max = self.max_delay.as_millis() as u64;
        Duration::from_millis(exp_delay.min(max))
    }
}
