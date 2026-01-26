//! Worker cache cleanup scheduler.
//!
//! Periodically cleans up old project caches on workers to prevent disk exhaustion.
//! Respects worker busy state and configurable thresholds.

#![allow(dead_code)] // Scaffold code - will be wired into main.rs

use crate::config::CacheCleanupConfig;
use crate::workers::{WorkerPool, WorkerState};
use rch_common::{SshClient, SshOptions, WorkerId, WorkerStatus};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, info, warn};

/// Result of a cleanup operation on a single worker.
#[derive(Debug, Clone)]
pub struct CleanupResult {
    /// Worker that was cleaned.
    pub worker_id: WorkerId,
    /// Whether cleanup was successful.
    pub success: bool,
    /// Number of directories removed (if known).
    pub dirs_removed: Option<u64>,
    /// Bytes freed (if known).
    pub bytes_freed: Option<u64>,
    /// Duration of cleanup operation.
    pub duration: Duration,
    /// Error message if cleanup failed.
    pub error: Option<String>,
}

/// Stats for a cleanup run across all workers.
#[derive(Debug, Default)]
pub struct CleanupStats {
    /// Number of workers checked.
    pub workers_checked: u32,
    /// Number of workers cleaned.
    pub workers_cleaned: u32,
    /// Number of workers skipped (busy or unhealthy).
    pub workers_skipped: u32,
    /// Number of cleanup errors.
    pub errors: u32,
    /// Total bytes freed across all workers.
    pub total_bytes_freed: u64,
    /// Total directories removed.
    pub total_dirs_removed: u64,
}

/// Cache cleanup scheduler service.
pub struct CacheCleanupScheduler {
    /// Worker pool reference.
    pool: WorkerPool,
    /// Configuration.
    config: CacheCleanupConfig,
    /// Last cleanup time per worker.
    last_cleanup: Arc<RwLock<HashMap<WorkerId, Instant>>>,
    /// SSH options for cleanup commands.
    ssh_options: SshOptions,
}

impl CacheCleanupScheduler {
    /// Create a new cache cleanup scheduler.
    pub fn new(pool: WorkerPool, config: CacheCleanupConfig) -> Self {
        Self {
            pool,
            config,
            last_cleanup: Arc::new(RwLock::new(HashMap::new())),
            ssh_options: SshOptions::default(),
        }
    }

    /// Start the cleanup scheduler.
    ///
    /// Returns a handle to the spawned task.
    pub fn start(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let scheduler = self;
        tokio::spawn(async move {
            if !scheduler.config.enabled {
                info!("Cache cleanup scheduler disabled");
                return;
            }

            info!(
                "Cache cleanup scheduler started (interval={}s, max_age={}h, min_free={}GB)",
                scheduler.config.interval_secs,
                scheduler.config.max_cache_age_hours,
                scheduler.config.min_free_gb
            );

            let mut ticker = interval(Duration::from_secs(scheduler.config.interval_secs));
            loop {
                ticker.tick().await;
                let stats = scheduler.run_cleanup_cycle().await;
                if stats.workers_cleaned > 0 || stats.errors > 0 {
                    info!(
                        "Cache cleanup cycle: checked={}, cleaned={}, skipped={}, errors={}, freed={}MB",
                        stats.workers_checked,
                        stats.workers_cleaned,
                        stats.workers_skipped,
                        stats.errors,
                        stats.total_bytes_freed / (1024 * 1024)
                    );
                } else {
                    debug!("Cache cleanup cycle: no action needed");
                }
            }
        })
    }

    /// Run a single cleanup cycle across all workers.
    async fn run_cleanup_cycle(&self) -> CleanupStats {
        let mut stats = CleanupStats::default();
        let workers = self.pool.all_workers().await;

        for worker_state in workers {
            stats.workers_checked += 1;
            let worker_id = worker_state.config.read().await.id.clone();

            // Check if worker is eligible for cleanup
            if !self.is_worker_eligible(&worker_state).await {
                debug!("Worker {} not eligible for cleanup", worker_id);
                stats.workers_skipped += 1;
                continue;
            }

            // Perform cleanup
            match self.cleanup_worker(&worker_state).await {
                Ok(result) => {
                    if result.success {
                        stats.workers_cleaned += 1;
                        if let Some(bytes) = result.bytes_freed {
                            stats.total_bytes_freed += bytes;
                        }
                        if let Some(dirs) = result.dirs_removed {
                            stats.total_dirs_removed += dirs;
                        }
                    } else {
                        stats.errors += 1;
                    }
                }
                Err(e) => {
                    warn!("Failed to cleanup worker {}: {}", worker_id, e);
                    stats.errors += 1;
                }
            }
        }

        stats
    }

    /// Check if a worker is eligible for cleanup.
    async fn is_worker_eligible(&self, worker_state: &WorkerState) -> bool {
        // Check worker status
        let status = worker_state.status().await;
        if status != WorkerStatus::Healthy {
            debug!(
                "Worker {:?} not healthy (status={:?}), skipping cleanup",
                worker_state.config.read().await.id,
                status
            );
            return false;
        }

        // Check if worker is busy (has active slots)
        let available = worker_state.available_slots().await;
        let config = worker_state.config.read().await;
        if available < config.total_slots {
            debug!(
                "Worker {} is busy ({}/{} slots available), skipping cleanup",
                config.id, available, config.total_slots
            );
            return false;
        }

        // Check circuit breaker state
        let circuit_state = worker_state.circuit_state().await;
        if circuit_state != Some(rch_common::CircuitState::Closed) {
            debug!(
                "Worker {} circuit not closed ({:?}), skipping cleanup",
                config.id, circuit_state
            );
            return false;
        }

        true
    }

    /// Perform cleanup on a single worker.
    async fn cleanup_worker(&self, worker_state: &WorkerState) -> anyhow::Result<CleanupResult> {
        let start = Instant::now();
        let config = worker_state.config.read().await.clone();

        info!(
            "Starting cache cleanup on worker {} (max_age={}h)",
            config.id, self.config.max_cache_age_hours
        );

        // Build cleanup command
        // Uses find to delete directories older than max_age_hours
        let remote_base = &self.config.remote_base;
        let max_age_minutes = self.config.max_cache_age_hours * 60;
        let cleanup_cmd = format!(
            "find {} -mindepth 2 -maxdepth 2 -type d -mmin +{} -exec rm -rf {{}} \\; 2>/dev/null; \
             du -sh {} 2>/dev/null || echo '0 {}'",
            remote_base, max_age_minutes, remote_base, remote_base
        );

        // Execute cleanup via SSH
        let mut ssh_client = SshClient::new(config.clone(), self.ssh_options.clone());
        ssh_client.connect().await?;

        let result = ssh_client.execute(&cleanup_cmd).await?;

        // Update last cleanup time
        {
            let mut last_cleanup = self.last_cleanup.write().await;
            last_cleanup.insert(config.id.clone(), Instant::now());
        }

        let duration = start.elapsed();

        if result.exit_code == 0 {
            info!(
                "Cache cleanup completed on worker {} in {:?}",
                config.id, duration
            );
            Ok(CleanupResult {
                worker_id: config.id,
                success: true,
                dirs_removed: None, // Could parse from output
                bytes_freed: None,  // Could parse from output
                duration,
                error: None,
            })
        } else {
            let error_msg = format!("Cleanup command failed with exit code {}", result.exit_code);
            warn!(
                "Cache cleanup failed on worker {}: {}",
                config.id, error_msg
            );
            Ok(CleanupResult {
                worker_id: config.id,
                success: false,
                dirs_removed: None,
                bytes_freed: None,
                duration,
                error: Some(error_msg),
            })
        }
    }

    /// Trigger immediate cleanup on a specific worker.
    pub async fn cleanup_worker_now(&self, worker_id: &WorkerId) -> anyhow::Result<CleanupResult> {
        let worker_state = self
            .pool
            .get(worker_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("Worker {} not found", worker_id))?;

        self.cleanup_worker(&worker_state).await
    }

    /// Get cleanup status for all workers.
    pub async fn get_status(&self) -> HashMap<WorkerId, Option<Instant>> {
        let last_cleanup = self.last_cleanup.read().await;
        let workers = self.pool.all_workers().await;

        let mut status = HashMap::new();
        for worker in workers {
            let id = worker.config.read().await.id.clone();
            let last = last_cleanup.get(&id).copied();
            status.insert(id, last);
        }
        status
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_config_defaults() {
        let config = CacheCleanupConfig::default();
        assert!(config.enabled);
        assert_eq!(config.interval_secs, 3600);
        assert_eq!(config.max_cache_age_hours, 72);
        assert_eq!(config.min_free_gb, 10);
        assert_eq!(config.idle_threshold_secs, 60);
        assert_eq!(config.remote_base, "/tmp/rch");
    }

    #[test]
    fn test_cleanup_stats_default() {
        let stats = CleanupStats::default();
        assert_eq!(stats.workers_checked, 0);
        assert_eq!(stats.workers_cleaned, 0);
        assert_eq!(stats.workers_skipped, 0);
        assert_eq!(stats.errors, 0);
        assert_eq!(stats.total_bytes_freed, 0);
    }

    #[test]
    fn test_cleanup_result_creation() {
        let result = CleanupResult {
            worker_id: WorkerId::new("test-worker"),
            success: true,
            dirs_removed: Some(5),
            bytes_freed: Some(1024 * 1024 * 100),
            duration: Duration::from_secs(2),
            error: None,
        };

        assert!(result.success);
        assert_eq!(result.dirs_removed, Some(5));
        assert_eq!(result.bytes_freed, Some(104857600));
    }
}
