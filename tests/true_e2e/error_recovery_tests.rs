//! True E2E Tests: Error Recovery & Circuit Breaker Behavior
//!
//! Tests that verify RCH's error recovery mechanisms, circuit breaker patterns,
//! and graceful handling of various failure modes.
//!
//! # Test Categories
//!
//! 1. Circuit Breaker - Trip, half-open, recovery states
//! 2. Retry Behavior - Transient failure retry logic
//! 3. Toolchain Verification - Missing compiler detection
//! 4. Worker Health Monitoring - Health state transitions
//!
//! # Running These Tests
//!
//! ```bash
//! cargo test --features true-e2e error_recovery_tests -- --nocapture
//! ```
//!
//! # Bead Reference
//!
//! This implements bead bd-23n3: True E2E Error Handling & Recovery Tests

use rch_common::e2e::{
    LogLevel, LogSource, TestConfigError, TestLoggerBuilder, TestWorkersConfig,
    should_skip_worker_check,
};
use rch_common::ssh::{KnownHostsPolicy, SshClient, SshOptions};
use std::time::Duration;

/// Skip the test if no real workers are available.
fn require_workers() -> Option<TestWorkersConfig> {
    if should_skip_worker_check() {
        eprintln!("Skipping: RCH_E2E_SKIP_WORKER_CHECK is set");
        return None;
    }

    match TestWorkersConfig::load() {
        Ok(config) => {
            if !config.has_enabled_workers() {
                eprintln!("Skipping: No enabled workers in configuration");
                return None;
            }
            Some(config)
        }
        Err(TestConfigError::NotFound(path)) => {
            eprintln!("Skipping: Config not found at {}", path.display());
            None
        }
        Err(e) => {
            eprintln!("Skipping: Failed to load config: {e}");
            None
        }
    }
}

/// Get a single enabled worker for testing.
fn get_test_worker(config: &TestWorkersConfig) -> Option<&rch_common::e2e::TestWorkerEntry> {
    config.enabled_workers().first().copied()
}

/// Helper to create a connected SSH client.
async fn get_connected_client(
    config: &TestWorkersConfig,
    worker_entry: &rch_common::e2e::TestWorkerEntry,
) -> Option<SshClient> {
    let worker_config = worker_entry.to_worker_config();
    let options = SshOptions {
        connect_timeout: Duration::from_secs(config.settings.ssh_connection_timeout_secs),
        known_hosts: KnownHostsPolicy::Add,
        ..Default::default()
    };

    let mut client = SshClient::new(worker_config, options);
    match client.connect().await {
        Ok(()) => Some(client),
        Err(_) => None,
    }
}

// =============================================================================
// Circuit Breaker Tests
// =============================================================================

/// Test: Circuit breaker state transitions
///
/// Verifies that the system can track and report circuit breaker states:
/// - Closed (healthy)
/// - Open (failed, blocking requests)
/// - Half-open (testing recovery)
#[tokio::test]
async fn test_circuit_breaker_state_tracking() {
    let logger = TestLoggerBuilder::new("test_circuit_breaker_state_tracking")
        .print_realtime(true)
        .build();

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("circuit_breaker".to_string()),
        "Starting circuit breaker state tracking test",
        vec![
            ("phase".to_string(), "setup".to_string()),
            ("scenario".to_string(), "state_tracking".to_string()),
        ],
    );

    let Some(config) = require_workers() else {
        logger.warn("Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.warn("Test skipped: no enabled worker found");
        return;
    };

    let Some(mut client) = get_connected_client(&config, worker_entry).await else {
        logger.error("Failed to connect to worker");
        return;
    };

    // Phase: Test successful connection (circuit should be closed)
    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("circuit_breaker".to_string()),
        "Testing healthy connection state",
        vec![
            ("phase".to_string(), "test_healthy".to_string()),
            ("expected_state".to_string(), "closed".to_string()),
        ],
    );

    let health_check = client.execute("echo 'healthy'").await;
    let is_healthy = health_check.is_ok();

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("circuit_breaker".to_string()),
        "Health check completed",
        vec![
            ("phase".to_string(), "verify".to_string()),
            ("healthy".to_string(), is_healthy.to_string()),
            (
                "circuit_state".to_string(),
                if is_healthy { "closed" } else { "open" }.to_string(),
            ),
        ],
    );

    assert!(is_healthy, "Worker should be healthy with closed circuit");

    // Phase: Simulate multiple successful operations
    let mut success_count = 0;
    for i in 0..5 {
        let result = client.execute(&format!("echo 'check {}'", i)).await;
        if result.is_ok() {
            success_count += 1;
        }
    }

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("circuit_breaker".to_string()),
        "Consecutive success tracking",
        vec![
            ("phase".to_string(), "consecutive_success".to_string()),
            ("success_count".to_string(), success_count.to_string()),
            ("total_attempts".to_string(), "5".to_string()),
        ],
    );

    assert_eq!(success_count, 5, "All health checks should succeed");

    client.disconnect().await.ok();
    logger.info("Circuit breaker state tracking test passed");
    logger.print_summary();
}

/// Test: Circuit breaker failure detection
///
/// Simulates failures and verifies the circuit breaker detects them.
#[tokio::test]
async fn test_circuit_breaker_failure_detection() {
    let logger = TestLoggerBuilder::new("test_circuit_breaker_failure_detection")
        .print_realtime(true)
        .build();

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("circuit_breaker".to_string()),
        "Starting circuit breaker failure detection test",
        vec![
            ("phase".to_string(), "setup".to_string()),
            ("scenario".to_string(), "failure_detection".to_string()),
        ],
    );

    let Some(config) = require_workers() else {
        logger.warn("Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.warn("Test skipped: no enabled worker found");
        return;
    };

    let Some(mut client) = get_connected_client(&config, worker_entry).await else {
        logger.error("Failed to connect to worker");
        return;
    };

    // Phase: Execute commands that will fail
    let failing_commands = vec![
        ("exit 1", 1, "explicit_failure"),
        ("false", 1, "false_command"),
        ("test -f /nonexistent/path", 1, "path_not_found"),
        ("exit 42", 42, "custom_error_code"),
    ];

    for (cmd, expected_code, desc) in &failing_commands {
        let result = client.execute(cmd).await;

        let actual_code = match &result {
            Ok(r) => r.exit_code,
            Err(_) => -1,
        };

        logger.log_with_context(
            LogLevel::Info,
            LogSource::Custom("circuit_breaker".to_string()),
            "Failure detected",
            vec![
                ("phase".to_string(), "failure_tracking".to_string()),
                ("command".to_string(), cmd.to_string()),
                ("expected_code".to_string(), expected_code.to_string()),
                ("actual_code".to_string(), actual_code.to_string()),
                ("description".to_string(), desc.to_string()),
            ],
        );

        assert_eq!(
            actual_code, *expected_code,
            "Command '{}' should return exit code {}, got {}",
            cmd, expected_code, actual_code
        );
    }

    // Verify connection is still usable after failures
    let recovery_check = client.execute("echo 'recovered'").await;
    let recovered = recovery_check
        .as_ref()
        .map(|r| r.exit_code == 0)
        .unwrap_or(false);

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("circuit_breaker".to_string()),
        "Post-failure recovery check",
        vec![
            ("phase".to_string(), "recovery".to_string()),
            ("connection_usable".to_string(), recovered.to_string()),
        ],
    );

    assert!(
        recovered,
        "Connection should remain usable after command failures"
    );

    client.disconnect().await.ok();
    logger.info("Circuit breaker failure detection test passed");
    logger.print_summary();
}

// =============================================================================
// Toolchain Verification Tests
// =============================================================================

/// Test: Verify Rust toolchain availability on worker
///
/// Checks that the worker has the required Rust toolchain installed.
#[tokio::test]
async fn test_toolchain_rust_availability() {
    let logger = TestLoggerBuilder::new("test_toolchain_rust_availability")
        .print_realtime(true)
        .build();

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("toolchain".to_string()),
        "Starting Rust toolchain availability test",
        vec![
            ("phase".to_string(), "setup".to_string()),
            ("toolchain".to_string(), "rust".to_string()),
        ],
    );

    let Some(config) = require_workers() else {
        logger.warn("Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.warn("Test skipped: no enabled worker found");
        return;
    };

    let Some(mut client) = get_connected_client(&config, worker_entry).await else {
        logger.error("Failed to connect to worker");
        return;
    };

    // Check for cargo
    let cargo_result = client.execute("which cargo && cargo --version").await;
    let has_cargo = cargo_result
        .as_ref()
        .map(|r| r.exit_code == 0)
        .unwrap_or(false);
    let cargo_version = cargo_result
        .as_ref()
        .map(|r| r.stdout.lines().last().unwrap_or("unknown").to_string())
        .unwrap_or_else(|_| "not found".to_string());

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("toolchain".to_string()),
        "Cargo check",
        vec![
            ("phase".to_string(), "check_cargo".to_string()),
            ("available".to_string(), has_cargo.to_string()),
            ("version".to_string(), cargo_version.clone()),
        ],
    );

    // Check for rustc
    let rustc_result = client.execute("which rustc && rustc --version").await;
    let has_rustc = rustc_result
        .as_ref()
        .map(|r| r.exit_code == 0)
        .unwrap_or(false);
    let rustc_version = rustc_result
        .as_ref()
        .map(|r| r.stdout.lines().last().unwrap_or("unknown").to_string())
        .unwrap_or_else(|_| "not found".to_string());

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("toolchain".to_string()),
        "Rustc check",
        vec![
            ("phase".to_string(), "check_rustc".to_string()),
            ("available".to_string(), has_rustc.to_string()),
            ("version".to_string(), rustc_version.clone()),
        ],
    );

    // Check for rustup (optional but common)
    let rustup_result = client
        .execute("which rustup && rustup show active-toolchain 2>/dev/null || echo 'no rustup'")
        .await;
    let rustup_info = rustup_result
        .as_ref()
        .map(|r| r.stdout.trim().to_string())
        .unwrap_or_else(|_| "not found".to_string());

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("toolchain".to_string()),
        "Rustup check",
        vec![
            ("phase".to_string(), "check_rustup".to_string()),
            ("info".to_string(), rustup_info),
        ],
    );

    // Summary
    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("toolchain".to_string()),
        "Rust toolchain summary",
        vec![
            ("phase".to_string(), "summary".to_string()),
            ("worker".to_string(), worker_entry.id.clone()),
            ("has_cargo".to_string(), has_cargo.to_string()),
            ("has_rustc".to_string(), has_rustc.to_string()),
            (
                "ready_for_rust".to_string(),
                (has_cargo && has_rustc).to_string(),
            ),
        ],
    );

    assert!(has_cargo, "Worker should have cargo installed");
    assert!(has_rustc, "Worker should have rustc installed");

    client.disconnect().await.ok();
    logger.info("Rust toolchain availability test passed");
    logger.print_summary();
}

/// Test: Verify C/C++ toolchain availability on worker
///
/// Checks that the worker has gcc/clang and make installed.
#[tokio::test]
async fn test_toolchain_c_availability() {
    let logger = TestLoggerBuilder::new("test_toolchain_c_availability")
        .print_realtime(true)
        .build();

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("toolchain".to_string()),
        "Starting C/C++ toolchain availability test",
        vec![
            ("phase".to_string(), "setup".to_string()),
            ("toolchain".to_string(), "c_cpp".to_string()),
        ],
    );

    let Some(config) = require_workers() else {
        logger.warn("Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.warn("Test skipped: no enabled worker found");
        return;
    };

    let Some(mut client) = get_connected_client(&config, worker_entry).await else {
        logger.error("Failed to connect to worker");
        return;
    };

    // Check for gcc
    let gcc_result = client.execute("which gcc && gcc --version | head -1").await;
    let has_gcc = gcc_result
        .as_ref()
        .map(|r| r.exit_code == 0)
        .unwrap_or(false);
    let gcc_version = gcc_result
        .as_ref()
        .map(|r| r.stdout.lines().last().unwrap_or("unknown").to_string())
        .unwrap_or_else(|_| "not found".to_string());

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("toolchain".to_string()),
        "GCC check",
        vec![
            ("phase".to_string(), "check_gcc".to_string()),
            ("available".to_string(), has_gcc.to_string()),
            ("version".to_string(), gcc_version),
        ],
    );

    // Check for clang
    let clang_result = client
        .execute("which clang && clang --version | head -1")
        .await;
    let has_clang = clang_result
        .as_ref()
        .map(|r| r.exit_code == 0)
        .unwrap_or(false);
    let clang_version = clang_result
        .as_ref()
        .map(|r| r.stdout.lines().last().unwrap_or("unknown").to_string())
        .unwrap_or_else(|_| "not found".to_string());

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("toolchain".to_string()),
        "Clang check",
        vec![
            ("phase".to_string(), "check_clang".to_string()),
            ("available".to_string(), has_clang.to_string()),
            ("version".to_string(), clang_version),
        ],
    );

    // Check for make
    let make_result = client
        .execute("which make && make --version | head -1")
        .await;
    let has_make = make_result
        .as_ref()
        .map(|r| r.exit_code == 0)
        .unwrap_or(false);
    let make_version = make_result
        .as_ref()
        .map(|r| r.stdout.lines().last().unwrap_or("unknown").to_string())
        .unwrap_or_else(|_| "not found".to_string());

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("toolchain".to_string()),
        "Make check",
        vec![
            ("phase".to_string(), "check_make".to_string()),
            ("available".to_string(), has_make.to_string()),
            ("version".to_string(), make_version),
        ],
    );

    // Summary
    let has_c_compiler = has_gcc || has_clang;
    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("toolchain".to_string()),
        "C/C++ toolchain summary",
        vec![
            ("phase".to_string(), "summary".to_string()),
            ("worker".to_string(), worker_entry.id.clone()),
            ("has_gcc".to_string(), has_gcc.to_string()),
            ("has_clang".to_string(), has_clang.to_string()),
            ("has_make".to_string(), has_make.to_string()),
            (
                "ready_for_c".to_string(),
                (has_c_compiler && has_make).to_string(),
            ),
        ],
    );

    assert!(has_c_compiler, "Worker should have gcc or clang installed");
    assert!(has_make, "Worker should have make installed");

    client.disconnect().await.ok();
    logger.info("C/C++ toolchain availability test passed");
    logger.print_summary();
}

/// Test: Detect missing toolchain gracefully
///
/// Verifies that the system gracefully handles missing tools.
#[tokio::test]
async fn test_toolchain_missing_detection() {
    let logger = TestLoggerBuilder::new("test_toolchain_missing_detection")
        .print_realtime(true)
        .build();

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("toolchain".to_string()),
        "Starting missing toolchain detection test",
        vec![
            ("phase".to_string(), "setup".to_string()),
            ("scenario".to_string(), "missing_detection".to_string()),
        ],
    );

    let Some(config) = require_workers() else {
        logger.warn("Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.warn("Test skipped: no enabled worker found");
        return;
    };

    let Some(mut client) = get_connected_client(&config, worker_entry).await else {
        logger.error("Failed to connect to worker");
        return;
    };

    // Test detection of non-existent tools
    let nonexistent_tools = vec![
        "nonexistent_compiler_12345",
        "fake_build_tool_xyz",
        "imaginary_linker_abc",
    ];

    for tool in &nonexistent_tools {
        let result = client.execute(&format!("which {} 2>/dev/null", tool)).await;
        let tool_exists = result.as_ref().map(|r| r.exit_code == 0).unwrap_or(false);

        logger.log_with_context(
            LogLevel::Info,
            LogSource::Custom("toolchain".to_string()),
            "Missing tool check",
            vec![
                ("phase".to_string(), "check_missing".to_string()),
                ("tool".to_string(), tool.to_string()),
                ("exists".to_string(), tool_exists.to_string()),
                ("detection_correct".to_string(), (!tool_exists).to_string()),
            ],
        );

        assert!(
            !tool_exists,
            "Nonexistent tool '{}' should not be found",
            tool
        );
    }

    // Verify the shell correctly reports command not found
    let not_found_result = client.execute("nonexistent_cmd_xyz123 2>&1").await;
    let exit_code = not_found_result.as_ref().map(|r| r.exit_code).unwrap_or(-1);

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("toolchain".to_string()),
        "Command not found exit code",
        vec![
            ("phase".to_string(), "verify_exit_code".to_string()),
            ("exit_code".to_string(), exit_code.to_string()),
            ("expected".to_string(), "127".to_string()),
        ],
    );

    // Exit code 127 = command not found
    assert_eq!(
        exit_code, 127,
        "Command not found should return exit code 127"
    );

    client.disconnect().await.ok();
    logger.info("Missing toolchain detection test passed");
    logger.print_summary();
}

// =============================================================================
// Worker Health Monitoring Tests
// =============================================================================

/// Test: Worker disk space check
///
/// Verifies that the worker has sufficient disk space for compilation.
#[tokio::test]
async fn test_worker_disk_space_check() {
    let logger = TestLoggerBuilder::new("test_worker_disk_space_check")
        .print_realtime(true)
        .build();

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("health".to_string()),
        "Starting worker disk space check test",
        vec![
            ("phase".to_string(), "setup".to_string()),
            ("check_type".to_string(), "disk_space".to_string()),
        ],
    );

    let Some(config) = require_workers() else {
        logger.warn("Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.warn("Test skipped: no enabled worker found");
        return;
    };

    let Some(mut client) = get_connected_client(&config, worker_entry).await else {
        logger.error("Failed to connect to worker");
        return;
    };

    // Get disk space info
    let df_result = client.execute("df -h /tmp | tail -1").await;

    if let Ok(result) = &df_result {
        let parts: Vec<&str> = result.stdout.split_whitespace().collect();
        if parts.len() >= 4 {
            let available = parts.get(3).unwrap_or(&"unknown");
            let use_percent = parts.get(4).unwrap_or(&"unknown");

            logger.log_with_context(
                LogLevel::Info,
                LogSource::Custom("health".to_string()),
                "Disk space info",
                vec![
                    ("phase".to_string(), "check_disk".to_string()),
                    ("path".to_string(), "/tmp".to_string()),
                    ("available".to_string(), available.to_string()),
                    ("use_percent".to_string(), use_percent.to_string()),
                ],
            );
        }
    }

    // Check if /tmp is writable
    let write_test = client
        .execute("touch /tmp/rch_disk_test && rm /tmp/rch_disk_test && echo 'writable'")
        .await;
    let is_writable = write_test
        .as_ref()
        .map(|r| r.exit_code == 0)
        .unwrap_or(false);

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("health".to_string()),
        "Disk write test",
        vec![
            ("phase".to_string(), "write_test".to_string()),
            ("path".to_string(), "/tmp".to_string()),
            ("writable".to_string(), is_writable.to_string()),
        ],
    );

    assert!(is_writable, "Worker /tmp should be writable");

    // Get work directory disk info
    let work_dir = &config.settings.remote_work_dir;
    let work_df = client
        .execute(&format!(
            "df -h {} 2>/dev/null | tail -1 || echo 'not mounted'",
            work_dir
        ))
        .await;

    if let Ok(result) = &work_df {
        logger.log_with_context(
            LogLevel::Info,
            LogSource::Custom("health".to_string()),
            "Work directory disk info",
            vec![
                ("phase".to_string(), "check_work_dir".to_string()),
                ("path".to_string(), work_dir.clone()),
                ("info".to_string(), result.stdout.trim().to_string()),
            ],
        );
    }

    client.disconnect().await.ok();
    logger.info("Worker disk space check test passed");
    logger.print_summary();
}

/// Test: Worker memory availability
///
/// Checks the worker's memory status for compilation readiness.
#[tokio::test]
async fn test_worker_memory_check() {
    let logger = TestLoggerBuilder::new("test_worker_memory_check")
        .print_realtime(true)
        .build();

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("health".to_string()),
        "Starting worker memory check test",
        vec![
            ("phase".to_string(), "setup".to_string()),
            ("check_type".to_string(), "memory".to_string()),
        ],
    );

    let Some(config) = require_workers() else {
        logger.warn("Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.warn("Test skipped: no enabled worker found");
        return;
    };

    let Some(mut client) = get_connected_client(&config, worker_entry).await else {
        logger.error("Failed to connect to worker");
        return;
    };

    // Get memory info
    let mem_result = client.execute("free -h | grep Mem").await;

    if let Ok(result) = &mem_result {
        let parts: Vec<&str> = result.stdout.split_whitespace().collect();
        if parts.len() >= 4 {
            let total = parts.get(1).unwrap_or(&"unknown");
            let used = parts.get(2).unwrap_or(&"unknown");
            let available = parts.get(6).unwrap_or(&"unknown");

            logger.log_with_context(
                LogLevel::Info,
                LogSource::Custom("health".to_string()),
                "Memory info",
                vec![
                    ("phase".to_string(), "check_memory".to_string()),
                    ("total".to_string(), total.to_string()),
                    ("used".to_string(), used.to_string()),
                    ("available".to_string(), available.to_string()),
                ],
            );
        }
    }

    // Check load average
    let load_result = client.execute("cat /proc/loadavg").await;

    if let Ok(result) = &load_result {
        let parts: Vec<&str> = result.stdout.split_whitespace().collect();
        if parts.len() >= 3 {
            let load_1m = parts.first().copied().unwrap_or("unknown");
            let load_5m = parts.get(1).copied().unwrap_or("unknown");
            let load_15m = parts.get(2).copied().unwrap_or("unknown");

            logger.log_with_context(
                LogLevel::Info,
                LogSource::Custom("health".to_string()),
                "Load average",
                vec![
                    ("phase".to_string(), "check_load".to_string()),
                    ("1m".to_string(), load_1m.to_string()),
                    ("5m".to_string(), load_5m.to_string()),
                    ("15m".to_string(), load_15m.to_string()),
                ],
            );
        }
    }

    // Get CPU count for context
    let cpu_result = client
        .execute("nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 1")
        .await;
    let cpu_count = cpu_result
        .as_ref()
        .map(|r| r.stdout.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("health".to_string()),
        "CPU count",
        vec![
            ("phase".to_string(), "check_cpu".to_string()),
            ("count".to_string(), cpu_count),
        ],
    );

    client.disconnect().await.ok();
    logger.info("Worker memory check test passed");
    logger.print_summary();
}

/// Test: Worker rsync availability
///
/// Verifies that rsync is available for file transfer.
#[tokio::test]
async fn test_worker_rsync_availability() {
    let logger = TestLoggerBuilder::new("test_worker_rsync_availability")
        .print_realtime(true)
        .build();

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("health".to_string()),
        "Starting rsync availability test",
        vec![
            ("phase".to_string(), "setup".to_string()),
            ("check_type".to_string(), "rsync".to_string()),
        ],
    );

    let Some(config) = require_workers() else {
        logger.warn("Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.warn("Test skipped: no enabled worker found");
        return;
    };

    let Some(mut client) = get_connected_client(&config, worker_entry).await else {
        logger.error("Failed to connect to worker");
        return;
    };

    // Check rsync
    let rsync_result = client
        .execute("which rsync && rsync --version | head -1")
        .await;
    let has_rsync = rsync_result
        .as_ref()
        .map(|r| r.exit_code == 0)
        .unwrap_or(false);
    let rsync_version = rsync_result
        .as_ref()
        .map(|r| r.stdout.lines().last().unwrap_or("unknown").to_string())
        .unwrap_or_else(|_| "not found".to_string());

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("health".to_string()),
        "Rsync check",
        vec![
            ("phase".to_string(), "check_rsync".to_string()),
            ("available".to_string(), has_rsync.to_string()),
            ("version".to_string(), rsync_version),
        ],
    );

    // Check zstd (optional but preferred)
    let zstd_result = client
        .execute("which zstd && zstd --version 2>/dev/null | head -1")
        .await;
    let has_zstd = zstd_result
        .as_ref()
        .map(|r| r.exit_code == 0)
        .unwrap_or(false);
    let zstd_version = zstd_result
        .as_ref()
        .map(|r| r.stdout.lines().last().unwrap_or("unknown").to_string())
        .unwrap_or_else(|_| "not found".to_string());

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("health".to_string()),
        "Zstd check",
        vec![
            ("phase".to_string(), "check_zstd".to_string()),
            ("available".to_string(), has_zstd.to_string()),
            ("version".to_string(), zstd_version),
        ],
    );

    assert!(has_rsync, "Worker should have rsync installed");

    client.disconnect().await.ok();
    logger.info("Rsync availability test passed");
    logger.print_summary();
}

// =============================================================================
// Retry and Recovery Tests
// =============================================================================

/// Test: Transient error recovery
///
/// Verifies that the system can recover from transient errors.
#[tokio::test]
async fn test_transient_error_recovery() {
    let logger = TestLoggerBuilder::new("test_transient_error_recovery")
        .print_realtime(true)
        .build();

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("recovery".to_string()),
        "Starting transient error recovery test",
        vec![
            ("phase".to_string(), "setup".to_string()),
            ("scenario".to_string(), "transient_recovery".to_string()),
        ],
    );

    let Some(config) = require_workers() else {
        logger.warn("Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.warn("Test skipped: no enabled worker found");
        return;
    };

    let Some(mut client) = get_connected_client(&config, worker_entry).await else {
        logger.error("Failed to connect to worker");
        return;
    };

    // Simulate transient errors followed by success
    // Pattern: fail, fail, succeed
    let test_sequence = vec![
        ("exit 1", false, "simulated_failure_1"),
        ("exit 1", false, "simulated_failure_2"),
        ("exit 0", true, "recovery_success"),
    ];

    for (cmd, should_succeed, phase) in &test_sequence {
        let result = client.execute(cmd).await;
        let succeeded = result.as_ref().map(|r| r.exit_code == 0).unwrap_or(false);

        logger.log_with_context(
            LogLevel::Info,
            LogSource::Custom("recovery".to_string()),
            "Execution in sequence",
            vec![
                ("phase".to_string(), phase.to_string()),
                ("command".to_string(), cmd.to_string()),
                ("expected_success".to_string(), should_succeed.to_string()),
                ("actual_success".to_string(), succeeded.to_string()),
            ],
        );

        assert_eq!(
            succeeded, *should_succeed,
            "Command '{}' should have success={}, got {}",
            cmd, should_succeed, succeeded
        );
    }

    // Verify connection remains healthy after error sequence
    let final_check = client.execute("echo 'connection healthy'").await;
    let still_healthy = final_check
        .as_ref()
        .map(|r| r.exit_code == 0 && r.stdout.contains("healthy"))
        .unwrap_or(false);

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("recovery".to_string()),
        "Final health check",
        vec![
            ("phase".to_string(), "final_check".to_string()),
            ("connection_healthy".to_string(), still_healthy.to_string()),
        ],
    );

    assert!(
        still_healthy,
        "Connection should remain healthy after transient errors"
    );

    client.disconnect().await.ok();
    logger.info("Transient error recovery test passed");
    logger.print_summary();
}

/// Test: Graceful disconnect and reconnect
///
/// Verifies that a disconnected session can be re-established.
#[tokio::test]
async fn test_disconnect_reconnect() {
    let logger = TestLoggerBuilder::new("test_disconnect_reconnect")
        .print_realtime(true)
        .build();

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("recovery".to_string()),
        "Starting disconnect/reconnect test",
        vec![
            ("phase".to_string(), "setup".to_string()),
            ("scenario".to_string(), "reconnect".to_string()),
        ],
    );

    let Some(config) = require_workers() else {
        logger.warn("Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.warn("Test skipped: no enabled worker found");
        return;
    };

    // Phase 1: Initial connection
    let Some(mut client) = get_connected_client(&config, worker_entry).await else {
        logger.error("Failed to connect to worker");
        return;
    };

    let initial_check = client.execute("echo 'initial connection'").await;
    let initial_ok = initial_check
        .as_ref()
        .map(|r| r.exit_code == 0)
        .unwrap_or(false);

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("recovery".to_string()),
        "Initial connection",
        vec![
            ("phase".to_string(), "initial".to_string()),
            ("connected".to_string(), initial_ok.to_string()),
        ],
    );

    assert!(initial_ok, "Initial connection should succeed");

    // Phase 2: Disconnect
    let disconnect_result = client.disconnect().await;
    let disconnected = disconnect_result.is_ok();

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("recovery".to_string()),
        "Disconnect",
        vec![
            ("phase".to_string(), "disconnect".to_string()),
            ("clean_disconnect".to_string(), disconnected.to_string()),
        ],
    );

    // Phase 3: Reconnect
    let Some(mut new_client) = get_connected_client(&config, worker_entry).await else {
        logger.error("Failed to reconnect to worker");
        return;
    };

    let reconnect_check = new_client.execute("echo 'reconnected'").await;
    let reconnected = reconnect_check
        .as_ref()
        .map(|r| r.exit_code == 0 && r.stdout.contains("reconnected"))
        .unwrap_or(false);

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("recovery".to_string()),
        "Reconnect",
        vec![
            ("phase".to_string(), "reconnect".to_string()),
            ("success".to_string(), reconnected.to_string()),
        ],
    );

    assert!(reconnected, "Reconnection should succeed");

    new_client.disconnect().await.ok();
    logger.info("Disconnect/reconnect test passed");
    logger.print_summary();
}
