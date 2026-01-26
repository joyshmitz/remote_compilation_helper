//! SQLite schema for telemetry persistence.

/// Current schema version for migrations.
#[cfg(test)]
pub const SCHEMA_VERSION: u32 = 1;

pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS telemetry_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    worker_id TEXT NOT NULL,
    timestamp INTEGER NOT NULL,

    cpu_percent REAL NOT NULL,
    cpu_user_percent REAL,
    cpu_system_percent REAL,
    cpu_iowait_percent REAL,
    load_avg_1m REAL,
    load_avg_5m REAL,
    load_avg_15m REAL,

    memory_total_mb INTEGER,
    memory_available_mb INTEGER,
    memory_used_percent REAL,
    swap_used_percent REAL,

    disk_read_mbps REAL,
    disk_write_mbps REAL,
    disk_iops REAL,
    disk_utilization_percent REAL,

    network_rx_mbps REAL,
    network_tx_mbps REAL,
    network_error_rate REAL
);

CREATE INDEX IF NOT EXISTS idx_telemetry_worker_time
    ON telemetry_snapshots(worker_id, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_telemetry_time
    ON telemetry_snapshots(timestamp);

CREATE TABLE IF NOT EXISTS telemetry_hourly (
    worker_id TEXT NOT NULL,
    hour_timestamp INTEGER NOT NULL,
    sample_count INTEGER NOT NULL,

    cpu_percent_avg REAL,
    cpu_percent_max REAL,
    memory_used_percent_avg REAL,
    disk_utilization_avg REAL,

    PRIMARY KEY (worker_id, hour_timestamp)
);

CREATE TABLE IF NOT EXISTS speedscore_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    worker_id TEXT NOT NULL,
    measured_at INTEGER NOT NULL,

    total_score REAL NOT NULL,
    cpu_score REAL NOT NULL,
    memory_score REAL NOT NULL,
    disk_score REAL NOT NULL,
    network_score REAL NOT NULL,
    compilation_score REAL NOT NULL,

    raw_results TEXT,
    algorithm_version INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS idx_speedscore_worker_time
    ON speedscore_history(worker_id, measured_at DESC);

CREATE TABLE IF NOT EXISTS speedscore_latest (
    worker_id TEXT PRIMARY KEY,
    speedscore_id INTEGER NOT NULL REFERENCES speedscore_history(id),
    total_score REAL NOT NULL,
    measured_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS test_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL,
    worker_id TEXT NOT NULL,
    command TEXT NOT NULL,
    kind TEXT NOT NULL,
    exit_code INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    completed_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_test_runs_time
    ON test_runs(completed_at DESC);

CREATE INDEX IF NOT EXISTS idx_test_runs_kind
    ON test_runs(kind);
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{Connection, params};

    // ========================
    // Schema version tests
    // ========================

    #[test]
    fn schema_version_is_one() {
        assert_eq!(SCHEMA_VERSION, 1);
    }

    // ========================
    // Migration application tests
    // ========================

    #[test]
    fn schema_applies_to_fresh_database() {
        let conn = Connection::open_in_memory().unwrap();
        let result = conn.execute_batch(SCHEMA);
        assert!(result.is_ok(), "Schema should apply cleanly: {:?}", result);
    }

    #[test]
    fn schema_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();

        // Apply schema twice - should succeed both times (CREATE IF NOT EXISTS)
        conn.execute_batch(SCHEMA).unwrap();
        let result = conn.execute_batch(SCHEMA);
        assert!(result.is_ok(), "Schema should be idempotent: {:?}", result);
    }

    #[test]
    fn schema_creates_all_tables() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        let expected_tables = [
            "telemetry_snapshots",
            "telemetry_hourly",
            "speedscore_history",
            "speedscore_latest",
            "test_runs",
        ];

        for table in expected_tables {
            let exists: i32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    params![table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "Table '{}' should exist", table);
        }
    }

    // ========================
    // Index creation verification tests
    // ========================

    #[test]
    fn schema_creates_telemetry_worker_time_index() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        let exists: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_telemetry_worker_time'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1, "idx_telemetry_worker_time should exist");
    }

    #[test]
    fn schema_creates_telemetry_time_index() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        let exists: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_telemetry_time'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1, "idx_telemetry_time should exist");
    }

    #[test]
    fn schema_creates_speedscore_worker_time_index() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        let exists: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_speedscore_worker_time'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1, "idx_speedscore_worker_time should exist");
    }

    #[test]
    fn schema_creates_test_runs_indexes() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        let indexes = ["idx_test_runs_time", "idx_test_runs_kind"];
        for idx in indexes {
            let exists: i32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
                    params![idx],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "Index '{}' should exist", idx);
        }
    }

    #[test]
    fn schema_creates_all_indexes() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        let expected_indexes = [
            "idx_telemetry_worker_time",
            "idx_telemetry_time",
            "idx_speedscore_worker_time",
            "idx_test_runs_time",
            "idx_test_runs_kind",
        ];

        for idx in expected_indexes {
            let exists: i32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?1",
                    params![idx],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "Index '{}' should exist", idx);
        }
    }

    // ========================
    // Constraint enforcement tests
    // ========================

    #[test]
    fn telemetry_snapshots_enforces_not_null_worker_id() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        let result = conn.execute(
            "INSERT INTO telemetry_snapshots (worker_id, timestamp, cpu_percent) VALUES (NULL, 123, 50.0)",
            [],
        );
        assert!(result.is_err(), "Should reject NULL worker_id");
    }

    #[test]
    fn telemetry_snapshots_enforces_not_null_timestamp() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        let result = conn.execute(
            "INSERT INTO telemetry_snapshots (worker_id, timestamp, cpu_percent) VALUES ('w1', NULL, 50.0)",
            [],
        );
        assert!(result.is_err(), "Should reject NULL timestamp");
    }

    #[test]
    fn telemetry_snapshots_enforces_not_null_cpu_percent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        let result = conn.execute(
            "INSERT INTO telemetry_snapshots (worker_id, timestamp, cpu_percent) VALUES ('w1', 123, NULL)",
            [],
        );
        assert!(result.is_err(), "Should reject NULL cpu_percent");
    }

    #[test]
    fn telemetry_snapshots_allows_null_optional_fields() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        // Insert with only required fields, optional fields are NULL
        let result = conn.execute(
            "INSERT INTO telemetry_snapshots (worker_id, timestamp, cpu_percent) VALUES ('w1', 123, 50.0)",
            [],
        );
        assert!(result.is_ok(), "Should allow NULL optional fields");
    }

    #[test]
    fn telemetry_hourly_enforces_composite_primary_key() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        // Insert first record
        conn.execute(
            "INSERT INTO telemetry_hourly (worker_id, hour_timestamp, sample_count) VALUES ('w1', 1000, 5)",
            [],
        )
        .unwrap();

        // Duplicate should fail (same primary key)
        let result = conn.execute(
            "INSERT INTO telemetry_hourly (worker_id, hour_timestamp, sample_count) VALUES ('w1', 1000, 10)",
            [],
        );
        assert!(
            result.is_err(),
            "Should reject duplicate composite primary key"
        );

        // Different hour_timestamp should succeed
        let result = conn.execute(
            "INSERT INTO telemetry_hourly (worker_id, hour_timestamp, sample_count) VALUES ('w1', 2000, 10)",
            [],
        );
        assert!(result.is_ok(), "Should allow different hour_timestamp");

        // Different worker_id should succeed
        let result = conn.execute(
            "INSERT INTO telemetry_hourly (worker_id, hour_timestamp, sample_count) VALUES ('w2', 1000, 10)",
            [],
        );
        assert!(result.is_ok(), "Should allow different worker_id");
    }

    #[test]
    fn speedscore_latest_enforces_primary_key() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        // First need a speedscore_history entry to reference
        conn.execute(
            "INSERT INTO speedscore_history (worker_id, measured_at, total_score, cpu_score, memory_score, disk_score, network_score, compilation_score) VALUES ('w1', 123, 80.0, 85.0, 75.0, 70.0, 80.0, 90.0)",
            [],
        )
        .unwrap();

        // Insert into speedscore_latest
        conn.execute(
            "INSERT INTO speedscore_latest (worker_id, speedscore_id, total_score, measured_at) VALUES ('w1', 1, 80.0, 123)",
            [],
        )
        .unwrap();

        // Duplicate worker_id should fail
        let result = conn.execute(
            "INSERT INTO speedscore_latest (worker_id, speedscore_id, total_score, measured_at) VALUES ('w1', 1, 85.0, 456)",
            [],
        );
        assert!(result.is_err(), "Should reject duplicate worker_id");
    }

    #[test]
    fn speedscore_history_enforces_not_null_scores() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        let required_fields = [
            (
                "total_score",
                "worker_id, measured_at, cpu_score, memory_score, disk_score, network_score, compilation_score",
            ),
            (
                "cpu_score",
                "worker_id, measured_at, total_score, memory_score, disk_score, network_score, compilation_score",
            ),
            (
                "memory_score",
                "worker_id, measured_at, total_score, cpu_score, disk_score, network_score, compilation_score",
            ),
            (
                "disk_score",
                "worker_id, measured_at, total_score, cpu_score, memory_score, network_score, compilation_score",
            ),
            (
                "network_score",
                "worker_id, measured_at, total_score, cpu_score, memory_score, disk_score, compilation_score",
            ),
            (
                "compilation_score",
                "worker_id, measured_at, total_score, cpu_score, memory_score, disk_score, network_score",
            ),
        ];

        for (null_field, fields) in required_fields {
            let sql = format!(
                "INSERT INTO speedscore_history ({}) VALUES ('w1', 123, 80.0, 85.0, 75.0, 70.0, 80.0)",
                fields
            );
            let result = conn.execute(&sql, []);
            assert!(
                result.is_err(),
                "Should reject NULL {} in speedscore_history",
                null_field
            );
        }
    }

    #[test]
    fn test_runs_enforces_not_null_fields() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        // All required fields present
        let result = conn.execute(
            "INSERT INTO test_runs (project_id, worker_id, command, kind, exit_code, duration_ms, completed_at) VALUES ('p1', 'w1', 'cargo test', 'cargo_test', 0, 1000, 123)",
            [],
        );
        assert!(result.is_ok(), "Should accept all required fields");

        // NULL project_id
        let result = conn.execute(
            "INSERT INTO test_runs (project_id, worker_id, command, kind, exit_code, duration_ms, completed_at) VALUES (NULL, 'w1', 'cargo test', 'cargo_test', 0, 1000, 123)",
            [],
        );
        assert!(result.is_err(), "Should reject NULL project_id");

        // NULL worker_id
        let result = conn.execute(
            "INSERT INTO test_runs (project_id, worker_id, command, kind, exit_code, duration_ms, completed_at) VALUES ('p1', NULL, 'cargo test', 'cargo_test', 0, 1000, 123)",
            [],
        );
        assert!(result.is_err(), "Should reject NULL worker_id");
    }

    #[test]
    fn telemetry_snapshots_autoincrement_id() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        // Insert two rows
        conn.execute(
            "INSERT INTO telemetry_snapshots (worker_id, timestamp, cpu_percent) VALUES ('w1', 100, 50.0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO telemetry_snapshots (worker_id, timestamp, cpu_percent) VALUES ('w2', 200, 60.0)",
            [],
        )
        .unwrap();

        // Check IDs are auto-incrementing
        let ids: Vec<i64> = conn
            .prepare("SELECT id FROM telemetry_snapshots ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(ids, vec![1, 2], "IDs should auto-increment");
    }

    #[test]
    fn speedscore_history_default_algorithm_version() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        // Insert without specifying algorithm_version
        conn.execute(
            "INSERT INTO speedscore_history (worker_id, measured_at, total_score, cpu_score, memory_score, disk_score, network_score, compilation_score) VALUES ('w1', 123, 80.0, 85.0, 75.0, 70.0, 80.0, 90.0)",
            [],
        )
        .unwrap();

        let version: i32 = conn
            .query_row(
                "SELECT algorithm_version FROM speedscore_history WHERE worker_id = 'w1'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(version, 1, "Default algorithm_version should be 1");
    }

    // ========================
    // Index effectiveness tests
    // ========================

    #[test]
    fn telemetry_worker_time_index_covers_common_query() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        // Insert some test data
        for i in 0..10 {
            conn.execute(
                "INSERT INTO telemetry_snapshots (worker_id, timestamp, cpu_percent) VALUES (?1, ?2, ?3)",
                params![format!("worker-{}", i % 3), i * 100, 50.0 + i as f64],
            )
            .unwrap();
        }

        // Query that should use the index
        let result: Vec<(String, i64)> = conn
            .prepare("SELECT worker_id, timestamp FROM telemetry_snapshots WHERE worker_id = 'worker-1' ORDER BY timestamp DESC LIMIT 5")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        // Should return worker-1 entries in descending timestamp order
        assert!(!result.is_empty());
        for (wid, _) in &result {
            assert_eq!(wid, "worker-1");
        }
    }

    #[test]
    fn speedscore_worker_time_index_covers_history_query() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();

        // Insert some speedscore history
        for i in 0..5 {
            conn.execute(
                "INSERT INTO speedscore_history (worker_id, measured_at, total_score, cpu_score, memory_score, disk_score, network_score, compilation_score) VALUES (?1, ?2, ?3, 85.0, 75.0, 70.0, 80.0, 90.0)",
                params!["worker-1", i * 1000, 80.0 + i as f64],
            )
            .unwrap();
        }

        // Query that should use the index
        let scores: Vec<f64> = conn
            .prepare("SELECT total_score FROM speedscore_history WHERE worker_id = 'worker-1' ORDER BY measured_at DESC")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(scores.len(), 5);
        // Should be in descending order
        assert!(scores[0] > scores[4]);
    }
}
