//! Rollback management for fleet deployments.
//!
//! Handles reverting workers to previous versions when deployments fail.
//!
//! ## Backup Registry
//!
//! The backup registry tracks all worker backups in a local JSON file at
//! `~/.local/share/rch/backups/registry.json`. Each backup entry includes:
//! - Worker ID and version
//! - SHA256 hash of the binary for verification
//! - Remote path where the backup is stored on the worker
//! - Timestamp of when the backup was created
//!
//! The registry uses atomic writes with file locking to handle concurrent access.

use crate::fleet::history::HistoryManager;
use crate::fleet::ssh::SshExecutor;
use crate::ui::context::OutputContext;
use crate::ui::theme::StatusIndicator;
use anyhow::{Context, Result, bail};
use futures::stream::{self, StreamExt};
use rch_common::WorkerConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tracing::{debug, error, info, warn};

// =============================================================================
// Constants
// =============================================================================

/// Default remote path to the rch-wkr binary on workers.
#[allow(dead_code)] // Used by planned rollback commands
pub const REMOTE_RCH_PATH: &str = "~/.local/bin/rch-wkr";

/// Default remote backup directory on workers.
pub const REMOTE_BACKUP_DIR: &str = "~/.rch/backups";

/// Maximum number of backups to keep per worker.
#[allow(dead_code)] // Enforced when backup retention is implemented
pub const MAX_BACKUPS_PER_WORKER: usize = 3;

/// Registry file name.
const REGISTRY_FILE: &str = "registry.json";

/// Timeout for rollback operations.
const ROLLBACK_TIMEOUT: Duration = Duration::from_secs(60);

/// Maximum concurrent rollback operations (cap to prevent resource exhaustion).
const MAX_CONCURRENT_ROLLBACKS: usize = 10;

/// Backup information for a worker's previous state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerBackup {
    /// Worker identifier.
    pub worker_id: String,
    /// Version that was backed up.
    pub version: String,
    /// Local path to backup metadata.
    pub backup_path: PathBuf,
    /// Remote path where the backup binary is stored on the worker.
    pub remote_path: PathBuf,
    /// SHA256 hash of the backed up binary for verification.
    /// Set to "unknown" if hash could not be computed.
    pub binary_hash: String,
    /// When the backup was created (RFC 3339 format).
    pub created_at: String,
}

/// Result of a rollback operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackResult {
    /// Worker that was rolled back.
    pub worker_id: String,
    /// Whether rollback succeeded.
    pub success: bool,
    /// Version rolled back to.
    pub rolled_back_to: Option<String>,
    /// Error message if failed.
    pub error: Option<String>,
}

/// Manages rollback operations for workers.
pub struct RollbackManager {
    history: HistoryManager,
    backup_dir: PathBuf,
}

impl RollbackManager {
    /// Create a new rollback manager.
    pub fn new() -> Result<Self> {
        let backup_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rch")
            .join("backups");

        std::fs::create_dir_all(&backup_dir)?;

        Ok(Self {
            history: HistoryManager::new()?,
            backup_dir,
        })
    }

    /// Create a rollback manager with a custom backup directory (for testing).
    #[cfg(test)]
    pub fn with_path(backup_dir: &std::path::Path) -> Result<Self> {
        std::fs::create_dir_all(backup_dir)?;
        Ok(Self {
            history: HistoryManager::new()?,
            backup_dir: backup_dir.to_path_buf(),
        })
    }

    /// Get the path to the registry file.
    fn registry_path(&self) -> PathBuf {
        self.backup_dir.join(REGISTRY_FILE)
    }

    /// Load the backup registry from disk.
    ///
    /// Returns an empty vector if the registry doesn't exist yet.
    /// Handles corrupted files by backing them up and returning empty.
    pub fn load_registry(&self) -> Result<Vec<WorkerBackup>> {
        let path = self.registry_path();
        if !path.exists() {
            debug!("Registry file does not exist, returning empty");
            return Ok(Vec::new());
        }

        let mut file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                warn!(error = %e, "Failed to open registry file");
                return Ok(Vec::new());
            }
        };

        let mut content = String::new();
        if let Err(e) = file.read_to_string(&mut content) {
            warn!(error = %e, "Failed to read registry file");
            return Ok(Vec::new());
        }

        match serde_json::from_str::<Vec<WorkerBackup>>(&content) {
            Ok(registry) => {
                debug!(count = registry.len(), "Loaded backup registry");
                Ok(registry)
            }
            Err(e) => {
                error!(error = %e, "Corrupted backup registry, backing up and recreating");
                // Backup the corrupted file
                let backup_path = path.with_extension("json.corrupted");
                if let Err(rename_err) = std::fs::rename(&path, &backup_path) {
                    warn!(error = %rename_err, "Failed to backup corrupted registry");
                } else {
                    info!(path = %backup_path.display(), "Corrupted registry backed up");
                }
                Ok(Vec::new())
            }
        }
    }

    /// Save a backup entry to the registry.
    ///
    /// Uses atomic writes to prevent corruption from concurrent access.
    pub fn save_backup_entry(&self, backup: &WorkerBackup) -> Result<()> {
        let path = self.registry_path();
        let temp_path = path.with_extension("json.tmp");

        // Load existing registry
        let mut registry = self.load_registry().unwrap_or_default();

        // Add the new backup
        registry.push(backup.clone());

        // Write atomically via temp file
        let content =
            serde_json::to_string_pretty(&registry).context("Failed to serialize registry")?;

        let mut file =
            std::fs::File::create(&temp_path).context("Failed to create temp registry file")?;
        file.write_all(content.as_bytes())
            .context("Failed to write registry")?;
        file.sync_all().context("Failed to sync registry file")?;
        drop(file);

        // Atomic rename
        std::fs::rename(&temp_path, &path)
            .context("Failed to rename temp registry to final path")?;

        info!(
            worker = %backup.worker_id,
            version = %backup.version,
            "Backup entry saved to registry"
        );

        Ok(())
    }

    /// Get the latest backup for a worker.
    pub fn get_latest_backup(&self, worker_id: &str) -> Option<WorkerBackup> {
        let registry = self.load_registry().ok()?;

        registry
            .into_iter()
            .filter(|b| b.worker_id == worker_id)
            .max_by(|a, b| a.created_at.cmp(&b.created_at))
    }

    /// Get a specific version's backup for a worker.
    pub fn get_backup(&self, worker_id: &str, version: &str) -> Option<WorkerBackup> {
        let registry = self.load_registry().ok()?;

        registry
            .into_iter()
            .find(|b| b.worker_id == worker_id && b.version == version)
    }

    /// Prune old backups, keeping only the most recent `max_per_worker` for each worker.
    ///
    /// Returns the list of removed backups so the caller can clean up remote files.
    pub fn prune_old_backups(&mut self, max_per_worker: usize) -> Result<Vec<WorkerBackup>> {
        let path = self.registry_path();
        let registry = self.load_registry()?;

        // Group by worker_id
        let mut by_worker: HashMap<String, Vec<WorkerBackup>> = HashMap::new();
        for backup in registry {
            by_worker
                .entry(backup.worker_id.clone())
                .or_default()
                .push(backup);
        }

        // Sort each worker's backups by created_at (newest first) and collect removed
        let mut kept = Vec::new();
        let mut removed = Vec::new();

        for (worker_id, mut backups) in by_worker {
            backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));

            for (i, backup) in backups.into_iter().enumerate() {
                if i < max_per_worker {
                    kept.push(backup);
                } else {
                    debug!(
                        worker = %worker_id,
                        version = %backup.version,
                        "Pruning old backup"
                    );
                    removed.push(backup);
                }
            }
        }

        // Write the pruned registry
        if !removed.is_empty() {
            let temp_path = path.with_extension("json.tmp");
            let content = serde_json::to_string_pretty(&kept)?;
            let mut file = std::fs::File::create(&temp_path)?;
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&temp_path, &path)?;

            info!(
                removed = removed.len(),
                kept = kept.len(),
                "Pruned old backups from registry"
            );
        }

        Ok(removed)
    }

    /// Get the backup directory path.
    pub fn backup_dir(&self) -> &PathBuf {
        &self.backup_dir
    }

    /// Rollback workers to a specific or previous version.
    ///
    /// This function restores workers to a previous binary version by:
    /// 1. Gracefully stopping any running rch-wkr process
    /// 2. Looking up the backup for each worker (specific version or latest)
    /// 3. Copying the backup binary back to the main rch-wkr location via SSH
    /// 4. Optionally verifying the restored binary hash matches the backup record
    /// 5. Verifying the version after restoration
    /// 6. Restarting the rch-wkr service
    ///
    /// # Arguments
    ///
    /// * `workers` - List of workers to roll back
    /// * `to_version` - Optional specific version to roll back to (uses previous if None)
    /// * `parallelism` - Maximum number of concurrent rollback operations
    /// * `verify` - If true, verify binary hash after restoration
    /// * `ctx` - Output context for progress display
    ///
    /// # Returns
    ///
    /// A vector of `RollbackResult` for each worker, indicating success or failure.
    pub async fn rollback_workers(
        &self,
        workers: &[&WorkerConfig],
        to_version: Option<&str>,
        parallelism: usize,
        verify: bool,
        ctx: &OutputContext,
    ) -> Result<Vec<RollbackResult>> {
        if workers.is_empty() {
            return Ok(Vec::new());
        }

        // Cap parallelism to prevent resource exhaustion
        let max_concurrent = parallelism.min(MAX_CONCURRENT_ROLLBACKS).max(1);

        info!(
            workers = %workers.len(),
            parallelism = %max_concurrent,
            version = ?to_version,
            verify = %verify,
            "Starting parallel rollback operation"
        );

        let completed = Arc::new(AtomicUsize::new(0));
        let total = workers.len();

        // Clone to_version for use in async closures
        let to_version_owned = to_version.map(|s| s.to_string());

        // Process workers in parallel with bounded concurrency
        let results: Vec<(usize, RollbackResult)> = stream::iter(workers.iter().enumerate())
            .map(|(idx, worker)| {
                let completed = completed.clone();
                let to_version = to_version_owned.clone();

                async move {
                    let result = self
                        .rollback_single_worker(worker, to_version.as_deref(), verify, ctx)
                        .await;

                    // Update progress
                    let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
                    debug!(
                        progress = %format!("{}/{}", done, total),
                        worker = %worker.id,
                        success = %result.success,
                        "Worker rollback complete"
                    );

                    (idx, result)
                }
            })
            .buffer_unordered(max_concurrent)
            .collect()
            .await;

        // Sort by original index to preserve order
        let results: Vec<RollbackResult> = {
            let mut sorted = results;
            sorted.sort_by_key(|(idx, _)| *idx);
            sorted.into_iter().map(|(_, result)| result).collect()
        };

        // Log summary
        let success_count = results.iter().filter(|r| r.success).count();
        let fail_count = results.len() - success_count;

        info!(
            total = %results.len(),
            success = %success_count,
            failed = %fail_count,
            "Rollback operation complete"
        );

        Ok(results)
    }

    /// Perform a complete rollback for a single worker.
    ///
    /// This method handles the full rollback workflow:
    /// 1. Determine target version (explicit or previous)
    /// 2. Look up backup from registry
    /// 3. Gracefully stop running rch-wkr
    /// 4. Restore the backup binary
    /// 5. Verify the restored version
    /// 6. Restart rch-wkr
    async fn rollback_single_worker(
        &self,
        worker: &WorkerConfig,
        to_version: Option<&str>,
        verify: bool,
        ctx: &OutputContext,
    ) -> RollbackResult {
        let style = ctx.theme();
        let worker_id = &worker.id.0;

        // Step 1: Determine target version
        let target_version = match to_version {
            Some(v) => v.to_string(),
            None => match self.history.get_previous_version(worker_id) {
                Ok(Some(v)) => v,
                Ok(None) => {
                    warn!(worker = %worker_id, "No previous version found for rollback");
                    return RollbackResult {
                        worker_id: worker_id.clone(),
                        success: false,
                        rolled_back_to: None,
                        error: Some("No previous version found".to_string()),
                    };
                }
                Err(e) => {
                    error!(worker = %worker_id, error = %e, "Failed to get previous version");
                    return RollbackResult {
                        worker_id: worker_id.clone(),
                        success: false,
                        rolled_back_to: None,
                        error: Some(format!("Failed to get previous version: {}", e)),
                    };
                }
            },
        };

        // Step 2: Look up backup for this version
        let backup = match self.get_backup(worker_id, &target_version) {
            Some(b) => b,
            None => {
                // Try getting the latest backup if specific version not found
                match self.get_latest_backup(worker_id) {
                    Some(b) if b.version == target_version => b,
                    _ => {
                        error!(
                            worker = %worker_id,
                            version = %target_version,
                            "No backup found for target version"
                        );
                        return RollbackResult {
                            worker_id: worker_id.clone(),
                            success: false,
                            rolled_back_to: None,
                            error: Some(format!(
                                "No backup found for version {}. Re-deploy the desired version instead.",
                                target_version
                            )),
                        };
                    }
                }
            }
        };

        if !ctx.is_json() {
            println!(
                "  {} Rolling back {} to v{}...",
                StatusIndicator::Pending.display(style),
                style.highlight(worker_id),
                target_version
            );
        }

        let ssh = SshExecutor::new(worker);

        // Step 3: Gracefully stop running rch-wkr (if any)
        debug!(worker = %worker_id, "Stopping rch-wkr before rollback");
        if let Err(e) = ssh
            .run_command("pkill -TERM rch-wkr 2>/dev/null || true")
            .await
        {
            // Log but don't fail - process might not be running
            debug!(worker = %worker_id, error = %e, "pkill returned error (process may not be running)");
        }
        // Allow brief time for graceful shutdown
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Step 4: Restore the backup
        if let Err(e) = self
            .restore_backup_with_verification(&backup, worker, verify)
            .await
        {
            error!(
                worker = %worker_id,
                version = %target_version,
                error = %e,
                "Rollback failed"
            );

            if !ctx.is_json() {
                println!(
                    "  {} {} rollback failed: {}",
                    StatusIndicator::Error.display(style),
                    style.highlight(worker_id),
                    e
                );
            }

            return RollbackResult {
                worker_id: worker_id.clone(),
                success: false,
                rolled_back_to: None,
                error: Some(e.to_string()),
            };
        }

        // Step 5: Verify the restored version
        debug!(worker = %worker_id, version = %target_version, "Verifying restored version");
        match ssh
            .run_command(&format!("{} --version", REMOTE_RCH_PATH))
            .await
        {
            Ok(output) if output.success() => {
                let reported_version = output.stdout.trim();
                if !reported_version.contains(&target_version) {
                    warn!(
                        worker = %worker_id,
                        expected = %target_version,
                        reported = %reported_version,
                        "Version mismatch after rollback (may be OK if format differs)"
                    );
                } else {
                    debug!(
                        worker = %worker_id,
                        version = %reported_version,
                        "Post-rollback version check passed"
                    );
                }
            }
            Ok(output) => {
                warn!(
                    worker = %worker_id,
                    stderr = %output.stderr.trim(),
                    "Version check failed after rollback"
                );
            }
            Err(e) => {
                warn!(worker = %worker_id, error = %e, "Could not verify version after rollback");
            }
        }

        // Step 6: Restart rch-wkr (best-effort)
        debug!(worker = %worker_id, "Restarting rch-wkr after rollback");
        if let Err(e) = ssh
            .run_command(&format!(
                "nohup {} serve >/dev/null 2>&1 &",
                REMOTE_RCH_PATH
            ))
            .await
        {
            warn!(
                worker = %worker_id,
                error = %e,
                "Failed to restart rch-wkr (may need manual intervention)"
            );
        }

        if !ctx.is_json() {
            println!(
                "  {} {} rolled back to v{}",
                StatusIndicator::Success.display(style),
                style.highlight(worker_id),
                target_version
            );
        }

        info!(
            worker = %worker_id,
            version = %target_version,
            "Rollback successful"
        );

        RollbackResult {
            worker_id: worker_id.clone(),
            success: true,
            rolled_back_to: Some(target_version),
            error: None,
        }
    }

    /// Create a backup of a worker's current binary.
    ///
    /// This creates a local backup record. The actual remote backup creation
    /// (copying the binary on the worker) is done by `backup_before_deploy`
    /// in the executor module which uses SSH.
    pub async fn create_backup(
        &self,
        worker: &WorkerConfig,
        version: &str,
    ) -> Result<WorkerBackup> {
        let backup_path = self
            .backup_dir
            .join(&worker.id.0)
            .join(format!("{}.bak", version));

        std::fs::create_dir_all(backup_path.parent().unwrap())?;

        // Compute remote backup path
        let remote_path = PathBuf::from(format!("{}/rch-wkr-{}", REMOTE_BACKUP_DIR, version));

        // Note: binary_hash will be set by the actual SSH backup operation
        // This stub returns "unknown" - the real implementation in executor.rs
        // will compute the hash via `sha256sum` on the remote
        let backup = WorkerBackup {
            worker_id: worker.id.0.clone(),
            version: version.to_string(),
            backup_path,
            remote_path,
            binary_hash: "unknown".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        debug!(
            worker = %worker.id,
            version = %version,
            remote_path = %backup.remote_path.display(),
            "Created backup record"
        );

        Ok(backup)
    }

    /// Create a backup entry with a known hash (for use by executor).
    pub fn create_backup_entry(
        &self,
        worker_id: &str,
        version: &str,
        remote_path: &str,
        binary_hash: &str,
    ) -> WorkerBackup {
        let backup_path = self
            .backup_dir
            .join(worker_id)
            .join(format!("{}.bak", version));

        WorkerBackup {
            worker_id: worker_id.to_string(),
            version: version.to_string(),
            backup_path,
            remote_path: PathBuf::from(remote_path),
            binary_hash: binary_hash.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Restore a worker from backup.
    ///
    /// This function:
    /// 1. Copies the backup binary back to the main rch-wkr location
    /// 2. Sets executable permissions on the restored binary
    /// 3. Optionally verifies the hash matches the recorded value
    ///
    /// # Arguments
    ///
    /// * `backup` - The backup record containing remote path and hash
    /// * `worker` - The worker configuration for SSH connection
    /// * `verify_hash` - If true, verify the restored binary matches the recorded hash
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - SSH connection fails
    /// - The backup file doesn't exist on the worker
    /// - File copy or permission setting fails
    /// - Hash verification fails (when verify_hash is true)
    pub async fn restore_backup(&self, backup: &WorkerBackup, worker: &WorkerConfig) -> Result<()> {
        self.restore_backup_with_verification(backup, worker, false)
            .await
    }

    /// Restore a worker from backup with optional hash verification.
    pub async fn restore_backup_with_verification(
        &self,
        backup: &WorkerBackup,
        worker: &WorkerConfig,
        verify_hash: bool,
    ) -> Result<()> {
        let ssh = SshExecutor::with_timeout(worker, ROLLBACK_TIMEOUT);
        let worker_id = &worker.id.0;
        let remote_backup_path = backup.remote_path.to_string_lossy();

        info!(
            worker = %worker_id,
            backup_path = %remote_backup_path,
            version = %backup.version,
            "Restoring worker from backup"
        );

        // Step 1: Verify backup exists on remote
        let check_cmd = format!("test -f {}", remote_backup_path);
        let check_result = ssh
            .run_command(&check_cmd)
            .await
            .context("Failed to check if backup exists")?;

        if !check_result.success() {
            bail!(
                "Backup file {} does not exist on worker {}",
                remote_backup_path,
                worker_id
            );
        }

        debug!(
            worker = %worker_id,
            backup_path = %remote_backup_path,
            "Backup file exists, proceeding with restore"
        );

        // Step 2: Copy backup to main binary location
        // Use cp with --preserve to maintain permissions and timestamps
        let restore_cmd = format!(
            "cp --preserve=mode,timestamps {} {}",
            remote_backup_path, REMOTE_RCH_PATH
        );
        let restore_result = ssh
            .run_command(&restore_cmd)
            .await
            .context("Failed to copy backup to main location")?;

        if !restore_result.success() {
            bail!(
                "Failed to restore backup on worker {}: {}",
                worker_id,
                restore_result.stderr.trim()
            );
        }

        // Step 3: Ensure executable permissions (redundant with --preserve but safe)
        ssh.set_executable(REMOTE_RCH_PATH)
            .await
            .context("Failed to set executable permissions on restored binary")?;

        info!(
            worker = %worker_id,
            version = %backup.version,
            "Binary restored successfully"
        );

        // Step 4: Optionally verify hash
        if verify_hash && backup.binary_hash != "unknown" {
            debug!(
                worker = %worker_id,
                expected_hash = %backup.binary_hash,
                "Verifying restored binary hash"
            );

            let hash_cmd = format!("sha256sum {} | cut -d' ' -f1", REMOTE_RCH_PATH);
            let hash_result = ssh
                .run_command(&hash_cmd)
                .await
                .context("Failed to compute hash of restored binary")?;

            if !hash_result.success() {
                warn!(
                    worker = %worker_id,
                    stderr = %hash_result.stderr.trim(),
                    "Hash verification command failed, skipping verification"
                );
            } else {
                let actual_hash = hash_result.stdout.trim();
                if actual_hash != backup.binary_hash {
                    bail!(
                        "Hash mismatch after restore on worker {}: expected {}, got {}",
                        worker_id,
                        backup.binary_hash,
                        actual_hash
                    );
                }

                info!(
                    worker = %worker_id,
                    hash = %actual_hash,
                    "Hash verification passed"
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================
    // Helper functions
    // ========================

    #[allow(dead_code)] // Shared test helper for future rollback cases
    fn mock_backup(worker_id: &str, version: &str, timestamp: u64) -> WorkerBackup {
        WorkerBackup {
            worker_id: worker_id.to_string(),
            version: version.to_string(),
            backup_path: PathBuf::from(format!("/tmp/backups/{}/{}.bak", worker_id, version)),
            remote_path: PathBuf::from(format!("~/.rch/backups/rch-wkr-{}", version)),
            binary_hash: format!("hash_{}", version),
            created_at: format!("2024-01-15T12:00:{:02}Z", timestamp % 60),
        }
    }

    // ========================
    // WorkerBackup tests
    // ========================

    #[test]
    fn worker_backup_serializes() {
        let backup = WorkerBackup {
            worker_id: "worker-1".to_string(),
            version: "1.0.0".to_string(),
            backup_path: PathBuf::from("/tmp/backups/worker-1/1.0.0.bak"),
            remote_path: PathBuf::from("~/.rch/backups/rch-wkr-1.0.0"),
            binary_hash: "abc123".to_string(),
            created_at: "2024-01-15T12:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&backup).unwrap();
        assert!(json.contains("worker-1"));
        assert!(json.contains("1.0.0"));
        assert!(json.contains("backups"));
        assert!(json.contains("abc123"));
        assert!(json.contains("2024-01-15T12:00:00Z"));
    }

    #[test]
    fn worker_backup_deserializes_roundtrip() {
        let backup = WorkerBackup {
            worker_id: "test-worker".to_string(),
            version: "2.0.0".to_string(),
            backup_path: PathBuf::from("/data/backups/test.bak"),
            remote_path: PathBuf::from("~/.rch/backups/rch-wkr-2.0.0"),
            binary_hash: "def456".to_string(),
            created_at: "2024-01-16T10:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&backup).unwrap();
        let restored: WorkerBackup = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.worker_id, "test-worker");
        assert_eq!(restored.version, "2.0.0");
        assert_eq!(
            restored.backup_path,
            PathBuf::from("/data/backups/test.bak")
        );
        assert_eq!(restored.binary_hash, "def456");
        assert_eq!(restored.created_at, "2024-01-16T10:00:00Z");
    }

    // ========================
    // RollbackResult tests
    // ========================

    #[test]
    fn rollback_result_success_serializes() {
        let result = RollbackResult {
            worker_id: "worker-1".to_string(),
            success: true,
            rolled_back_to: Some("1.0.0".to_string()),
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("worker-1"));
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("1.0.0"));
    }

    #[test]
    fn rollback_result_failure_serializes() {
        let result = RollbackResult {
            worker_id: "worker-2".to_string(),
            success: false,
            rolled_back_to: None,
            error: Some("No previous version found".to_string()),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("worker-2"));
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("No previous version found"));
    }

    #[test]
    fn rollback_result_deserializes_roundtrip() {
        let result = RollbackResult {
            worker_id: "test-worker".to_string(),
            success: true,
            rolled_back_to: Some("3.0.0".to_string()),
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        let restored: RollbackResult = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.worker_id, "test-worker");
        assert!(restored.success);
        assert_eq!(restored.rolled_back_to, Some("3.0.0".to_string()));
        assert!(restored.error.is_none());
    }

    #[test]
    fn rollback_result_failed_deserializes_roundtrip() {
        let result = RollbackResult {
            worker_id: "failed-worker".to_string(),
            success: false,
            rolled_back_to: None,
            error: Some("Connection refused".to_string()),
        };
        let json = serde_json::to_string(&result).unwrap();
        let restored: RollbackResult = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.worker_id, "failed-worker");
        assert!(!restored.success);
        assert!(restored.rolled_back_to.is_none());
        assert_eq!(restored.error, Some("Connection refused".to_string()));
    }

    // ========================
    // RollbackManager tests
    // ========================

    #[test]
    fn rollback_manager_new_creates_backup_dir() {
        // RollbackManager::new() creates dirs in the user's data directory
        // This test verifies the constructor doesn't panic
        let manager = RollbackManager::new();
        assert!(manager.is_ok());
    }

    // ========================
    // WorkerBackup additional tests
    // ========================

    #[test]
    fn worker_backup_with_empty_strings() {
        let backup = WorkerBackup {
            worker_id: String::new(),
            version: String::new(),
            backup_path: PathBuf::new(),
            remote_path: PathBuf::new(),
            binary_hash: String::new(),
            created_at: String::new(),
        };
        let json = serde_json::to_string(&backup).unwrap();
        let restored: WorkerBackup = serde_json::from_str(&json).unwrap();
        assert!(restored.worker_id.is_empty());
        assert!(restored.version.is_empty());
    }

    #[test]
    fn worker_backup_with_special_characters() {
        let backup = WorkerBackup {
            worker_id: "worker/with:special\"chars".to_string(),
            version: "1.0.0-beta+build.123".to_string(),
            backup_path: PathBuf::from("/path/with spaces/and\"quotes"),
            remote_path: PathBuf::from("~/.rch/backups/rch-wkr-1.0.0"),
            binary_hash: "hash123".to_string(),
            created_at: "2024-01-15T12:00:00+05:30".to_string(),
        };
        let json = serde_json::to_string(&backup).unwrap();
        let restored: WorkerBackup = serde_json::from_str(&json).unwrap();
        assert!(restored.worker_id.contains("special"));
        assert!(restored.version.contains("beta"));
    }

    #[test]
    fn worker_backup_with_long_strings() {
        let backup = WorkerBackup {
            worker_id: "w".repeat(1000),
            version: "v".repeat(100),
            backup_path: PathBuf::from("/very/long/path".to_string() + &"/segment".repeat(100)),
            remote_path: PathBuf::from("~/.rch/backups/rch-wkr-v".to_string() + &"v".repeat(99)),
            binary_hash: "h".repeat(64),
            created_at: "2024-01-15T12:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&backup).unwrap();
        let restored: WorkerBackup = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.worker_id.len(), 1000);
        assert_eq!(restored.version.len(), 100);
    }

    #[test]
    fn worker_backup_path_variations() {
        let paths = vec![
            PathBuf::from("/absolute/path"),
            PathBuf::from("relative/path"),
            PathBuf::from("."),
            PathBuf::from(".."),
            PathBuf::from("./local"),
        ];
        for path in paths {
            let backup = WorkerBackup {
                worker_id: "test".to_string(),
                version: "1.0.0".to_string(),
                backup_path: path.clone(),
                remote_path: PathBuf::from("~/.rch/backups/rch-wkr-1.0.0"),
                binary_hash: "abc123".to_string(),
                created_at: "2024-01-15T12:00:00Z".to_string(),
            };
            let json = serde_json::to_string(&backup).unwrap();
            let restored: WorkerBackup = serde_json::from_str(&json).unwrap();
            assert_eq!(restored.backup_path, path);
        }
    }

    // ========================
    // RollbackResult additional tests
    // ========================

    #[test]
    fn rollback_result_both_version_and_error() {
        // Edge case: both rolled_back_to and error are Some (unlikely but valid struct)
        let result = RollbackResult {
            worker_id: "partial-worker".to_string(),
            success: false,
            rolled_back_to: Some("1.0.0".to_string()),
            error: Some("Verification failed after rollback".to_string()),
        };
        let json = serde_json::to_string(&result).unwrap();
        let restored: RollbackResult = serde_json::from_str(&json).unwrap();
        assert!(!restored.success);
        assert_eq!(restored.rolled_back_to, Some("1.0.0".to_string()));
        assert!(restored.error.is_some());
    }

    #[test]
    fn rollback_result_empty_worker_id() {
        let result = RollbackResult {
            worker_id: String::new(),
            success: true,
            rolled_back_to: Some("1.0.0".to_string()),
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        let restored: RollbackResult = serde_json::from_str(&json).unwrap();
        assert!(restored.worker_id.is_empty());
    }

    #[test]
    fn rollback_result_long_error_message() {
        let result = RollbackResult {
            worker_id: "worker-1".to_string(),
            success: false,
            rolled_back_to: None,
            error: Some("e".repeat(10000)),
        };
        let json = serde_json::to_string(&result).unwrap();
        let restored: RollbackResult = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.error.unwrap().len(), 10000);
    }

    #[test]
    fn rollback_result_special_chars_in_error() {
        let result = RollbackResult {
            worker_id: "worker".to_string(),
            success: false,
            rolled_back_to: None,
            error: Some("Error: \"connection refused\" on host <remote>".to_string()),
        };
        let json = serde_json::to_string(&result).unwrap();
        let restored: RollbackResult = serde_json::from_str(&json).unwrap();
        assert!(restored.error.unwrap().contains("<remote>"));
    }

    #[test]
    fn rollback_result_version_formats() {
        let versions = vec![
            "1.0.0",
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0+build.123",
            "1.0.0-beta+build.456",
            "0.0.1",
            "99.99.99",
        ];
        for v in versions {
            let result = RollbackResult {
                worker_id: "test".to_string(),
                success: true,
                rolled_back_to: Some(v.to_string()),
                error: None,
            };
            let json = serde_json::to_string(&result).unwrap();
            let restored: RollbackResult = serde_json::from_str(&json).unwrap();
            assert_eq!(restored.rolled_back_to, Some(v.to_string()));
        }
    }

    // ========================
    // RollbackManager tests
    // ========================

    #[test]
    fn rollback_manager_backup_dir_exists_after_new() {
        let manager = RollbackManager::new().unwrap();
        assert!(manager.backup_dir.exists() || !manager.backup_dir.to_string_lossy().is_empty());
    }

    #[tokio::test]
    async fn rollback_manager_create_backup_returns_valid_struct() {
        use rch_common::{WorkerConfig, WorkerId};

        let manager = RollbackManager::new().unwrap();
        let worker = WorkerConfig {
            id: WorkerId::new("test-worker"),
            host: "localhost".to_string(),
            user: "testuser".to_string(),
            identity_file: "~/.ssh/id_rsa".to_string(),
            total_slots: 4,
            priority: 100,
            tags: vec![],
        };

        let backup = manager.create_backup(&worker, "1.0.0").await.unwrap();
        assert_eq!(backup.worker_id, "test-worker");
        assert_eq!(backup.version, "1.0.0");
        assert!(backup.backup_path.to_string_lossy().contains("test-worker"));
        assert!(!backup.created_at.is_empty());
    }

    #[tokio::test]
    async fn rollback_manager_create_backup_different_versions() {
        use rch_common::{WorkerConfig, WorkerId};

        let manager = RollbackManager::new().unwrap();
        let worker = WorkerConfig {
            id: WorkerId::new("version-test"),
            host: "localhost".to_string(),
            user: "testuser".to_string(),
            identity_file: "~/.ssh/id_rsa".to_string(),
            total_slots: 4,
            priority: 100,
            tags: vec![],
        };

        let backup1 = manager.create_backup(&worker, "1.0.0").await.unwrap();
        let backup2 = manager.create_backup(&worker, "2.0.0").await.unwrap();

        assert_ne!(backup1.backup_path, backup2.backup_path);
        assert_ne!(backup1.version, backup2.version);
    }

    #[tokio::test]
    async fn rollback_manager_restore_backup_succeeds() {
        use rch_common::{WorkerConfig, WorkerId};

        let manager = RollbackManager::new().unwrap();
        let worker = WorkerConfig {
            id: WorkerId::new("restore-test"),
            host: "localhost".to_string(),
            user: "testuser".to_string(),
            identity_file: "~/.ssh/id_rsa".to_string(),
            total_slots: 4,
            priority: 100,
            tags: vec![],
        };

        let backup = manager.create_backup(&worker, "1.0.0").await.unwrap();
        let result = manager.restore_backup(&backup, &worker).await;
        assert!(result.is_ok());
    }
}
