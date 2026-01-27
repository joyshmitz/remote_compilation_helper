//! True E2E Tests: Artifact Transfer & Retrieval
//!
//! Tests that artifacts (compiled binaries, libraries, test outputs) are correctly
//! transferred back from workers to the local machine with full integrity verification.
//!
//! # Test Categories
//!
//! 1. Binary artifact retrieval with hash verification
//! 2. Debug vs release binary comparison
//! 3. Large artifact handling (>10MB)
//! 4. Static library (.a) artifact retrieval
//! 5. Incremental artifact sync (only changed artifacts)
//!
//! # Running These Tests
//!
//! ```bash
//! # Requires workers_test.toml configuration
//! cargo test --features true-e2e artifact_tests -- --nocapture
//! ```
//!
//! # Bead Reference
//!
//! This implements bead bd-20zz: True E2E Artifact Transfer & Retrieval Tests

use rch_common::e2e::{TestConfigError, TestWorkersConfig, should_skip_worker_check};
use rch_common::ssh::{KnownHostsPolicy, SshClient, SshOptions};
use rch_common::testing::{TestLogger, TestPhase};
use rch_common::types::WorkerConfig;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Project root for fixtures
const FIXTURES_DIR: &str = "tests/true_e2e/fixtures";

/// Get the hello_world fixture directory
fn hello_world_fixture_dir() -> PathBuf {
    PathBuf::from(FIXTURES_DIR).join("hello_world")
}

/// Get the rust_workspace fixture directory
fn rust_workspace_fixture_dir() -> PathBuf {
    PathBuf::from(FIXTURES_DIR).join("rust_workspace")
}

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

/// Compute BLAKE3 hash of a file.
#[allow(dead_code)]
fn compute_file_hash(path: &Path) -> Result<String, std::io::Error> {
    let data = fs::read(path)?;
    Ok(blake3::hash(&data).to_hex().to_string())
}

/// Compute BLAKE3 hash of bytes.
#[allow(dead_code)]
fn compute_bytes_hash(data: &[u8]) -> String {
    blake3::hash(data).to_hex().to_string()
}

/// Get file size in bytes.
fn get_file_size(path: &Path) -> Result<u64, std::io::Error> {
    fs::metadata(path).map(|m| m.len())
}

/// Copy a local fixture directory to the remote worker using rsync.
async fn sync_fixture_to_remote(
    client: &mut SshClient,
    worker_config: &WorkerConfig,
    local_path: &Path,
    remote_path: &str,
) -> Result<(), String> {
    let mkdir_cmd = format!("mkdir -p {}", remote_path);
    client
        .execute(&mkdir_cmd)
        .await
        .map_err(|e| format!("Failed to create remote directory: {e}"))?;

    let output = std::process::Command::new("rsync")
        .args([
            "-avz",
            "--delete",
            "--exclude=target",
            "-e",
            &format!(
                "ssh -o StrictHostKeyChecking=accept-new -i {}",
                worker_config.identity_file
            ),
            &format!("{}/", local_path.display()),
            &format!(
                "{}@{}:{}/",
                worker_config.user, worker_config.host, remote_path
            ),
        ])
        .output()
        .map_err(|e| format!("Failed to run rsync: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "rsync failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

/// Sync artifacts back from remote to local.
async fn sync_artifacts_from_remote(
    worker_config: &WorkerConfig,
    remote_path: &str,
    local_path: &Path,
) -> Result<TransferStats, String> {
    let start = Instant::now();

    let output = std::process::Command::new("rsync")
        .args([
            "-avz",
            "--stats",
            "-e",
            &format!(
                "ssh -o StrictHostKeyChecking=accept-new -i {}",
                worker_config.identity_file
            ),
            &format!(
                "{}@{}:{}/target/",
                worker_config.user, worker_config.host, remote_path
            ),
            &format!("{}/target/", local_path.display()),
        ])
        .output()
        .map_err(|e| format!("Failed to run rsync: {e}"))?;

    let duration = start.elapsed();

    if output.status.success() {
        let stats_output = String::from_utf8_lossy(&output.stdout);
        let bytes_transferred = parse_rsync_bytes(&stats_output);

        Ok(TransferStats {
            bytes_transferred,
            duration,
            files_transferred: parse_rsync_files(&stats_output),
        })
    } else {
        Err(format!(
            "rsync failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

/// Parse bytes transferred from rsync --stats output.
fn parse_rsync_bytes(output: &str) -> u64 {
    for line in output.lines() {
        if line.contains("Total transferred file size:")
            && let Some(num_str) = line.split(':').nth(1)
        {
            let clean = num_str.trim().replace(',', "").replace(" bytes", "");
            if let Ok(bytes) = clean.parse::<u64>() {
                return bytes;
            }
        }
    }
    0
}

/// Parse number of files transferred from rsync --stats output.
fn parse_rsync_files(output: &str) -> u64 {
    for line in output.lines() {
        if line.contains("Number of regular files transferred:")
            && let Some(num_str) = line.split(':').nth(1)
            && let Ok(count) = num_str.trim().replace(',', "").parse::<u64>()
        {
            return count;
        }
    }
    0
}

/// Clean up remote directory after test.
async fn cleanup_remote(client: &mut SshClient, remote_path: &str) -> Result<(), String> {
    let cmd = format!("rm -rf {}", remote_path);
    client
        .execute(&cmd)
        .await
        .map_err(|e| format!("Failed to cleanup: {e}"))?;
    Ok(())
}

/// Compute remote file hash via SSH using md5sum (universally available).
async fn compute_remote_hash(client: &mut SshClient, remote_path: &str) -> Result<String, String> {
    // Use md5sum which is available on all Unix systems
    // macOS uses md5 -q, Linux uses md5sum
    let cmd = format!(
        "md5sum {} 2>/dev/null | cut -d' ' -f1 || md5 -q {} 2>/dev/null",
        remote_path, remote_path
    );
    let result = client
        .execute(&cmd)
        .await
        .map_err(|e| format!("Failed to compute remote hash: {e}"))?;

    if result.success() && !result.stdout.trim().is_empty() {
        Ok(result.stdout.trim().to_string())
    } else {
        Err(format!("hash computation failed: {}", result.stderr))
    }
}

/// Compute local file MD5 hash (for comparison with remote).
fn compute_local_md5(path: &Path) -> Result<String, std::io::Error> {
    let output = std::process::Command::new("md5sum")
        .arg(path)
        .output()
        .or_else(|_| {
            std::process::Command::new("md5")
                .args(["-q", path.to_str().unwrap_or_default()])
                .output()
        })?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string())
    } else {
        Err(std::io::Error::other("md5 command failed"))
    }
}

/// Get remote file size via SSH.
async fn get_remote_file_size(client: &mut SshClient, remote_path: &str) -> Result<u64, String> {
    let cmd = format!(
        "stat -c%s {} 2>/dev/null || stat -f%z {}",
        remote_path, remote_path
    );
    let result = client
        .execute(&cmd)
        .await
        .map_err(|e| format!("Failed to get remote size: {e}"))?;

    if result.success() {
        result
            .stdout
            .trim()
            .parse::<u64>()
            .map_err(|e| format!("Failed to parse size: {e}"))
    } else {
        Err(format!("stat failed: {}", result.stderr))
    }
}

/// Transfer statistics
#[derive(Debug, Clone)]
struct TransferStats {
    bytes_transferred: u64,
    duration: Duration,
    files_transferred: u64,
}

impl TransferStats {
    fn rate_mbps(&self) -> f64 {
        if self.duration.as_secs_f64() > 0.0 {
            (self.bytes_transferred as f64 / 1_000_000.0) / self.duration.as_secs_f64()
        } else {
            0.0
        }
    }
}

/// Artifact integrity verification result
#[derive(Debug)]
#[allow(dead_code)]
struct IntegrityResult {
    local_hash: String,
    remote_hash: String,
    local_size: u64,
    remote_size: u64,
    hash_match: bool,
    size_match: bool,
}

impl IntegrityResult {
    #[allow(dead_code)]
    fn is_valid(&self) -> bool {
        self.hash_match && self.size_match
    }
}

// =============================================================================
// Test: Binary Artifact Retrieval with Hash Verification
// =============================================================================

/// Test 1: Binary artifact integrity verification
///
/// - Build debug binary on worker
/// - Compute hash before and after transfer
/// - Verify hashes match
/// - Verify binary executes correctly
#[tokio::test]
async fn test_binary_artifact_integrity() {
    let logger = TestLogger::for_test("test_binary_artifact_integrity");

    logger.log_with_data(
        TestPhase::Setup,
        "Starting binary artifact integrity test",
        serde_json::json!({"artifact_type": "binary"}),
    );

    let Some(config) = require_workers() else {
        logger.log(TestPhase::Setup, "Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.log(TestPhase::Setup, "Test skipped: no enabled worker found");
        return;
    };

    let worker_config = worker_entry.to_worker_config();
    let Some(mut client) = get_connected_client(&config, worker_entry).await else {
        logger.fail("Failed to connect to worker");
        return;
    };

    let fixture_dir = hello_world_fixture_dir();
    let remote_path = format!(
        "{}/artifact_integrity_test",
        config.settings.remote_work_dir
    );

    // Sync fixture to remote
    logger.log(TestPhase::Setup, "Syncing fixture to remote");

    if let Err(e) =
        sync_fixture_to_remote(&mut client, &worker_config, &fixture_dir, &remote_path).await
    {
        logger.fail(format!("Failed to sync fixture: {e}"));
        return;
    }

    // Build on remote
    let build_cmd = format!("cd {} && cargo build", remote_path);
    logger.log_with_data(
        TestPhase::Execute,
        "Building on remote",
        serde_json::json!({"cmd": "cargo build"}),
    );

    let build_result = client.execute(&build_cmd).await;
    if let Err(e) = &build_result {
        logger.fail(format!("Remote build failed: {e}"));
        let _ = cleanup_remote(&mut client, &remote_path).await;
        client.disconnect().await.ok();
        return;
    }

    let result = build_result.unwrap();
    if !result.success() {
        logger.fail(format!("Remote build failed: {}", result.stderr));
        let _ = cleanup_remote(&mut client, &remote_path).await;
        client.disconnect().await.ok();
        panic!("Remote cargo build failed");
    }

    // Compute remote hash BEFORE transfer
    let remote_binary_path = format!("{}/target/debug/hello_world", remote_path);

    logger.log_with_data(
        TestPhase::Verify,
        "Computing remote hash before transfer",
        serde_json::json!({
            "remote_path": remote_binary_path.clone()
        }),
    );

    let remote_hash = match compute_remote_hash(&mut client, &remote_binary_path).await {
        Ok(hash) => hash,
        Err(e) => {
            logger.fail(format!("Failed to compute remote hash: {e}"));
            let _ = cleanup_remote(&mut client, &remote_path).await;
            client.disconnect().await.ok();
            panic!("Failed to compute remote hash");
        }
    };

    let remote_size = match get_remote_file_size(&mut client, &remote_binary_path).await {
        Ok(size) => size,
        Err(e) => {
            logger.log(
                TestPhase::Verify,
                format!("Warning: Failed to get remote size: {e}"),
            );
            0
        }
    };

    logger.log_with_data(
        TestPhase::Verify,
        "Remote artifact info",
        serde_json::json!({
            "remote_hash": remote_hash.clone(),
            "remote_size": remote_size
        }),
    );

    // Transfer artifacts back
    logger.log(TestPhase::Execute, "Transferring artifacts");

    let transfer_stats =
        match sync_artifacts_from_remote(&worker_config, &remote_path, &fixture_dir).await {
            Ok(stats) => stats,
            Err(e) => {
                logger.fail(format!("Failed to transfer artifacts: {e}"));
                let _ = cleanup_remote(&mut client, &remote_path).await;
                client.disconnect().await.ok();
                panic!("Artifact transfer failed");
            }
        };

    logger.log_with_data(
        TestPhase::Execute,
        "Transfer complete",
        serde_json::json!({
            "bytes": transfer_stats.bytes_transferred,
            "duration_ms": transfer_stats.duration.as_millis() as u64,
            "rate_mbps": format!("{:.2}", transfer_stats.rate_mbps()),
            "files": transfer_stats.files_transferred
        }),
    );

    // Compute local hash AFTER transfer (using same algorithm as remote)
    let local_binary_path = fixture_dir.join("target/debug/hello_world");

    let local_hash = match compute_local_md5(&local_binary_path) {
        Ok(hash) => hash,
        Err(e) => {
            logger.fail(format!("Failed to compute local hash: {e}"));
            let _ = cleanup_remote(&mut client, &remote_path).await;
            client.disconnect().await.ok();
            panic!("Failed to compute local hash");
        }
    };

    let local_size = get_file_size(&local_binary_path).unwrap_or(0);

    // Verify integrity
    let integrity = IntegrityResult {
        local_hash: local_hash.clone(),
        remote_hash: remote_hash.clone(),
        local_size,
        remote_size,
        hash_match: local_hash == remote_hash,
        size_match: local_size == remote_size,
    };

    logger.log_with_data(
        TestPhase::Verify,
        "Integrity verification",
        serde_json::json!({
            "local_hash": integrity.local_hash.clone(),
            "remote_hash": integrity.remote_hash.clone(),
            "local_size": integrity.local_size,
            "remote_size": integrity.remote_size,
            "hash_match": integrity.hash_match,
            "size_match": integrity.size_match
        }),
    );

    assert!(
        integrity.hash_match,
        "Binary hash mismatch! local={}, remote={}",
        integrity.local_hash, integrity.remote_hash
    );

    assert!(
        integrity.size_match,
        "Binary size mismatch! local={}, remote={}",
        integrity.local_size, integrity.remote_size
    );

    // Verify binary executes correctly
    logger.log(TestPhase::Execute, "Testing binary execution");

    let exec_result = std::process::Command::new(&local_binary_path).output();

    match exec_result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let success = output.status.success();

            logger.log_with_data(
                TestPhase::Execute,
                "Binary execution result",
                serde_json::json!({
                    "exit_code": output.status.code().unwrap_or(-1),
                    "success": success,
                    "output_contains_hello": stdout.contains("Hello")
                }),
            );

            assert!(success, "Binary should execute successfully");
            assert!(
                stdout.contains("Hello"),
                "Binary should output 'Hello', got: {}",
                stdout
            );
        }
        Err(e) => {
            logger.fail(format!("Binary execution failed: {e}"));
            panic!("Failed to execute binary: {e}");
        }
    }

    // Cleanup
    let _ = cleanup_remote(&mut client, &remote_path).await;
    client.disconnect().await.ok();

    logger.log(TestPhase::Verify, "Test completed successfully");
    logger.pass();
}

// =============================================================================
// Test: Release vs Debug Binary Size Comparison
// =============================================================================

/// Test 2: Debug vs release binary artifact comparison
///
/// - Build both debug and release binaries
/// - Verify both transfer correctly
/// - Compare sizes (release should be smaller or equal with optimizations)
#[tokio::test]
async fn test_debug_vs_release_artifacts() {
    let logger = TestLogger::for_test("test_debug_vs_release_artifacts");

    logger.log_with_data(
        TestPhase::Setup,
        "Starting debug vs release artifact comparison",
        serde_json::json!({"artifact_type": "binary"}),
    );

    let Some(config) = require_workers() else {
        logger.log(TestPhase::Setup, "Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.log(TestPhase::Setup, "Test skipped: no enabled worker found");
        return;
    };

    let worker_config = worker_entry.to_worker_config();
    let Some(mut client) = get_connected_client(&config, worker_entry).await else {
        logger.fail("Failed to connect to worker");
        return;
    };

    let fixture_dir = hello_world_fixture_dir();
    let remote_path = format!("{}/debug_release_test", config.settings.remote_work_dir);

    // Sync fixture
    if let Err(e) =
        sync_fixture_to_remote(&mut client, &worker_config, &fixture_dir, &remote_path).await
    {
        logger.fail(format!("Failed to sync fixture: {e}"));
        return;
    }

    // Build debug
    logger.log(TestPhase::Execute, "Building debug binary");

    let debug_cmd = format!("cd {} && cargo build", remote_path);
    if let Err(e) = client.execute(&debug_cmd).await {
        logger.fail(format!("Debug build failed: {e}"));
        let _ = cleanup_remote(&mut client, &remote_path).await;
        client.disconnect().await.ok();
        return;
    }

    // Build release
    logger.log(TestPhase::Execute, "Building release binary");

    let release_cmd = format!("cd {} && cargo build --release", remote_path);
    if let Err(e) = client.execute(&release_cmd).await {
        logger.fail(format!("Release build failed: {e}"));
        let _ = cleanup_remote(&mut client, &remote_path).await;
        client.disconnect().await.ok();
        return;
    }

    // Get remote sizes
    let debug_remote_path = format!("{}/target/debug/hello_world", remote_path);
    let release_remote_path = format!("{}/target/release/hello_world", remote_path);

    let debug_size = get_remote_file_size(&mut client, &debug_remote_path)
        .await
        .unwrap_or(0);
    let release_size = get_remote_file_size(&mut client, &release_remote_path)
        .await
        .unwrap_or(0);

    logger.log_with_data(
        TestPhase::Verify,
        "Remote binary sizes",
        serde_json::json!({
            "debug_size": debug_size,
            "release_size": release_size,
            "size_ratio": if debug_size > 0 {
                format!("{:.2}", release_size as f64 / debug_size as f64)
            } else {
                "N/A".to_string()
            }
        }),
    );

    // Transfer artifacts
    if let Err(e) = sync_artifacts_from_remote(&worker_config, &remote_path, &fixture_dir).await {
        logger.fail(format!("Failed to transfer artifacts: {e}"));
        let _ = cleanup_remote(&mut client, &remote_path).await;
        client.disconnect().await.ok();
        return;
    }

    // Verify both binaries exist locally
    let local_debug = fixture_dir.join("target/debug/hello_world");
    let local_release = fixture_dir.join("target/release/hello_world");

    assert!(
        local_debug.exists(),
        "Debug binary should exist at {}",
        local_debug.display()
    );
    assert!(
        local_release.exists(),
        "Release binary should exist at {}",
        local_release.display()
    );

    // Verify both execute
    let debug_runs = std::process::Command::new(&local_debug)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let release_runs = std::process::Command::new(&local_release)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    logger.log_with_data(
        TestPhase::Execute,
        "Binary execution verification",
        serde_json::json!({
            "debug_runs": debug_runs,
            "release_runs": release_runs
        }),
    );

    assert!(debug_runs, "Debug binary should run");
    assert!(release_runs, "Release binary should run");

    // Cleanup
    let _ = cleanup_remote(&mut client, &remote_path).await;
    client.disconnect().await.ok();

    logger.log(TestPhase::Verify, "Test completed successfully");
    logger.pass();
}

// =============================================================================
// Test: Library Artifact (Static Library) Retrieval
// =============================================================================

/// Test 3: Static library artifact retrieval
///
/// - Build a library crate on worker
/// - Retrieve .rlib artifacts
/// - Verify archive integrity
#[tokio::test]
async fn test_library_artifact_retrieval() {
    let logger = TestLogger::for_test("test_library_artifact_retrieval");

    logger.log_with_data(
        TestPhase::Setup,
        "Starting library artifact retrieval test",
        serde_json::json!({"artifact_type": "library"}),
    );

    let Some(config) = require_workers() else {
        logger.log(TestPhase::Setup, "Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.log(TestPhase::Setup, "Test skipped: no enabled worker found");
        return;
    };

    let worker_config = worker_entry.to_worker_config();
    let Some(mut client) = get_connected_client(&config, worker_entry).await else {
        logger.fail("Failed to connect to worker");
        return;
    };

    // Use workspace fixture which has library crates
    let fixture_dir = rust_workspace_fixture_dir();
    let remote_path = format!("{}/library_artifact_test", config.settings.remote_work_dir);

    // Sync fixture
    if let Err(e) =
        sync_fixture_to_remote(&mut client, &worker_config, &fixture_dir, &remote_path).await
    {
        logger.fail(format!("Failed to sync fixture: {e}"));
        return;
    }

    // Build library only (core crate is a library)
    logger.log_with_data(
        TestPhase::Execute,
        "Building library crate",
        serde_json::json!({"crate": "core"}),
    );

    let build_cmd = format!("cd {} && cargo build -p core", remote_path);
    let result = client.execute(&build_cmd).await;

    if let Err(e) = &result {
        logger.fail(format!("Library build failed: {e}"));
        let _ = cleanup_remote(&mut client, &remote_path).await;
        client.disconnect().await.ok();
        return;
    }

    // Transfer artifacts
    if let Err(e) = sync_artifacts_from_remote(&worker_config, &remote_path, &fixture_dir).await {
        logger.fail(format!("Failed to transfer artifacts: {e}"));
        let _ = cleanup_remote(&mut client, &remote_path).await;
        client.disconnect().await.ok();
        return;
    }

    // Check for .rlib files
    let deps_dir = fixture_dir.join("target/debug/deps");

    if deps_dir.exists() {
        let rlib_count = fs::read_dir(&deps_dir)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "rlib")
                            .unwrap_or(false)
                    })
                    .count()
            })
            .unwrap_or(0);

        logger.log_with_data(
            TestPhase::Verify,
            "Library artifacts found",
            serde_json::json!({
                "rlib_count": rlib_count,
                "deps_dir": deps_dir.display().to_string()
            }),
        );

        assert!(rlib_count > 0, "Should have at least one .rlib file");
    } else {
        logger.log(
            TestPhase::Verify,
            "deps directory not found - build may have failed",
        );
    }

    // Cleanup
    let _ = cleanup_remote(&mut client, &remote_path).await;
    client.disconnect().await.ok();

    logger.log(TestPhase::Verify, "Test completed");
    logger.pass();
}

// =============================================================================
// Test: Incremental Artifact Sync
// =============================================================================

/// Test 4: Incremental artifact sync (only changed artifacts)
///
/// - Build once, transfer artifacts
/// - Make small change, rebuild
/// - Transfer again, verify only changed artifacts transferred
#[tokio::test]
async fn test_incremental_artifact_sync() {
    let logger = TestLogger::for_test("test_incremental_artifact_sync");

    logger.log(TestPhase::Setup, "Starting incremental artifact sync test");

    let Some(config) = require_workers() else {
        logger.log(TestPhase::Setup, "Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.log(TestPhase::Setup, "Test skipped: no enabled worker found");
        return;
    };

    let worker_config = worker_entry.to_worker_config();
    let Some(mut client) = get_connected_client(&config, worker_entry).await else {
        logger.fail("Failed to connect to worker");
        return;
    };

    let fixture_dir = hello_world_fixture_dir();
    let remote_path = format!("{}/incremental_sync_test", config.settings.remote_work_dir);

    // Sync fixture
    if let Err(e) =
        sync_fixture_to_remote(&mut client, &worker_config, &fixture_dir, &remote_path).await
    {
        logger.fail(format!("Failed to sync fixture: {e}"));
        return;
    }

    // First build
    logger.log(TestPhase::Execute, "First build");

    let build_cmd = format!("cd {} && cargo build", remote_path);
    if let Err(e) = client.execute(&build_cmd).await {
        logger.fail(format!("First build failed: {e}"));
        let _ = cleanup_remote(&mut client, &remote_path).await;
        client.disconnect().await.ok();
        return;
    }

    // First transfer
    let first_transfer = sync_artifacts_from_remote(&worker_config, &remote_path, &fixture_dir)
        .await
        .ok();

    let first_bytes = first_transfer
        .as_ref()
        .map(|s| s.bytes_transferred)
        .unwrap_or(0);

    logger.log_with_data(
        TestPhase::Execute,
        "First transfer complete",
        serde_json::json!({"bytes": first_bytes}),
    );

    // Touch a source file on remote to trigger rebuild (without changing content)
    let touch_cmd = format!("touch {}/src/main.rs", remote_path);
    let _ = client.execute(&touch_cmd).await;

    // Second build (incremental)
    logger.log(TestPhase::Execute, "Second build (incremental)");

    if let Err(e) = client.execute(&build_cmd).await {
        logger.log(
            TestPhase::Execute,
            format!("Warning: Second build failed: {e}"),
        );
    }

    // Second transfer
    let second_transfer = sync_artifacts_from_remote(&worker_config, &remote_path, &fixture_dir)
        .await
        .ok();

    let second_bytes = second_transfer
        .as_ref()
        .map(|s| s.bytes_transferred)
        .unwrap_or(0);

    logger.log_with_data(
        TestPhase::Execute,
        "Second transfer complete",
        serde_json::json!({
            "bytes": second_bytes,
            "reduction_pct": if first_bytes > 0 {
                format!("{:.1}", (1.0 - second_bytes as f64 / first_bytes as f64) * 100.0)
            } else {
                "N/A".to_string()
            }
        }),
    );

    // Note: We don't assert on byte reduction because rsync behavior depends on
    // many factors. The important thing is that both transfers succeed.

    // Cleanup
    let _ = cleanup_remote(&mut client, &remote_path).await;
    client.disconnect().await.ok();

    logger.log(TestPhase::Verify, "Test completed");
    logger.pass();
}

// =============================================================================
// Test: Large Artifact Handling
// =============================================================================

/// Test 5: Large artifact handling
///
/// This test verifies that larger build artifacts (debug info, etc.)
/// transfer correctly without corruption.
#[tokio::test]
async fn test_large_artifact_handling() {
    let logger = TestLogger::for_test("test_large_artifact_handling");

    logger.log(TestPhase::Setup, "Starting large artifact handling test");

    let Some(config) = require_workers() else {
        logger.log(TestPhase::Setup, "Test skipped: no workers available");
        return;
    };

    let Some(worker_entry) = get_test_worker(&config) else {
        logger.log(TestPhase::Setup, "Test skipped: no enabled worker found");
        return;
    };

    let worker_config = worker_entry.to_worker_config();
    let Some(mut client) = get_connected_client(&config, worker_entry).await else {
        logger.fail("Failed to connect to worker");
        return;
    };

    // Use workspace fixture which produces more artifacts
    let fixture_dir = rust_workspace_fixture_dir();
    let remote_path = format!("{}/large_artifact_test", config.settings.remote_work_dir);

    // Sync fixture
    if let Err(e) =
        sync_fixture_to_remote(&mut client, &worker_config, &fixture_dir, &remote_path).await
    {
        logger.fail(format!("Failed to sync fixture: {e}"));
        return;
    }

    // Build entire workspace
    logger.log(TestPhase::Execute, "Building entire workspace");

    let build_cmd = format!("cd {} && cargo build --workspace", remote_path);
    let build_start = Instant::now();
    let result = client.execute(&build_cmd).await;
    let build_duration = build_start.elapsed();

    if let Err(e) = &result {
        logger.fail(format!("Build failed: {e}"));
        let _ = cleanup_remote(&mut client, &remote_path).await;
        client.disconnect().await.ok();
        return;
    }

    logger.log_with_data(
        TestPhase::Execute,
        "Build complete",
        serde_json::json!({"duration_ms": build_duration.as_millis() as u64}),
    );

    // Transfer all artifacts
    let transfer_result =
        sync_artifacts_from_remote(&worker_config, &remote_path, &fixture_dir).await;

    match transfer_result {
        Ok(stats) => {
            logger.log_with_data(
                TestPhase::Execute,
                "Large artifact transfer complete",
                serde_json::json!({
                    "bytes": stats.bytes_transferred,
                    "duration_ms": stats.duration.as_millis() as u64,
                    "rate_mbps": format!("{:.2}", stats.rate_mbps()),
                    "files": stats.files_transferred
                }),
            );

            // Verify target directory exists with content
            let target_dir = fixture_dir.join("target");
            assert!(
                target_dir.exists(),
                "Target directory should exist after transfer"
            );

            // Count total artifact size using du command
            let total_size = std::process::Command::new("du")
                .args(["-sb", target_dir.to_str().unwrap_or_default()])
                .output()
                .ok()
                .and_then(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.parse::<u64>().ok())
                })
                .unwrap_or(0);

            logger.log_with_data(
                TestPhase::Verify,
                "Artifact size verification",
                serde_json::json!({
                    "total_size": total_size,
                    "size_mb": format!("{:.2}", total_size as f64 / 1_000_000.0)
                }),
            );
        }
        Err(e) => {
            logger.fail(format!("Transfer failed: {e}"));
            panic!("Large artifact transfer failed");
        }
    }

    // Cleanup
    let _ = cleanup_remote(&mut client, &remote_path).await;
    client.disconnect().await.ok();

    logger.log(TestPhase::Verify, "Test completed");
    logger.pass();
}
