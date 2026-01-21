//! True E2E Tests: SSH Connection & Authentication
//!
//! Tests SSH connections to real workers without mocking, verifying
//! authentication, connection pooling, and timeout handling.
//!
//! # Test Categories
//!
//! 1. Basic SSH Connectivity - connect, authenticate, close
//! 2. Connection Timeout - handling invalid hosts, budget verification
//! 3. Connection Pooling - reuse, cleanup, leak prevention
//! 4. Key Authentication - default and custom key paths
//! 5. Reconnection - recovery after disconnect
//! 6. Multiple Workers - simultaneous connections
//!
//! # Running These Tests
//!
//! ```bash
//! # Requires workers_test.toml configuration
//! cargo test --features true-e2e ssh_tests -- --nocapture
//! ```

use rch_common::e2e::{
    LogLevel, LogSource, TestConfigError, TestLoggerBuilder, TestWorkersConfig,
    should_skip_worker_check,
};
use rch_common::ssh::{CommandResult, KnownHostsPolicy, SshClient, SshOptions, SshPool};
use rch_common::types::{WorkerConfig, WorkerId};
use std::time::{Duration, Instant};

/// Skip the test if no real workers are available.
/// Returns the loaded config if workers are available.
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

/// Convert TestWorkerEntry to WorkerConfig for SSH client.
fn to_worker_config(entry: &rch_common::e2e::TestWorkerEntry) -> WorkerConfig {
    entry.to_worker_config()
}

// =============================================================================
// Test: Basic SSH Connectivity
// =============================================================================

/// Test 1.1: Basic SSH connection to worker
#[tokio::test]
async fn test_ssh_basic_connect() {
    let logger = TestLoggerBuilder::new("test_ssh_basic_connect")
        .print_realtime(true)
        .build();

    logger.info("Starting SSH basic connectivity test");

    let Some(config) = require_workers() else {
        logger.warn("Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.warn("Test skipped: no enabled worker found");
        return;
    };

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Harness,
        "Testing connection to worker",
        vec![
            ("worker_id".to_string(), worker_entry.id.clone()),
            ("host".to_string(), worker_entry.host.clone()),
            ("user".to_string(), worker_entry.user.clone()),
        ],
    );

    let worker_config = to_worker_config(worker_entry);
    let options = SshOptions {
        connect_timeout: Duration::from_secs(config.settings.ssh_connection_timeout_secs),
        known_hosts: KnownHostsPolicy::Add,
        ..Default::default()
    };

    let mut client = SshClient::new(worker_config.clone(), options);

    // Phase: Connect
    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("ssh".to_string()),
        "SSH connection attempt",
        vec![
            ("worker".to_string(), worker_entry.id.clone()),
            ("port".to_string(), worker_entry.port.to_string()),
        ],
    );

    let connect_start = Instant::now();
    match client.connect().await {
        Ok(()) => {
            let connect_duration = connect_start.elapsed();
            logger.log_with_context(
                LogLevel::Info,
                LogSource::Custom("ssh".to_string()),
                "SSH connection established",
                vec![
                    ("worker".to_string(), worker_entry.id.clone()),
                    ("success".to_string(), "true".to_string()),
                    ("connect_duration_ms".to_string(), connect_duration.as_millis().to_string()),
                ],
            );

            assert!(client.is_connected(), "Client should report connected");
        }
        Err(e) => {
            logger.log_with_context(
                LogLevel::Error,
                LogSource::Custom("ssh".to_string()),
                "SSH connection failed",
                vec![
                    ("worker".to_string(), worker_entry.id.clone()),
                    ("error".to_string(), e.to_string()),
                ],
            );
            panic!("SSH connection failed: {e}");
        }
    }

    // Phase: Verify with echo
    logger.info("Verifying connection with echo command");
    match client.execute("echo 'ssh_test_ok'").await {
        Ok(result) => {
            logger.log_with_context(
                LogLevel::Info,
                LogSource::Custom("ssh".to_string()),
                "Command execution result",
                vec![
                    ("exit_code".to_string(), result.exit_code.to_string()),
                    ("stdout_len".to_string(), result.stdout.len().to_string()),
                    ("duration_ms".to_string(), result.duration_ms.to_string()),
                ],
            );
            assert!(result.success(), "Echo command should succeed");
            assert!(
                result.stdout.contains("ssh_test_ok"),
                "Output should contain test string"
            );
        }
        Err(e) => {
            logger.error(format!("Echo command failed: {e}"));
            panic!("Echo command failed: {e}");
        }
    }

    // Phase: Disconnect
    logger.info("Disconnecting from worker");
    match client.disconnect().await {
        Ok(()) => {
            logger.log_with_context(
                LogLevel::Info,
                LogSource::Custom("ssh".to_string()),
                "SSH connection closed",
                vec![
                    ("worker".to_string(), worker_entry.id.clone()),
                    ("clean_close".to_string(), "true".to_string()),
                ],
            );
            assert!(!client.is_connected(), "Client should report disconnected");
        }
        Err(e) => {
            logger.warn(format!("Disconnect warning: {e}"));
        }
    }

    logger.info("SSH basic connectivity test passed");
    logger.print_summary();
}

/// Test 1.2: Key authentication verification
#[tokio::test]
async fn test_ssh_key_authentication() {
    let logger = TestLoggerBuilder::new("test_ssh_key_authentication")
        .print_realtime(true)
        .build();

    logger.info("Starting SSH key authentication test");

    let Some(config) = require_workers() else {
        logger.warn("Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.warn("Test skipped: no enabled worker found");
        return;
    };

    // Log key info (path only, never content!)
    let key_path = worker_entry.identity_file.clone();
    let expanded_key = worker_entry.expanded_identity_file();

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("ssh".to_string()),
        "Testing key authentication",
        vec![
            ("key_path".to_string(), key_path.clone()),
            (
                "key_exists".to_string(),
                expanded_key
                    .as_ref()
                    .map(|p| p.exists())
                    .unwrap_or(false)
                    .to_string(),
            ),
        ],
    );

    let worker_config = to_worker_config(worker_entry);
    let options = SshOptions {
        connect_timeout: Duration::from_secs(config.settings.ssh_connection_timeout_secs),
        known_hosts: KnownHostsPolicy::Add,
        ..Default::default()
    };

    let mut client = SshClient::new(worker_config, options);

    let auth_start = Instant::now();
    match client.connect().await {
        Ok(()) => {
            let auth_duration = auth_start.elapsed();
            logger.log_with_context(
                LogLevel::Info,
                LogSource::Custom("ssh".to_string()),
                "SSH authentication result",
                vec![
                    ("worker".to_string(), worker_entry.id.clone()),
                    ("success".to_string(), "true".to_string()),
                    ("auth_duration_ms".to_string(), auth_duration.as_millis().to_string()),
                ],
            );
        }
        Err(e) => {
            logger.log_with_context(
                LogLevel::Error,
                LogSource::Custom("ssh".to_string()),
                "SSH authentication failed",
                vec![
                    ("worker".to_string(), worker_entry.id.clone()),
                    ("error".to_string(), e.to_string()),
                ],
            );
            panic!("Key authentication failed: {e}");
        }
    }

    // Verify we can execute commands (proves auth worked)
    let result = client.execute("whoami").await.expect("whoami should work");
    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("ssh".to_string()),
        "Identity verification",
        vec![
            ("remote_user".to_string(), result.stdout.trim().to_string()),
            ("expected_user".to_string(), worker_entry.user.clone()),
        ],
    );

    assert!(
        result.stdout.trim() == worker_entry.user,
        "whoami should return configured user"
    );

    client.disconnect().await.ok();
    logger.info("SSH key authentication test passed");
    logger.print_summary();
}

// =============================================================================
// Test: Connection Timeout
// =============================================================================

/// Test 2.1: Connection timeout with invalid host
#[tokio::test]
async fn test_ssh_connection_timeout() {
    let logger = TestLoggerBuilder::new("test_ssh_connection_timeout")
        .print_realtime(true)
        .build();

    logger.info("Starting SSH connection timeout test");

    // Create config for non-existent host with short timeout
    let timeout_secs = 2;
    let worker_config = WorkerConfig {
        id: WorkerId::new("timeout-test"),
        host: "192.0.2.1".to_string(), // TEST-NET-1 (RFC 5737) - guaranteed unreachable
        user: "nobody".to_string(),
        identity_file: "~/.ssh/id_rsa".to_string(),
        total_slots: 1,
        priority: 1,
        tags: vec![],
    };

    let options = SshOptions {
        connect_timeout: Duration::from_secs(timeout_secs),
        known_hosts: KnownHostsPolicy::AcceptAll,
        ..Default::default()
    };

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("ssh".to_string()),
        "Testing connection timeout",
        vec![
            ("host".to_string(), worker_config.host.clone()),
            ("timeout_secs".to_string(), timeout_secs.to_string()),
        ],
    );

    let mut client = SshClient::new(worker_config, options);

    let start = Instant::now();
    let result = client.connect().await;
    let elapsed = start.elapsed();

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("ssh".to_string()),
        "Connection attempt completed",
        vec![
            ("success".to_string(), result.is_ok().to_string()),
            ("actual_wait_ms".to_string(), elapsed.as_millis().to_string()),
            ("error_type".to_string(), result.as_ref().err().map(|e| format!("{e:?}")).unwrap_or_default()),
        ],
    );

    // Should fail
    assert!(result.is_err(), "Connection to unreachable host should fail");

    // Should timeout within reasonable bounds (timeout + buffer)
    let max_expected = Duration::from_secs(timeout_secs + 5);
    assert!(
        elapsed < max_expected,
        "Timeout should occur within {} seconds, took {:?}",
        timeout_secs + 5,
        elapsed
    );

    logger.info("SSH connection timeout test passed");
    logger.print_summary();
}

// =============================================================================
// Test: Connection Pooling
// =============================================================================

/// Test 3.1: Connection pool reuses connections
#[tokio::test]
async fn test_ssh_pool_reuse() {
    let logger = TestLoggerBuilder::new("test_ssh_pool_reuse")
        .print_realtime(true)
        .build();

    logger.info("Starting SSH connection pool reuse test");

    let Some(config) = require_workers() else {
        logger.warn("Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.warn("Test skipped: no enabled worker found");
        return;
    };

    let worker_config = to_worker_config(worker_entry);
    let options = SshOptions {
        connect_timeout: Duration::from_secs(config.settings.ssh_connection_timeout_secs),
        known_hosts: KnownHostsPolicy::Add,
        control_master: true, // Enable connection reuse
        ..Default::default()
    };

    let pool = SshPool::new(options);

    // First connection - should create new
    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("ssh".to_string()),
        "Connection pool stats (before first connection)",
        vec![("active".to_string(), pool.active_connections().await.to_string())],
    );

    let first_start = Instant::now();
    let _client1 = pool
        .get_or_connect(&worker_config)
        .await
        .expect("First connection should succeed");
    let first_duration = first_start.elapsed();

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("ssh".to_string()),
        "First connection established",
        vec![
            ("duration_ms".to_string(), first_duration.as_millis().to_string()),
            ("active".to_string(), pool.active_connections().await.to_string()),
        ],
    );

    // Second connection - should reuse existing
    let second_start = Instant::now();
    let _client2 = pool
        .get_or_connect(&worker_config)
        .await
        .expect("Second connection should succeed");
    let second_duration = second_start.elapsed();

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("ssh".to_string()),
        "Second connection (reused)",
        vec![
            ("duration_ms".to_string(), second_duration.as_millis().to_string()),
            ("active".to_string(), pool.active_connections().await.to_string()),
            ("reused".to_string(), (second_duration < first_duration / 2).to_string()),
        ],
    );

    // Pool should have only 1 active connection (reused)
    let active = pool.active_connections().await;
    assert_eq!(active, 1, "Pool should reuse connection, not create new one");

    // Second connection should be faster (reused)
    // (This may not always hold due to jitter, so we just log it)
    logger.info(format!(
        "First: {}ms, Second: {}ms (expected second to be faster if reused)",
        first_duration.as_millis(),
        second_duration.as_millis()
    ));

    // Cleanup
    pool.close_all().await.expect("Pool cleanup should succeed");

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("ssh".to_string()),
        "Pool closed",
        vec![("active".to_string(), pool.active_connections().await.to_string())],
    );

    assert_eq!(
        pool.active_connections().await,
        0,
        "Pool should have no connections after close_all"
    );

    logger.info("SSH connection pool reuse test passed");
    logger.print_summary();
}

/// Test 3.2: Connection pool doesn't leak connections
#[tokio::test]
async fn test_ssh_pool_no_leaks() {
    let logger = TestLoggerBuilder::new("test_ssh_pool_no_leaks")
        .print_realtime(true)
        .build();

    logger.info("Starting SSH connection pool leak test");

    let Some(config) = require_workers() else {
        logger.warn("Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.warn("Test skipped: no enabled worker found");
        return;
    };

    let worker_config = to_worker_config(worker_entry);
    let options = SshOptions {
        connect_timeout: Duration::from_secs(config.settings.ssh_connection_timeout_secs),
        known_hosts: KnownHostsPolicy::Add,
        ..Default::default()
    };

    let pool = SshPool::new(options);

    // Create and use multiple connections
    for i in 0..5 {
        let client = pool
            .get_or_connect(&worker_config)
            .await
            .expect("Connection should succeed");

        // Execute a command
        let client_guard = client.read().await;
        let _ = client_guard.execute("echo test").await;
        drop(client_guard);

        logger.log_with_context(
            LogLevel::Debug,
            LogSource::Custom("ssh".to_string()),
            "Connection iteration",
            vec![
                ("iteration".to_string(), i.to_string()),
                ("active".to_string(), pool.active_connections().await.to_string()),
            ],
        );
    }

    // Should still have only 1 connection (all reused)
    let final_count = pool.active_connections().await;
    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("ssh".to_string()),
        "Final pool state",
        vec![("active".to_string(), final_count.to_string())],
    );

    assert_eq!(
        final_count, 1,
        "Pool should not leak connections - expected 1, got {}",
        final_count
    );

    pool.close_all().await.ok();
    logger.info("SSH connection pool leak test passed");
    logger.print_summary();
}

// =============================================================================
// Test: Health Check
// =============================================================================

/// Test 4: SSH health check
#[tokio::test]
async fn test_ssh_health_check() {
    let logger = TestLoggerBuilder::new("test_ssh_health_check")
        .print_realtime(true)
        .build();

    logger.info("Starting SSH health check test");

    let Some(config) = require_workers() else {
        logger.warn("Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.warn("Test skipped: no enabled worker found");
        return;
    };

    let worker_config = to_worker_config(worker_entry);
    let options = SshOptions {
        connect_timeout: Duration::from_secs(config.settings.ssh_connection_timeout_secs),
        known_hosts: KnownHostsPolicy::Add,
        ..Default::default()
    };

    let mut client = SshClient::new(worker_config, options);
    client.connect().await.expect("Connection should succeed");

    // Health check should pass
    let health_start = Instant::now();
    let healthy = client.health_check().await.expect("Health check should complete");
    let health_duration = health_start.elapsed();

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("ssh".to_string()),
        "Health check result",
        vec![
            ("worker".to_string(), worker_entry.id.clone()),
            ("healthy".to_string(), healthy.to_string()),
            ("duration_ms".to_string(), health_duration.as_millis().to_string()),
        ],
    );

    assert!(healthy, "Connected worker should report healthy");

    client.disconnect().await.ok();
    logger.info("SSH health check test passed");
    logger.print_summary();
}

// =============================================================================
// Test: Multiple Workers (if available)
// =============================================================================

/// Test 5: Connect to multiple workers simultaneously
#[tokio::test]
async fn test_ssh_multiple_workers() {
    let logger = TestLoggerBuilder::new("test_ssh_multiple_workers")
        .print_realtime(true)
        .build();

    logger.info("Starting SSH multiple workers test");

    let Some(config) = require_workers() else {
        logger.warn("Test skipped: no workers available");
        return;
    };

    let enabled = config.enabled_workers();
    if enabled.len() < 2 {
        logger.warn(format!(
            "Test skipped: need at least 2 workers, have {}",
            enabled.len()
        ));
        return;
    }

    let options = SshOptions {
        connect_timeout: Duration::from_secs(config.settings.ssh_connection_timeout_secs),
        known_hosts: KnownHostsPolicy::Add,
        ..Default::default()
    };

    let pool = SshPool::new(options);

    // Connect to all enabled workers
    let mut successful = 0;
    for worker_entry in enabled.iter() {
        let worker_config = to_worker_config(worker_entry);

        match pool.get_or_connect(&worker_config).await {
            Ok(client) => {
                let guard = client.read().await;
                match guard.execute("hostname").await {
                    Ok(result) => {
                        logger.log_with_context(
                            LogLevel::Info,
                            LogSource::Custom("ssh".to_string()),
                            "Worker connection success",
                            vec![
                                ("worker_id".to_string(), worker_entry.id.clone()),
                                ("hostname".to_string(), result.stdout.trim().to_string()),
                            ],
                        );
                        successful += 1;
                    }
                    Err(e) => {
                        logger.log_with_context(
                            LogLevel::Warn,
                            LogSource::Custom("ssh".to_string()),
                            "Worker command failed",
                            vec![
                                ("worker_id".to_string(), worker_entry.id.clone()),
                                ("error".to_string(), e.to_string()),
                            ],
                        );
                    }
                }
            }
            Err(e) => {
                logger.log_with_context(
                    LogLevel::Warn,
                    LogSource::Custom("ssh".to_string()),
                    "Worker connection failed",
                    vec![
                        ("worker_id".to_string(), worker_entry.id.clone()),
                        ("error".to_string(), e.to_string()),
                    ],
                );
            }
        }
    }

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("ssh".to_string()),
        "Multiple workers test summary",
        vec![
            ("total_workers".to_string(), enabled.len().to_string()),
            ("successful".to_string(), successful.to_string()),
            ("pool_active".to_string(), pool.active_connections().await.to_string()),
        ],
    );

    assert!(
        successful >= 1,
        "At least one worker should be reachable"
    );

    pool.close_all().await.ok();
    logger.info("SSH multiple workers test passed");
    logger.print_summary();
}

// =============================================================================
// Test: Reconnection
// =============================================================================

/// Test 6: Reconnection after disconnect
#[tokio::test]
async fn test_ssh_reconnection() {
    let logger = TestLoggerBuilder::new("test_ssh_reconnection")
        .print_realtime(true)
        .build();

    logger.info("Starting SSH reconnection test");

    let Some(config) = require_workers() else {
        logger.warn("Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.warn("Test skipped: no enabled worker found");
        return;
    };

    let worker_config = to_worker_config(worker_entry);
    let options = SshOptions {
        connect_timeout: Duration::from_secs(config.settings.ssh_connection_timeout_secs),
        known_hosts: KnownHostsPolicy::Add,
        ..Default::default()
    };

    let mut client = SshClient::new(worker_config, options);

    // First connection
    logger.info("Establishing first connection");
    client.connect().await.expect("First connect should succeed");
    assert!(client.is_connected(), "Should be connected");

    let result1 = client.execute("echo 'first'").await.expect("First command should work");
    assert!(result1.stdout.contains("first"));

    // Disconnect
    logger.info("Disconnecting");
    client.disconnect().await.expect("Disconnect should succeed");
    assert!(!client.is_connected(), "Should be disconnected");

    // Reconnect
    logger.info("Reconnecting");
    client.connect().await.expect("Reconnect should succeed");
    assert!(client.is_connected(), "Should be connected again");

    let result2 = client.execute("echo 'second'").await.expect("Second command should work");
    assert!(result2.stdout.contains("second"));

    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("ssh".to_string()),
        "Reconnection test results",
        vec![
            ("first_command".to_string(), "success".to_string()),
            ("disconnect".to_string(), "success".to_string()),
            ("reconnect".to_string(), "success".to_string()),
            ("second_command".to_string(), "success".to_string()),
        ],
    );

    client.disconnect().await.ok();
    logger.info("SSH reconnection test passed");
    logger.print_summary();
}
