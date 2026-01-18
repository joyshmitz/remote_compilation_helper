//! SQLite schema for telemetry persistence.

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
"#;
