//! Common types used across RCH components.

use serde::{Deserialize, Serialize};

/// Unique identifier for a worker in the fleet.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkerId(pub String);

impl WorkerId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for WorkerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Status of a worker in the fleet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    /// Worker is healthy and accepting jobs.
    Healthy,
    /// Worker is responding slowly.
    Degraded,
    /// Worker failed to respond to heartbeat.
    Unreachable,
    /// Worker is not accepting new jobs (finishing current).
    Draining,
    /// Worker is manually disabled.
    Disabled,
}

impl Default for WorkerStatus {
    fn default() -> Self {
        Self::Healthy
    }
}

/// Worker selection request sent from hook to daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionRequest {
    /// Project identifier (usually directory name or hash).
    pub project: String,
    /// Estimated CPU cores needed for this compilation.
    pub estimated_cores: u32,
    /// Preferred worker IDs (e.g., from project config).
    #[serde(default)]
    pub preferred_workers: Vec<WorkerId>,
}

/// Reason for worker selection result.
///
/// Provides context when no worker is available, enabling informative
/// fallback messages in the hook.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionReason {
    /// Worker assigned successfully.
    Success,
    /// No workers configured in workers.toml.
    NoWorkersConfigured,
    /// All workers are unreachable (failed health checks).
    AllWorkersUnreachable,
    /// All workers have circuits open (after repeated failures).
    AllCircuitsOpen,
    /// All workers are at capacity (no available slots).
    AllWorkersBusy,
    /// No workers match required tags or preferences.
    NoMatchingWorkers,
    /// Internal error during selection.
    SelectionError(String),
}

impl std::fmt::Display for SelectionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => write!(f, "worker assigned successfully"),
            Self::NoWorkersConfigured => write!(f, "no workers configured"),
            Self::AllWorkersUnreachable => write!(f, "all workers unreachable"),
            Self::AllCircuitsOpen => write!(f, "all worker circuits open"),
            Self::AllWorkersBusy => write!(f, "all workers at capacity"),
            Self::NoMatchingWorkers => write!(f, "no matching workers found"),
            Self::SelectionError(e) => write!(f, "selection error: {}", e),
        }
    }
}

/// Details about a selected worker for remote compilation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectedWorker {
    /// Selected worker ID.
    pub id: WorkerId,
    /// Host address for SSH.
    pub host: String,
    /// SSH user.
    pub user: String,
    /// Path to SSH identity file.
    pub identity_file: String,
    /// Number of slots available on this worker after reservation.
    pub slots_available: u32,
    /// Worker's speed score (0-100).
    pub speed_score: f64,
}

/// Worker selection response from daemon to hook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionResponse {
    /// Selected worker details, if available.
    pub worker: Option<SelectedWorker>,
    /// Reason for the selection result.
    pub reason: SelectionReason,
}

/// Configuration for a remote worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    /// Unique identifier for this worker.
    pub id: WorkerId,
    /// SSH hostname or IP address.
    pub host: String,
    /// SSH username.
    pub user: String,
    /// Path to SSH private key.
    pub identity_file: String,
    /// Total CPU slots available on this worker.
    pub total_slots: u32,
    /// Priority for worker selection (higher = preferred).
    #[serde(default = "default_priority")]
    pub priority: u32,
    /// Optional tags for filtering.
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_priority() -> u32 {
    100
}

/// RCH configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RchConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub compilation: CompilationConfig,
    #[serde(default)]
    pub transfer: TransferConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Whether RCH is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Log level (trace, debug, info, warn, error).
    #[serde(default = "default_log_level")]
    pub log_level: String,
    /// Path to Unix socket for daemon communication.
    #[serde(default = "default_socket_path")]
    pub socket_path: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_level: "info".to_string(),
            socket_path: "/tmp/rch.sock".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilationConfig {
    /// Minimum confidence score to intercept (0.0-1.0).
    #[serde(default = "default_confidence")]
    pub confidence_threshold: f64,
    /// Skip interception if estimated local time < this (ms).
    #[serde(default = "default_min_local_time")]
    pub min_local_time_ms: u64,
}

impl Default for CompilationConfig {
    fn default() -> Self {
        Self {
            confidence_threshold: 0.85,
            min_local_time_ms: 2000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferConfig {
    /// zstd compression level (1-19).
    #[serde(default = "default_compression")]
    pub compression_level: u32,
    /// Patterns to exclude from transfer.
    #[serde(default = "default_excludes")]
    pub exclude_patterns: Vec<String>,
}

impl Default for TransferConfig {
    fn default() -> Self {
        Self {
            compression_level: 3,
            exclude_patterns: default_excludes(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_socket_path() -> String {
    "/tmp/rch.sock".to_string()
}

fn default_confidence() -> f64 {
    0.85
}

fn default_min_local_time() -> u64 {
    2000
}

fn default_compression() -> u32 {
    3
}

fn default_excludes() -> Vec<String> {
    vec![
        "target/".to_string(),
        ".git/objects/".to_string(),
        "node_modules/".to_string(),
        "*.rlib".to_string(),
        "*.rmeta".to_string(),
    ]
}
