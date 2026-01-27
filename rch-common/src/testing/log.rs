//! Structured test logging for CI debugging.
//!
//! Provides JSONL output for test execution to make debugging failed tests easier.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

/// Test execution phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestPhase {
    /// Test initialization.
    Setup,
    /// Main test execution.
    Execute,
    /// Result verification.
    Verify,
    /// Resource cleanup.
    Teardown,
}

impl std::fmt::Display for TestPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Setup => write!(f, "setup"),
            Self::Execute => write!(f, "execute"),
            Self::Verify => write!(f, "verify"),
            Self::Teardown => write!(f, "teardown"),
        }
    }
}

/// A structured log entry for test execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestLogEntry {
    /// ISO-8601 timestamp.
    pub timestamp: String,
    /// Name of the test.
    pub test_name: String,
    /// Current phase of test execution.
    pub phase: TestPhase,
    /// Log message.
    pub message: String,
    /// Optional structured data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// Duration since test start in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

impl TestLogEntry {
    /// Create a new log entry.
    pub fn new(test_name: &str, phase: TestPhase, message: impl Into<String>) -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            test_name: test_name.to_string(),
            phase,
            message: message.into(),
            data: None,
            duration_ms: None,
        }
    }

    /// Add structured data to the log entry.
    #[must_use]
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }

    /// Add duration to the log entry.
    #[must_use]
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }
}

/// Aggregated test result for summary output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Name of the test.
    pub test_name: String,
    /// Whether the test passed.
    pub passed: bool,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
    /// Captured stdout (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    /// Captured stderr (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    /// Exit code (for command-based tests).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Environment variables at test time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<std::collections::HashMap<String, String>>,
    /// Terminal information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_info: Option<TerminalInfo>,
    /// All log entries for this test.
    pub logs: Vec<TestLogEntry>,
}

/// Terminal metadata for debugging output issues.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInfo {
    /// Whether stdout is a TTY.
    pub stdout_tty: bool,
    /// Whether stderr is a TTY.
    pub stderr_tty: bool,
    /// TERM environment variable value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub term: Option<String>,
    /// NO_COLOR environment variable present.
    pub no_color: bool,
    /// FORCE_COLOR environment variable present.
    pub force_color: bool,
    /// Terminal width if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u16>,
}

impl TerminalInfo {
    /// Capture current terminal information.
    pub fn capture() -> Self {
        use std::io::IsTerminal;

        Self {
            stdout_tty: std::io::stdout().is_terminal(),
            stderr_tty: std::io::stderr().is_terminal(),
            term: std::env::var("TERM").ok(),
            no_color: std::env::var("NO_COLOR").is_ok(),
            force_color: std::env::var("FORCE_COLOR").is_ok(),
            width: terminal_width(),
        }
    }
}

/// Get terminal width from COLUMNS env or termsize.
fn terminal_width() -> Option<u16> {
    // Try COLUMNS env first
    if let Ok(cols) = std::env::var("COLUMNS")
        && let Ok(w) = cols.parse()
    {
        return Some(w);
    }
    // Could use termsize crate here, but keep it simple for now
    None
}

/// Structured test logger that writes JSONL output.
///
/// Creates a log file per test in `target/test-logs/` for post-mortem debugging.
pub struct TestLogger {
    test_name: String,
    start_time: Instant,
    logs: Mutex<Vec<TestLogEntry>>,
    log_file: Option<Mutex<std::fs::File>>,
}

impl TestLogger {
    /// Create a new test logger for the given test name.
    pub fn for_test(test_name: &str) -> Self {
        let log_file = Self::create_log_file(test_name).ok();

        let logger = Self {
            test_name: test_name.to_string(),
            start_time: Instant::now(),
            logs: Mutex::new(Vec::new()),
            log_file: log_file.map(Mutex::new),
        };

        // Log test start
        logger.log(TestPhase::Setup, "TEST START");

        logger
    }

    /// Create log file in target/test-logs/.
    ///
    /// Uses CARGO_TARGET_DIR or falls back to looking for target/ in current dir
    /// or parent directories.
    fn create_log_file(test_name: &str) -> std::io::Result<std::fs::File> {
        // Try to find workspace target dir from environment
        let log_dir = if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
            PathBuf::from(target_dir).join("test-logs")
        } else {
            // Look for target/ directory starting from current dir
            let mut cwd = std::env::current_dir().unwrap_or_default();
            loop {
                let target = cwd.join("target");
                if target.is_dir() {
                    break target.join("test-logs");
                }
                if !cwd.pop() {
                    // Fallback to current directory
                    break PathBuf::from("target/test-logs");
                }
            }
        };

        std::fs::create_dir_all(&log_dir)?;

        let safe_name = test_name.replace("::", "_").replace(['/', '\\'], "_");
        let log_path = log_dir.join(format!("{safe_name}.jsonl"));

        std::fs::File::create(log_path)
    }

    /// Log a message for a specific phase.
    pub fn log(&self, phase: TestPhase, message: impl Into<String>) {
        let duration_ms = self.start_time.elapsed().as_millis() as u64;
        let entry = TestLogEntry::new(&self.test_name, phase, message).with_duration(duration_ms);

        self.write_entry(&entry);
    }

    /// Log a message with structured data.
    pub fn log_with_data(
        &self,
        phase: TestPhase,
        message: impl Into<String>,
        data: serde_json::Value,
    ) {
        let duration_ms = self.start_time.elapsed().as_millis() as u64;
        let entry = TestLogEntry::new(&self.test_name, phase, message)
            .with_duration(duration_ms)
            .with_data(data);

        self.write_entry(&entry);
    }

    /// Write a log entry to file and memory.
    fn write_entry(&self, entry: &TestLogEntry) {
        // Store in memory
        if let Ok(mut logs) = self.logs.lock() {
            logs.push(entry.clone());
        }

        // Write to file
        if let Some(file) = &self.log_file
            && let Ok(mut f) = file.lock()
            && let Ok(json) = serde_json::to_string(entry)
        {
            let _ = writeln!(f, "{json}");
        }

        // Also emit to tracing for immediate visibility
        tracing::info!(
            test = %self.test_name,
            phase = %entry.phase,
            duration_ms = entry.duration_ms,
            "{}",
            entry.message
        );
    }

    /// Mark test as passed and log completion.
    pub fn pass(self) {
        self.log(TestPhase::Verify, "TEST PASS");
    }

    /// Mark test as failed and log completion.
    pub fn fail(self, reason: impl Into<String>) {
        self.log_with_data(
            TestPhase::Verify,
            "TEST FAIL",
            serde_json::json!({ "reason": reason.into() }),
        );
    }

    /// Get the elapsed duration.
    pub fn elapsed_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }

    /// Build a test result summary.
    pub fn into_result(self, passed: bool) -> TestResult {
        let duration_ms = self.elapsed_ms();
        let logs = self.logs.lock().map(|l| l.clone()).unwrap_or_default();

        TestResult {
            test_name: self.test_name.clone(),
            passed,
            duration_ms,
            stdout: None,
            stderr: None,
            exit_code: None,
            env: None,
            terminal_info: Some(TerminalInfo::capture()),
            logs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry_serializes_correctly() {
        let entry = TestLogEntry::new("test_example", TestPhase::Setup, "Hello")
            .with_duration(42)
            .with_data(serde_json::json!({"key": "value"}));

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("test_example"));
        assert!(json.contains("setup"));
        assert!(json.contains("Hello"));
        assert!(json.contains("42"));
        assert!(json.contains("key"));
    }

    #[test]
    fn test_logger_creates_entries() {
        let logger = TestLogger::for_test("test_logger_creates_entries");
        logger.log(TestPhase::Execute, "Running test");
        logger.log(TestPhase::Verify, "Checking result");

        let result = logger.into_result(true);
        assert_eq!(result.test_name, "test_logger_creates_entries");
        assert!(result.passed);
        assert!(result.logs.len() >= 3); // START + 2 logs
    }

    #[test]
    fn test_terminal_info_captures_env() {
        let info = TerminalInfo::capture();
        // Just verify it doesn't panic and produces valid data
        // The capture function should always succeed
        let _ = info.stdout_tty;
        let _ = info.stderr_tty;
    }

    #[test]
    fn test_phase_display() {
        assert_eq!(TestPhase::Setup.to_string(), "setup");
        assert_eq!(TestPhase::Execute.to_string(), "execute");
        assert_eq!(TestPhase::Verify.to_string(), "verify");
        assert_eq!(TestPhase::Teardown.to_string(), "teardown");
    }
}
