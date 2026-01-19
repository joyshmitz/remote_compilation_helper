//! UI utilities for RCH terminal output.
//!
//! This module provides context-aware output utilities that automatically
//! adapt to the current environment (interactive terminal, hook mode, piped, etc.).
//!
//! # Key Components
//!
//! - [`OutputContext`]: Detects and represents the current output context
//!
//! # Design Philosophy
//!
//! - **stdout** is for DATA (JSON, compilation output, etc.)
//! - **stderr** is for DIAGNOSTICS (status, progress, errors)
//!
//! Rich output goes to stderr so that:
//! 1. Agents can parse stdout reliably
//! 2. Piping `rch status | jq` works correctly
//! 3. Compilation output passthrough is unaffected

pub mod context;

pub use context::OutputContext;
