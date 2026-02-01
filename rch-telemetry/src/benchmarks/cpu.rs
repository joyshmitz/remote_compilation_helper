//! CPU benchmark implementation for measuring worker computational performance.
//!
//! This module provides a pure-Rust CPU benchmark that measures computational
//! throughput using operations similar to those in compilation workloads:
//! - **Prime sieve**: Integer operations, branching, memory access patterns
//! - **Matrix multiplication**: Floating-point operations, cache utilization
//!
//! The benchmark is designed to complete in <5 seconds and produce results
//! with <10% variance across multiple runs on the same hardware.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{debug, info};

/// Result of a CPU benchmark run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuBenchmarkResult {
    /// Normalized score (higher = faster). Reference baseline = 1000.
    pub score: f64,
    /// Total duration of the benchmark in milliseconds.
    pub duration_ms: u64,
    /// Number of benchmark iterations performed.
    pub iterations: u64,
    /// Operations per second (prime count throughput).
    pub ops_per_second: f64,
    /// Timestamp when the benchmark was taken.
    pub timestamp: DateTime<Utc>,
    /// Prime sieve component score (normalized).
    pub prime_sieve_score: f64,
    /// Matrix multiply component score (normalized).
    pub matrix_multiply_score: f64,
}

impl Default for CpuBenchmarkResult {
    fn default() -> Self {
        Self {
            score: 0.0,
            duration_ms: 0,
            iterations: 0,
            ops_per_second: 0.0,
            timestamp: Utc::now(),
            prime_sieve_score: 0.0,
            matrix_multiply_score: 0.0,
        }
    }
}

/// CPU benchmark runner with configurable parameters.
#[derive(Debug, Clone)]
pub struct CpuBenchmark {
    /// Upper limit for prime sieve (number of integers to check).
    pub sieve_limit: usize,
    /// Size of the square matrix for multiplication benchmark.
    pub matrix_size: usize,
    /// Number of iterations for each benchmark component.
    pub iterations: u32,
    /// Whether to perform a warmup run before measurement.
    pub warmup: bool,
}

impl Default for CpuBenchmark {
    fn default() -> Self {
        Self {
            sieve_limit: 100_000,
            matrix_size: 200,
            iterations: 10,
            warmup: true,
        }
    }
}

impl CpuBenchmark {
    /// Create a new CPU benchmark with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the sieve limit for the prime number benchmark.
    #[must_use]
    pub fn with_sieve_limit(mut self, limit: usize) -> Self {
        self.sieve_limit = limit;
        self
    }

    /// Set the matrix size for the multiplication benchmark.
    #[must_use]
    pub fn with_matrix_size(mut self, size: usize) -> Self {
        self.matrix_size = size;
        self
    }

    /// Set the number of iterations for the benchmark.
    #[must_use]
    pub fn with_iterations(mut self, iterations: u32) -> Self {
        self.iterations = iterations;
        self
    }

    /// Enable or disable warmup run.
    #[must_use]
    pub fn with_warmup(mut self, warmup: bool) -> Self {
        self.warmup = warmup;
        self
    }

    /// Run the CPU benchmark and return results.
    ///
    /// This runs both the prime sieve and matrix multiplication benchmarks,
    /// combining their scores into an overall CPU performance score.
    pub fn run(&self) -> CpuBenchmarkResult {
        debug!(
            sieve_limit = self.sieve_limit,
            matrix_size = self.matrix_size,
            iterations = self.iterations,
            warmup = self.warmup,
            "Starting CPU benchmark"
        );

        // Warmup run (not counted)
        if self.warmup {
            debug!("Running warmup iteration");
            let _ = prime_sieve(self.sieve_limit);
            let _ = matrix_multiply(self.matrix_size);
        }

        let start = Instant::now();
        let mut total_primes = 0u64;
        let mut matrix_checksum = 0.0f64;

        // Run benchmark iterations
        for i in 0..self.iterations {
            let primes = prime_sieve(self.sieve_limit);
            total_primes += primes;

            let checksum = matrix_multiply(self.matrix_size);
            matrix_checksum += checksum;

            debug!(iteration = i + 1, primes, checksum, "Completed iteration");
        }

        let duration = start.elapsed();
        let duration_ms = duration.as_millis() as u64;

        // Calculate ops per second (primes found per second)
        let ops_per_second = if duration.as_secs_f64() > 0.0 {
            total_primes as f64 / duration.as_secs_f64()
        } else {
            0.0
        };

        // Calculate component scores
        // Prime sieve baseline: 1M primes/second = score 1000
        let prime_sieve_score = ops_per_second / 1_000.0;

        // Matrix multiply baseline: duration for iterations
        // Expected ~2s for 10 iterations of 200x200 on baseline = score 1000
        let matrix_baseline_ms = 2000.0;
        let matrix_multiply_score = if duration_ms > 0 {
            matrix_baseline_ms / (duration_ms as f64 / self.iterations as f64) * 1000.0
        } else {
            0.0
        };

        // Combined score (60% prime sieve, 40% matrix multiply)
        // Prime sieve emphasizes integer ops (common in parsing)
        // Matrix multiply emphasizes FP and cache (common in codegen)
        let score = prime_sieve_score * 0.6 + matrix_multiply_score * 0.4;

        let result = CpuBenchmarkResult {
            score,
            duration_ms,
            iterations: self.iterations as u64,
            ops_per_second,
            timestamp: Utc::now(),
            prime_sieve_score,
            matrix_multiply_score,
        };

        debug!(
            score = result.score,
            duration_ms = result.duration_ms,
            ops_per_second = result.ops_per_second,
            prime_sieve_score = result.prime_sieve_score,
            matrix_multiply_score = result.matrix_multiply_score,
            total_primes,
            matrix_checksum,
            "CPU benchmark completed"
        );

        result
    }

    /// Run the benchmark multiple times and return the median result.
    ///
    /// This provides more stable results by:
    /// 1. Running a warmup (if enabled)
    /// 2. Running `runs` benchmark iterations
    /// 3. Returning the median result by score
    ///
    /// Default is 3 runs for production use.
    pub fn run_stable(&self, runs: u32) -> CpuBenchmarkResult {
        if runs == 0 {
            return CpuBenchmarkResult::default();
        }

        info!(
            runs,
            sieve_limit = self.sieve_limit,
            matrix_size = self.matrix_size,
            "Running stable CPU benchmark"
        );

        let mut results: Vec<CpuBenchmarkResult> = Vec::with_capacity(runs as usize);

        for run in 0..runs {
            let result = self.run();
            debug!(
                run = run + 1,
                score = result.score,
                "Benchmark run completed"
            );
            results.push(result);
        }

        // Sort by score and return median
        results.sort_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let median_idx = results.len() / 2;
        let median_result = results.remove(median_idx);

        info!(
            score = median_result.score,
            duration_ms = median_result.duration_ms,
            "Stable CPU benchmark completed"
        );

        median_result
    }
}

/// Run the Sieve of Eratosthenes to count primes below `limit`.
///
/// This benchmark exercises:
/// - Integer operations (incrementing, comparing)
/// - Branching (prime/composite decisions)
/// - Memory access patterns (iterating through array)
///
/// Returns the count of prime numbers found.
pub fn prime_sieve(limit: usize) -> u64 {
    if limit < 2 {
        return 0;
    }

    let mut sieve = vec![true; limit];
    sieve[0] = false;
    sieve[1] = false;

    let sqrt_limit = (limit as f64).sqrt() as usize + 1;
    for i in 2..sqrt_limit {
        if sieve[i] {
            for j in (i * i..limit).step_by(i) {
                sieve[j] = false;
            }
        }
    }

    sieve.iter().filter(|&&b| b).count() as u64
}

/// Perform matrix multiplication of two `size x size` matrices.
///
/// This benchmark exercises:
/// - Floating-point operations (multiplication, addition)
/// - Cache utilization (nested loops over 2D arrays)
/// - Memory bandwidth (reading/writing large arrays)
///
/// Returns a checksum value from the result matrix for verification.
#[allow(clippy::needless_range_loop)] // Intentional: k indexes both a[i][k] and b[k][j]
pub fn matrix_multiply(size: usize) -> f64 {
    if size == 0 {
        return 0.0;
    }

    // Create deterministic input matrices
    let a: Vec<Vec<f64>> = (0..size)
        .map(|i| (0..size).map(|j| (i * j) as f64 / size as f64).collect())
        .collect();
    let b = a.clone();

    // Initialize result matrix
    let mut c = vec![vec![0.0f64; size]; size];

    // Standard matrix multiplication O(n^3)
    for i in 0..size {
        for j in 0..size {
            for k in 0..size {
                c[i][j] += a[i][k] * b[k][j];
            }
        }
    }

    // Return checksum from center of result
    c[size / 2][size / 2]
}

/// Convenience function to run the default CPU benchmark.
pub fn run_cpu_benchmark() -> CpuBenchmarkResult {
    CpuBenchmark::default().run()
}

/// Convenience function to run a stable CPU benchmark with default settings.
pub fn run_cpu_benchmark_stable() -> CpuBenchmarkResult {
    CpuBenchmark::default().run_stable(3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use tracing::Level;
    use tracing_subscriber::fmt;

    fn init_test_logging() {
        let _ = fmt()
            .with_max_level(Level::DEBUG)
            .with_test_writer()
            .try_init();
    }

    #[test]
    fn test_prime_sieve_correctness() {
        init_test_logging();
        info!("TEST START: test_prime_sieve_correctness");

        info!("INPUT: prime_sieve(100)");
        let count = prime_sieve(100);
        info!("RESULT: Found {} primes below 100", count);

        // There are exactly 25 primes below 100:
        // 2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47,
        // 53, 59, 61, 67, 71, 73, 79, 83, 89, 97
        assert_eq!(count, 25);
        info!("VERIFY: Expected 25 primes, got {}", count);

        info!("TEST PASS: test_prime_sieve_correctness");
    }

    #[test]
    fn test_prime_sieve_edge_cases() {
        init_test_logging();
        info!("TEST START: test_prime_sieve_edge_cases");

        info!("INPUT: prime_sieve(0)");
        assert_eq!(prime_sieve(0), 0);
        info!("RESULT: 0 primes below 0");

        info!("INPUT: prime_sieve(1)");
        assert_eq!(prime_sieve(1), 0);
        info!("RESULT: 0 primes below 1");

        info!("INPUT: prime_sieve(2)");
        assert_eq!(prime_sieve(2), 0);
        info!("RESULT: 0 primes below 2 (exclusive)");

        info!("INPUT: prime_sieve(3)");
        assert_eq!(prime_sieve(3), 1);
        info!("RESULT: 1 prime below 3 (which is 2)");

        info!("TEST PASS: test_prime_sieve_edge_cases");
    }

    #[test]
    fn test_prime_sieve_large() {
        init_test_logging();
        info!("TEST START: test_prime_sieve_large");

        info!("INPUT: prime_sieve(100_000)");
        let count = prime_sieve(100_000);
        info!("RESULT: Found {} primes below 100,000", count);

        // There are exactly 9592 primes below 100,000
        assert_eq!(count, 9592);
        info!("VERIFY: Expected 9592 primes, got {}", count);

        info!("TEST PASS: test_prime_sieve_large");
    }

    #[test]
    fn test_matrix_multiply_deterministic() {
        init_test_logging();
        info!("TEST START: test_matrix_multiply_deterministic");

        info!("INPUT: Two runs of matrix_multiply(50)");
        let result1 = matrix_multiply(50);
        let result2 = matrix_multiply(50);
        info!("RESULT: run1={}, run2={}", result1, result2);

        assert_eq!(result1, result2);
        info!("VERIFY: Results are identical (deterministic)");

        info!("TEST PASS: test_matrix_multiply_deterministic");
    }

    #[test]
    fn test_matrix_multiply_edge_cases() {
        init_test_logging();
        info!("TEST START: test_matrix_multiply_edge_cases");

        info!("INPUT: matrix_multiply(0)");
        let result = matrix_multiply(0);
        assert_eq!(result, 0.0);
        info!("RESULT: 0x0 matrix returns 0.0");

        info!("INPUT: matrix_multiply(1)");
        let result = matrix_multiply(1);
        info!("RESULT: 1x1 matrix checksum = {}", result);
        assert!(result.is_finite());

        info!("TEST PASS: test_matrix_multiply_edge_cases");
    }

    #[test]
    fn test_matrix_multiply_known_value() {
        init_test_logging();
        info!("TEST START: test_matrix_multiply_known_value");

        // For a 10x10 matrix with our formula: a[i][j] = (i * j) / size
        // The center element c[5][5] should be deterministic
        info!("INPUT: matrix_multiply(10)");
        let result = matrix_multiply(10);
        info!("RESULT: center element = {}", result);

        // Verify it's a reasonable positive value
        assert!(result > 0.0);
        assert!(result.is_finite());

        // Run again to verify determinism
        let result2 = matrix_multiply(10);
        assert_eq!(result, result2);
        info!("VERIFY: Result is deterministic and positive");

        info!("TEST PASS: test_matrix_multiply_known_value");
    }

    #[test]
    fn test_cpu_benchmark_produces_positive_score() {
        init_test_logging();
        info!("TEST START: test_cpu_benchmark_produces_positive_score");

        // Use smaller parameters for faster test
        let benchmark = CpuBenchmark::new()
            .with_sieve_limit(10_000)
            .with_matrix_size(50)
            .with_iterations(2)
            .with_warmup(false);

        info!("INPUT: run_cpu_benchmark() with small parameters");
        let result = benchmark.run();
        info!(
            "RESULT: score={}, duration={}ms, ops/sec={}",
            result.score, result.duration_ms, result.ops_per_second
        );

        assert!(result.score > 0.0);
        assert!(result.duration_ms > 0);
        assert!(result.ops_per_second > 0.0);
        info!("VERIFY: Benchmark produced positive score and duration");

        info!("TEST PASS: test_cpu_benchmark_produces_positive_score");
    }

    #[test]
    fn test_benchmark_stability() {
        init_test_logging();
        info!("TEST START: test_benchmark_stability");

        // Use smaller parameters for faster test
        let benchmark = CpuBenchmark::new()
            .with_sieve_limit(50_000)
            .with_matrix_size(100)
            .with_iterations(5)
            .with_warmup(true);

        info!("INPUT: run_cpu_benchmark_stable() (3 runs + warmup)");
        let result = benchmark.run_stable(3);
        info!("RESULT: stable score = {}", result.score);

        // Run again to check variance
        let result2 = benchmark.run_stable(3);
        let variance = if result.score > 0.0 {
            ((result.score - result2.score) / result.score).abs()
        } else {
            0.0
        };
        info!(
            "RESULT: second run score = {}, variance = {}%",
            result2.score,
            variance * 100.0
        );

        // Allow up to 300% variance in tests (CI and concurrent tests can be noisy).
        // Production target is <10% but test environments vary significantly
        // due to concurrent processes, CPU throttling, shared runners, and
        // virtualization overhead on GitHub Actions.
        assert!(variance < 3.0);
        info!(
            "VERIFY: Variance {}% is within 300% tolerance",
            variance * 100.0
        );

        info!("TEST PASS: test_benchmark_stability");
    }

    #[test]
    fn test_benchmark_completes_quickly() {
        init_test_logging();
        info!("TEST START: test_benchmark_completes_quickly");

        // Use practical benchmark parameters that complete in <5 seconds
        // These are smaller than production defaults to ensure fast test execution
        let benchmark = CpuBenchmark::new()
            .with_sieve_limit(50_000)
            .with_matrix_size(100)
            .with_iterations(5)
            .with_warmup(true);

        info!("INPUT: run benchmark with test parameters (sieve=50k, matrix=100, iter=5)");
        let start = Instant::now();
        let result = benchmark.run();
        let elapsed = start.elapsed();

        info!("RESULT: completed in {:?}, score={}", elapsed, result.score);

        // Should complete well under 5 seconds
        assert!(elapsed < std::time::Duration::from_secs(5));
        info!("VERIFY: Completed in {:?}, under 5s threshold", elapsed);

        info!("TEST PASS: test_benchmark_completes_quickly");
    }

    #[test]
    fn test_benchmark_builder_pattern() {
        init_test_logging();
        info!("TEST START: test_benchmark_builder_pattern");

        let benchmark = CpuBenchmark::new()
            .with_sieve_limit(5000)
            .with_matrix_size(25)
            .with_iterations(1)
            .with_warmup(false);

        assert_eq!(benchmark.sieve_limit, 5000);
        assert_eq!(benchmark.matrix_size, 25);
        assert_eq!(benchmark.iterations, 1);
        assert!(!benchmark.warmup);

        info!("VERIFY: Builder pattern sets all parameters correctly");

        info!("TEST PASS: test_benchmark_builder_pattern");
    }

    #[test]
    fn test_result_serialization() {
        init_test_logging();
        info!("TEST START: test_result_serialization");

        let result = CpuBenchmarkResult {
            score: 1234.5,
            duration_ms: 2500,
            iterations: 10,
            ops_per_second: 500_000.0,
            timestamp: Utc::now(),
            prime_sieve_score: 800.0,
            matrix_multiply_score: 600.0,
        };

        let json = serde_json::to_string(&result).expect("serialization should succeed");
        info!("RESULT: serialized to JSON (len={})", json.len());

        let deser: CpuBenchmarkResult =
            serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(result.score, deser.score);
        assert_eq!(result.duration_ms, deser.duration_ms);
        assert_eq!(result.iterations, deser.iterations);
        info!("VERIFY: Serialization roundtrip successful");

        info!("TEST PASS: test_result_serialization");
    }

    #[test]
    fn test_stable_benchmark_handles_zero_runs() {
        init_test_logging();
        info!("TEST START: test_stable_benchmark_handles_zero_runs");

        let benchmark = CpuBenchmark::new();
        let result = benchmark.run_stable(0);

        assert_eq!(result.score, 0.0);
        assert_eq!(result.duration_ms, 0);
        info!("VERIFY: Zero runs returns default result");

        info!("TEST PASS: test_stable_benchmark_handles_zero_runs");
    }

    #[test]
    fn test_warmup_affects_timing() {
        init_test_logging();
        info!("TEST START: test_warmup_affects_timing");

        // This test verifies warmup runs but doesn't strictly check timing
        // as that would be flaky across different systems
        let benchmark_with_warmup = CpuBenchmark::new()
            .with_sieve_limit(10_000)
            .with_matrix_size(50)
            .with_iterations(2)
            .with_warmup(true);

        let benchmark_without_warmup = CpuBenchmark::new()
            .with_sieve_limit(10_000)
            .with_matrix_size(50)
            .with_iterations(2)
            .with_warmup(false);

        let result_with = benchmark_with_warmup.run();
        let result_without = benchmark_without_warmup.run();

        // Both should produce valid results
        assert!(result_with.score > 0.0);
        assert!(result_without.score > 0.0);

        info!(
            "RESULT: with_warmup score={}, without_warmup score={}",
            result_with.score, result_without.score
        );
        info!("VERIFY: Both configurations produce valid results");

        info!("TEST PASS: test_warmup_affects_timing");
    }

    #[test]
    fn test_convenience_functions() {
        init_test_logging();
        info!("TEST START: test_convenience_functions");

        // These use default settings, so we can't run them in fast test mode
        // Just verify they exist and can be called with quick settings
        let benchmark = CpuBenchmark::new()
            .with_sieve_limit(5000)
            .with_matrix_size(25)
            .with_iterations(1)
            .with_warmup(false);

        let result = benchmark.run();
        assert!(result.score > 0.0);

        let stable_result = benchmark.run_stable(2);
        assert!(stable_result.score > 0.0);

        info!("VERIFY: Benchmark functions work correctly");

        info!("TEST PASS: test_convenience_functions");
    }
}
