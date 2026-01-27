//! Test utilities for RCH.
//!
//! Provides structured test logging, output capture, and debugging infrastructure
//! for consistent test output across the codebase.
//!
//! # Features
//!
//! - JSONL test logs for CI debugging
//! - Structured phase markers (TEST START/PASS/FAIL)
//! - Terminal metadata capture
//! - Output capture for stdout/stderr
//!
//! # Usage
//!
//! ```ignore
//! use rch_common::testing::{TestLogger, TestPhase};
//!
//! #[test]
//! fn test_example() {
//!     let logger = TestLogger::for_test("test_example");
//!     logger.log(TestPhase::Setup, "Initializing test state");
//!     // ... test logic ...
//!     logger.log(TestPhase::Verify, "Checking results");
//! }
//! ```

mod log;

pub use log::{TestLogEntry, TestLogger, TestPhase, TestResult};
