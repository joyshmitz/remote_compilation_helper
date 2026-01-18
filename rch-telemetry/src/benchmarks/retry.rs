//! Retry utilities for benchmark execution.
//!
//! Provides a generic retry policy with exponential backoff and jitter,
//! and a helper to run benchmark phases with retryable errors.

use std::future::Future;
use std::time::Duration;

use tokio::time::sleep;
use tracing::{debug, info, warn};

/// Errors that can be retried.
pub trait RetryableError {
    /// Whether this error should be retried immediately.
    fn is_retryable(&self) -> bool;
}

/// Retry policy for benchmark execution.
#[derive(Debug, Clone)]
pub struct BenchmarkRetryPolicy {
    /// Maximum attempts including the first try (minimum 1).
    pub max_retries: u32,
    /// Base delay between retries (exponential backoff).
    pub base_delay: Duration,
    /// Maximum delay between retries.
    pub max_delay: Duration,
    /// Jitter factor (0.0-1.0) applied to delay.
    pub jitter: f64,
}

impl Default for BenchmarkRetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_secs(5),
            max_delay: Duration::from_secs(60),
            jitter: 0.2,
        }
    }
}

impl BenchmarkRetryPolicy {
    /// Calculate backoff delay for a given attempt (1-based).
    pub fn backoff_delay(&self, attempt: u32) -> Duration {
        let attempt = attempt.max(1);
        let base_secs = self.base_delay.as_secs_f64();
        let max_secs = self.max_delay.as_secs_f64().max(0.0);

        let multiplier = 2_u32.saturating_pow(attempt.saturating_sub(1)) as f64;
        let mut delay = (base_secs * multiplier).min(max_secs);

        if self.jitter > 0.0 && delay > 0.0 {
            let jitter = (fastrand::f64() * 2.0 - 1.0) * self.jitter;
            delay = (delay * (1.0 + jitter)).max(0.0);
        }

        Duration::from_secs_f64(delay)
    }

    fn max_attempts(&self) -> u32 {
        self.max_retries.max(1)
    }
}

/// Run an async operation with retries on retryable errors.
pub async fn run_with_retry<F, Fut, T, E>(
    phase: &str,
    policy: &BenchmarkRetryPolicy,
    mut op: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: RetryableError,
{
    let max_attempts = policy.max_attempts();
    let mut attempt = 1;

    loop {
        debug!(phase, attempt, max_attempts, "Starting benchmark attempt");

        match op().await {
            Ok(value) => {
                info!(phase, attempt, "Benchmark attempt succeeded");
                return Ok(value);
            }
            Err(err) if err.is_retryable() && attempt < max_attempts => {
                warn!(phase, attempt, "Benchmark attempt failed (retryable)");
                let delay = policy.backoff_delay(attempt);
                debug!(
                    phase,
                    attempt,
                    delay_secs = delay.as_secs_f64(),
                    "Retrying after backoff"
                );
                sleep(delay).await;
                attempt += 1;
            }
            Err(err) => {
                warn!(phase, attempt, "Benchmark attempt failed (non-retryable)");
                return Err(err);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[derive(Debug)]
    enum TestError {
        Retryable,
        Fatal,
    }

    impl std::fmt::Display for TestError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                TestError::Retryable => write!(f, "retryable"),
                TestError::Fatal => write!(f, "fatal"),
            }
        }
    }

    impl std::error::Error for TestError {}

    impl RetryableError for TestError {
        fn is_retryable(&self) -> bool {
            matches!(self, TestError::Retryable)
        }
    }

    #[tokio::test]
    async fn test_retry_succeeds_on_third_attempt() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let policy = BenchmarkRetryPolicy {
            max_retries: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(5),
            jitter: 0.0,
        };

        let result = run_with_retry("test", &policy, move || {
            let attempts_clone = attempts_clone.clone();
            async move {
                let count = attempts_clone.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(TestError::Retryable)
                } else {
                    Ok(42u32)
                }
            }
        })
        .await;

        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_non_retryable_error_fails_immediately() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let policy = BenchmarkRetryPolicy {
            max_retries: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(5),
            jitter: 0.0,
        };

        let result: Result<u32, TestError> = run_with_retry("test", &policy, move || {
            let attempts_clone = attempts_clone.clone();
            async move {
                attempts_clone.fetch_add(1, Ordering::SeqCst);
                Err(TestError::Fatal)
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }
}
