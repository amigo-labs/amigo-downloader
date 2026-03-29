//! Retry strategies with exponential backoff and jitter.

use std::future::Future;
use std::time::Duration;

use rand::Rng;
use tracing::warn;

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

impl From<crate::config::RetryConfig> for RetryPolicy {
    fn from(cfg: crate::config::RetryConfig) -> Self {
        Self {
            max_retries: cfg.max_retries,
            base_delay: Duration::from_secs_f64(cfg.base_delay_secs),
            max_delay: Duration::from_secs_f64(cfg.max_delay_secs),
        }
    }
}

impl RetryPolicy {
    /// Calculate delay for attempt N with exponential backoff + jitter.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base = self.base_delay.as_millis() as u64;
        let exp_delay = base.saturating_mul(2u64.saturating_pow(attempt));
        let max = self.max_delay.as_millis() as u64;
        let capped = exp_delay.min(max);

        // Add ±25% jitter to prevent thundering herd
        let jitter_range = (capped / 4).max(1);
        let mut rng = rand::thread_rng();
        let jitter = rng.gen_range(0..=jitter_range * 2) as i64 - jitter_range as i64;
        let with_jitter = (capped as i64 + jitter).max(0) as u64;

        Duration::from_millis(with_jitter)
    }
}

/// Outcome of a retryable operation — tells the retry loop whether to keep retrying.
pub enum RetryOutcome<T> {
    /// Operation succeeded.
    Success(T),
    /// Retryable failure — will retry if attempts remain.
    Retry(crate::Error),
    /// Non-retryable failure — abort immediately.
    Abort(crate::Error),
}

/// Execute an async operation with retries according to the given policy.
///
/// The closure receives the current attempt number (0-based) and returns a `RetryOutcome`.
pub async fn retry_with_policy<F, Fut, T>(
    policy: &RetryPolicy,
    mut f: F,
) -> Result<T, crate::Error>
where
    F: FnMut(u32) -> Fut,
    Fut: Future<Output = RetryOutcome<T>>,
{
    let mut last_error = None;

    for attempt in 0..=policy.max_retries {
        if attempt > 0 {
            let delay = policy.delay_for_attempt(attempt - 1);
            warn!("Retrying (attempt {attempt}/{}) after {delay:?}", policy.max_retries);
            tokio::time::sleep(delay).await;
        }

        match f(attempt).await {
            RetryOutcome::Success(value) => return Ok(value),
            RetryOutcome::Abort(e) => return Err(e),
            RetryOutcome::Retry(e) => {
                warn!("Attempt {attempt} failed: {e}");
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| crate::Error::Other("All retries exhausted".into())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delay_increases_exponentially() {
        let policy = RetryPolicy {
            max_retries: 5,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
        };

        // With jitter the exact value varies, but the magnitude should grow
        let d0 = policy.delay_for_attempt(0);
        let d3 = policy.delay_for_attempt(3);
        // attempt 0 ~ 1s ±25%, attempt 3 ~ 8s ±25%
        assert!(d0.as_millis() <= 1500);
        assert!(d3.as_millis() >= 4000);
    }

    #[test]
    fn test_delay_capped_at_max() {
        let policy = RetryPolicy {
            max_retries: 10,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(5),
        };

        // Even with high attempt, delay shouldn't exceed max + jitter
        let d = policy.delay_for_attempt(20);
        // max is 5000ms, +25% jitter = 6250ms absolute max
        assert!(d.as_millis() <= 6500);
    }

    #[tokio::test]
    async fn test_retry_with_policy_success_first_try() {
        let policy = RetryPolicy {
            max_retries: 3,
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
        };

        let result = retry_with_policy(&policy, |_attempt| async {
            RetryOutcome::Success(42)
        }).await;

        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_with_policy_succeeds_after_retries() {
        let policy = RetryPolicy {
            max_retries: 3,
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
        };

        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let c = counter.clone();

        let result = retry_with_policy(&policy, move |_attempt| {
            let c = c.clone();
            async move {
                let n = c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if n < 2 {
                    RetryOutcome::Retry(crate::Error::Other("not yet".into()))
                } else {
                    RetryOutcome::Success("done")
                }
            }
        }).await;

        assert_eq!(result.unwrap(), "done");
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_with_policy_abort_stops_immediately() {
        let policy = RetryPolicy {
            max_retries: 5,
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
        };

        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let c = counter.clone();

        let result: Result<(), _> = retry_with_policy(&policy, move |_attempt| {
            let c = c.clone();
            async move {
                c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                RetryOutcome::Abort(crate::Error::Other("fatal".into()))
            }
        }).await;

        assert!(result.is_err());
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_with_policy_exhausts_retries() {
        let policy = RetryPolicy {
            max_retries: 2,
            base_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
        };

        let result: Result<(), _> = retry_with_policy(&policy, |_attempt| async {
            RetryOutcome::Retry(crate::Error::Other("always fails".into()))
        }).await;

        assert!(result.is_err());
    }
}
