//! Common benchmark error classification.
//!
//! Provides a single error type that captures common benchmark failures and
//! exposes retry/reschedule semantics for higher-level orchestration.

use crate::benchmarks::retry::RetryableError;

/// Errors that can occur while running benchmarks.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BenchmarkError {
    #[error("SSH connection failed: {0}")]
    SshConnection(String),

    #[error("SSH timeout after {0}s")]
    SshTimeout(u64),

    #[error("Worker resource exhausted: {0}")]
    ResourceExhausted(String),

    #[error("Disk full on worker")]
    DiskFull,

    #[error("Compilation failed: {0}")]
    CompilationFailed(String),

    #[error("Network benchmark failed: {0}")]
    NetworkBenchmarkFailed(String),

    #[error("Invalid benchmark result: {0}")]
    InvalidResult(String),

    #[error("Worker unreachable")]
    WorkerUnreachable,

    #[error("Cancelled by user")]
    Cancelled,
}

impl BenchmarkError {
    /// Whether this error should be retried immediately.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::SshConnection(_)
                | Self::SshTimeout(_)
                | Self::NetworkBenchmarkFailed(_)
                | Self::WorkerUnreachable
        )
    }

    /// Whether this error should be rescheduled for a later attempt.
    pub fn should_reschedule(&self) -> bool {
        matches!(self, Self::ResourceExhausted(_) | Self::WorkerUnreachable)
    }
}

impl RetryableError for BenchmarkError {
    fn is_retryable(&self) -> bool {
        BenchmarkError::is_retryable(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retryable_classification() {
        assert!(BenchmarkError::SshConnection("oops".into()).is_retryable());
        assert!(BenchmarkError::SshTimeout(30).is_retryable());
        assert!(!BenchmarkError::DiskFull.is_retryable());
        assert!(!BenchmarkError::CompilationFailed("fail".into()).is_retryable());
    }

    #[test]
    fn test_should_reschedule() {
        assert!(BenchmarkError::ResourceExhausted("busy".into()).should_reschedule());
        assert!(BenchmarkError::WorkerUnreachable.should_reschedule());
        assert!(!BenchmarkError::SshTimeout(5).should_reschedule());
    }
}
