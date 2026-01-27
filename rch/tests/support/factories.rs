//! Test data factories for UI integration tests.
//!
//! Provides mock data structures for testing UI components.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Mock status data for testing StatusTable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockStatusData {
    pub connected: bool,
    pub daemon_addr: String,
    pub workers_online: u32,
    pub workers_total: u32,
    pub queue_depth: u32,
    pub active_jobs: Vec<MockActiveJob>,
    pub cache_hit_rate: f64,
}

/// Mock active job for testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockActiveJob {
    pub id: String,
    pub command: String,
    pub worker: String,
    pub duration_secs: f64,
}

/// Mock worker information for testing WorkerTable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockWorkerInfo {
    pub name: String,
    pub host: String,
    pub status: MockWorkerStatus,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub load: f64,
    pub jobs_completed: u64,
    pub used_slots: u32,
    pub total_slots: u32,
    pub speed_score: f64,
    pub circuit_state: String,
}

/// Mock worker status enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MockWorkerStatus {
    Online,
    Busy,
    Offline,
    Degraded,
    Draining,
}

impl std::fmt::Display for MockWorkerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Online => write!(f, "Online"),
            Self::Busy => write!(f, "Busy"),
            Self::Offline => write!(f, "Offline"),
            Self::Degraded => write!(f, "Degraded"),
            Self::Draining => write!(f, "Draining"),
        }
    }
}

/// Mock error scenario for testing ErrorPanel.
#[derive(Debug, Clone)]
pub struct MockErrorScenario {
    pub code: String,
    pub message: String,
    pub suggestions: Vec<String>,
}

// ============================================================================
// FACTORY FUNCTIONS
// ============================================================================

/// Create mock status data for testing.
pub fn mock_status_data() -> MockStatusData {
    MockStatusData {
        connected: true,
        daemon_addr: "127.0.0.1:9274".to_string(),
        workers_online: 3,
        workers_total: 3,
        queue_depth: 2,
        active_jobs: vec![MockActiveJob {
            id: "j-a3f2".to_string(),
            command: "cargo build --release".to_string(),
            worker: "worker1".to_string(),
            duration_secs: 12.3,
        }],
        cache_hit_rate: 0.81,
    }
}

/// Create mock status data with no active jobs.
pub fn mock_status_idle() -> MockStatusData {
    MockStatusData {
        connected: true,
        daemon_addr: "127.0.0.1:9274".to_string(),
        workers_online: 3,
        workers_total: 3,
        queue_depth: 0,
        active_jobs: vec![],
        cache_hit_rate: 0.75,
    }
}

/// Create mock status data with multiple active jobs.
pub fn mock_status_busy() -> MockStatusData {
    MockStatusData {
        connected: true,
        daemon_addr: "127.0.0.1:9274".to_string(),
        workers_online: 4,
        workers_total: 4,
        queue_depth: 5,
        active_jobs: vec![
            MockActiveJob {
                id: "j-a1b2".to_string(),
                command: "cargo build --release".to_string(),
                worker: "worker1".to_string(),
                duration_secs: 45.2,
            },
            MockActiveJob {
                id: "j-c3d4".to_string(),
                command: "cargo test".to_string(),
                worker: "worker2".to_string(),
                duration_secs: 12.8,
            },
            MockActiveJob {
                id: "j-e5f6".to_string(),
                command: "cargo clippy".to_string(),
                worker: "worker3".to_string(),
                duration_secs: 5.1,
            },
        ],
        cache_hit_rate: 0.92,
    }
}

/// Create mock worker data for testing.
pub fn mock_worker_data() -> Vec<MockWorkerInfo> {
    vec![
        MockWorkerInfo {
            name: "worker1".to_string(),
            host: "192.168.1.10".to_string(),
            status: MockWorkerStatus::Online,
            cpu_usage: 0.65,
            memory_usage: 0.45,
            load: 1.2,
            jobs_completed: 847,
            used_slots: 2,
            total_slots: 8,
            speed_score: 1.25,
            circuit_state: "closed".to_string(),
        },
        MockWorkerInfo {
            name: "worker2".to_string(),
            host: "192.168.1.11".to_string(),
            status: MockWorkerStatus::Busy,
            cpu_usage: 0.95,
            memory_usage: 0.78,
            load: 3.8,
            jobs_completed: 652,
            used_slots: 8,
            total_slots: 8,
            speed_score: 1.15,
            circuit_state: "closed".to_string(),
        },
        MockWorkerInfo {
            name: "worker3".to_string(),
            host: "192.168.1.12".to_string(),
            status: MockWorkerStatus::Offline,
            cpu_usage: 0.0,
            memory_usage: 0.0,
            load: 0.0,
            jobs_completed: 423,
            used_slots: 0,
            total_slots: 4,
            speed_score: 0.0,
            circuit_state: "open".to_string(),
        },
    ]
}

/// Create mock worker data with all workers healthy.
pub fn mock_workers_all_healthy() -> Vec<MockWorkerInfo> {
    vec![
        MockWorkerInfo {
            name: "fast-build".to_string(),
            host: "10.0.0.1".to_string(),
            status: MockWorkerStatus::Online,
            cpu_usage: 0.30,
            memory_usage: 0.40,
            load: 0.8,
            jobs_completed: 1200,
            used_slots: 2,
            total_slots: 16,
            speed_score: 1.85,
            circuit_state: "closed".to_string(),
        },
        MockWorkerInfo {
            name: "test-runner".to_string(),
            host: "10.0.0.2".to_string(),
            status: MockWorkerStatus::Online,
            cpu_usage: 0.45,
            memory_usage: 0.55,
            load: 1.5,
            jobs_completed: 890,
            used_slots: 4,
            total_slots: 8,
            speed_score: 1.20,
            circuit_state: "closed".to_string(),
        },
    ]
}

/// Create mock error scenarios for testing ErrorPanel.
pub fn mock_error_scenarios() -> Vec<MockErrorScenario> {
    vec![
        MockErrorScenario {
            code: "RCH-E042".to_string(),
            message: "Worker connection failed".to_string(),
            suggestions: vec![
                "Check SSH connectivity".to_string(),
                "Verify worker is online".to_string(),
            ],
        },
        MockErrorScenario {
            code: "RCH-E001".to_string(),
            message: "Configuration parse error".to_string(),
            suggestions: vec![
                "Edit config file".to_string(),
                "Run: rch config check".to_string(),
            ],
        },
        MockErrorScenario {
            code: "RCH-E100".to_string(),
            message: "SSH authentication failed".to_string(),
            suggestions: vec![
                "Check SSH key: ssh-add -l".to_string(),
                "Verify key permissions: chmod 600 ~/.ssh/id_ed25519".to_string(),
                "Test connection: ssh worker1".to_string(),
            ],
        },
        MockErrorScenario {
            code: "RCH-E502".to_string(),
            message: "Daemon not running".to_string(),
            suggestions: vec![
                "Start daemon: rch daemon start".to_string(),
                "Check status: rch status".to_string(),
            ],
        },
    ]
}

/// Create a fixed timestamp for deterministic testing.
pub fn fixed_timestamp() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2024-01-15T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn test_mock_status_data() {
        let data = mock_status_data();
        assert!(data.connected);
        assert_eq!(data.workers_online, 3);
        assert_eq!(data.active_jobs.len(), 1);
    }

    #[test]
    fn test_mock_status_idle() {
        let data = mock_status_idle();
        assert!(data.active_jobs.is_empty());
        assert_eq!(data.queue_depth, 0);
    }

    #[test]
    fn test_mock_status_busy() {
        let data = mock_status_busy();
        assert_eq!(data.active_jobs.len(), 3);
        assert!(data.queue_depth > 0);
    }

    #[test]
    fn test_mock_worker_data() {
        let workers = mock_worker_data();
        assert_eq!(workers.len(), 3);

        // Check statuses are varied
        assert_eq!(workers[0].status, MockWorkerStatus::Online);
        assert_eq!(workers[1].status, MockWorkerStatus::Busy);
        assert_eq!(workers[2].status, MockWorkerStatus::Offline);
    }

    #[test]
    fn test_mock_workers_all_healthy() {
        let workers = mock_workers_all_healthy();
        assert!(workers.iter().all(|w| w.status == MockWorkerStatus::Online));
    }

    #[test]
    fn test_mock_error_scenarios() {
        let scenarios = mock_error_scenarios();
        assert!(!scenarios.is_empty());

        // Each scenario should have code, message, and at least one suggestion
        for scenario in &scenarios {
            assert!(!scenario.code.is_empty());
            assert!(!scenario.message.is_empty());
            assert!(!scenario.suggestions.is_empty());
        }
    }

    #[test]
    fn test_fixed_timestamp() {
        let ts = fixed_timestamp();
        // Should be deterministic
        assert_eq!(ts, fixed_timestamp());
        assert_eq!(ts.year(), 2024);
        assert_eq!(ts.month(), 1);
        assert_eq!(ts.day(), 15);
    }

    #[test]
    fn test_worker_status_display() {
        assert_eq!(MockWorkerStatus::Online.to_string(), "Online");
        assert_eq!(MockWorkerStatus::Offline.to_string(), "Offline");
        assert_eq!(MockWorkerStatus::Busy.to_string(), "Busy");
    }
}
