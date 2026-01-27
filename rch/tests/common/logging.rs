//! Test logging utilities for rch tests.
//!
//! This module provides convenient wrappers around `rch_common::testing`
//! to standardize test logging across the crate.
//!
//! # Usage
//!
//! ```ignore
//! use crate::common::logging::{init_test_logging, TestLogger};
//!
//! #[test]
//! fn test_example() {
//!     init_test_logging();
//!     let logger = TestLogger::for_test("test_example");
//!
//!     // ... test logic ...
//!
//!     logger.pass();  // or logger.fail("reason")
//! }
//! ```

use tracing_subscriber::{EnvFilter, fmt};

/// Re-export TestLogger and related types from rch-common for convenience.
#[allow(unused_imports)]
pub use rch_common::testing::{TestLogger, TestPhase};

/// Initialize tracing for tests.
///
/// This sets up tracing with a test writer and debug-level filtering for rch.
/// Call this at the start of tests that need logging visibility.
pub fn init_test_logging() {
    let _ = fmt()
        .with_test_writer()
        .with_env_filter(EnvFilter::from_default_env().add_directive("rch=debug".parse().unwrap()))
        .try_init();
}

/// Log a message using tracing with test target.
///
/// This macro is for backwards compatibility. New tests should prefer
/// using `TestLogger` directly for structured JSONL output.
#[macro_export]
macro_rules! test_log {
    ($($arg:tt)*) => {
        tracing::info!(target: "test", $($arg)*);
    };
}

/// Convenience macro to create a TestLogger for the current test.
///
/// Uses the test function name as the log name.
#[macro_export]
macro_rules! test_logger {
    () => {{
        // Get function name from module path
        fn _f() {}
        fn _type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = _type_name_of(_f);
        // Extract just the function name (last segment before "::_f")
        let name = name.strip_suffix("::_f").unwrap_or(name);
        let name = name.rsplit("::").next().unwrap_or(name);
        rch_common::testing::TestLogger::for_test(name)
    }};
}
