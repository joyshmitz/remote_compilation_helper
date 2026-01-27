//! Snapshot testing utilities for UI components.
//!
//! Provides helpers for insta snapshot testing with normalization.

use super::strip_ansi_codes;

/// Normalize output for snapshot comparison.
///
/// Removes dynamic content like timestamps and job IDs to enable
/// deterministic snapshot comparisons.
pub fn normalize_for_snapshot(s: &str) -> String {
    let mut result = s.to_string();

    // Remove timestamps (ISO 8601 format)
    // Matches: 2024-01-15T12:30:45, 2024-01-15 12:30:45
    let ts_patterns = [
        r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}",
        r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}",
    ];
    for pattern in ts_patterns {
        let re = regex::Regex::new(pattern).unwrap();
        result = re.replace_all(&result, "<TIMESTAMP>").to_string();
    }

    // Remove job IDs (j-xxxx format)
    let job_re = regex::Regex::new(r"j-[a-z0-9]{4}").unwrap();
    result = job_re.replace_all(&result, "j-XXXX").to_string();

    // Remove build IDs (b-xxxx format)
    let build_re = regex::Regex::new(r"b-[a-z0-9]{4}").unwrap();
    result = build_re.replace_all(&result, "b-XXXX").to_string();

    // Remove worker IDs in common patterns
    let worker_re = regex::Regex::new(r"worker-[a-z0-9]{8}").unwrap();
    result = worker_re
        .replace_all(&result, "worker-XXXXXXXX")
        .to_string();

    // Normalize trailing whitespace
    result
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Strip ANSI codes and normalize for snapshot comparison.
pub fn normalize_plain(s: &str) -> String {
    normalize_for_snapshot(&strip_ansi_codes(s))
}

/// Assert a UI snapshot with normalization.
///
/// Use this macro to create normalized snapshots of UI output.
///
/// # Example
///
/// ```ignore
/// assert_ui_snapshot!("status_table", table.render_to_string());
/// ```
#[macro_export]
macro_rules! assert_ui_snapshot {
    ($name:expr, $value:expr) => {{
        let normalized = $crate::support::normalize_for_snapshot(&$value);
        insta::with_settings!({
            snapshot_path => "../snapshots",
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!($name, normalized);
        });
    }};
}

/// Assert a plain text UI snapshot (ANSI stripped and normalized).
#[macro_export]
macro_rules! assert_plain_snapshot {
    ($name:expr, $value:expr) => {{
        let normalized = $crate::support::normalize_plain(&$value);
        insta::with_settings!({
            snapshot_path => "../snapshots",
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!($name, normalized);
        });
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_timestamp_iso() {
        let input = "Started at 2024-01-15T12:30:45";
        let result = normalize_for_snapshot(input);
        assert_eq!(result, "Started at <TIMESTAMP>");
    }

    #[test]
    fn test_normalize_timestamp_space() {
        let input = "Started at 2024-01-15 12:30:45";
        let result = normalize_for_snapshot(input);
        assert_eq!(result, "Started at <TIMESTAMP>");
    }

    #[test]
    fn test_normalize_job_id() {
        let input = "Job j-a3f2 completed";
        let result = normalize_for_snapshot(input);
        assert_eq!(result, "Job j-XXXX completed");
    }

    #[test]
    fn test_normalize_build_id() {
        let input = "Build b-x9k1 started";
        let result = normalize_for_snapshot(input);
        assert_eq!(result, "Build b-XXXX started");
    }

    #[test]
    fn test_normalize_worker_id() {
        let input = "Worker worker-ab12cd34";
        let result = normalize_for_snapshot(input);
        assert_eq!(result, "Worker worker-XXXXXXXX");
    }

    #[test]
    fn test_normalize_trailing_whitespace() {
        let input = "line1   \nline2    \n";
        let result = normalize_for_snapshot(input);
        // lines() + join("\n") doesn't preserve trailing newlines
        assert_eq!(result, "line1\nline2");
    }

    #[test]
    fn test_normalize_plain_strips_ansi() {
        let input = "\x1b[32mGreen\x1b[0m at 2024-01-15T12:30:45";
        let result = normalize_plain(input);
        assert_eq!(result, "Green at <TIMESTAMP>");
    }

    #[test]
    fn test_normalize_multiple_ids() {
        let input = "Jobs: j-a1b2, j-c3d4, j-e5f6";
        let result = normalize_for_snapshot(input);
        assert_eq!(result, "Jobs: j-XXXX, j-XXXX, j-XXXX");
    }
}
