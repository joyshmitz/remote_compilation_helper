//! Test support utilities for UI integration testing.
//!
//! Provides mock terminal, snapshot testing helpers, and test data factories.

pub mod factories;
pub mod mock_terminal;
pub mod snapshots;

pub use factories::*;
pub use mock_terminal::*;
pub use snapshots::*;
