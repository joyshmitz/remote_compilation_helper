//! Error display utilities for unified error handling.
//!
//! This module provides the `display_error()` helper function that:
//! - Renders errors using ErrorPanel for rich/plain terminal output
//! - Falls back to simple text for non-TTY output
//! - Respects NO_COLOR and FORCE_COLOR environment variables
//! - Supports JSON serialization for machine output
//!
//! # Example
//!
//! ```ignore
//! use rch_common::ui::{display_error, OutputContext};
//! use anyhow::anyhow;
//!
//! let err = anyhow!("Something went wrong");
//! let ctx = OutputContext::detect();
//!
//! display_error(&err, &ctx);
//! ```

use crate::errors::catalog::ErrorCode;

use super::{ErrorPanel, OutputContext};

/// Display an error using the appropriate format for the output context.
///
/// This function:
/// - Detects if stdout/stderr is a TTY
/// - Respects NO_COLOR and FORCE_COLOR environment variables
/// - Uses ErrorPanel for rich output when available
/// - Falls back to simple text for non-TTY/machine output
/// - Logs the full error chain to debug log regardless of display mode
pub fn display_error<E: std::error::Error>(error: &E, ctx: &OutputContext) {
    // Log full error chain to debug log
    tracing::debug!("Error occurred: {}", error);
    let mut source = error.source();
    while let Some(err) = source {
        tracing::debug!("  Caused by: {}", err);
        source = err.source();
    }

    // For machine output, do nothing - caller should use to_json()
    if ctx.is_machine() {
        return;
    }

    // Create ErrorPanel from the error
    let panel = error_to_panel(error);

    // Render the panel
    panel.render(*ctx);
}

/// Display an error with a specific error code.
///
/// Uses the error catalog to provide consistent error codes and remediation.
pub fn display_error_with_code<E: std::error::Error>(
    error: &E,
    code: ErrorCode,
    ctx: &OutputContext,
) {
    // Log full error chain to debug log
    tracing::debug!("Error occurred [{}]: {}", code.code_string(), error);
    let mut source = error.source();
    while let Some(err) = source {
        tracing::debug!("  Caused by: {}", err);
        source = err.source();
    }

    // For machine output, do nothing - caller should use to_json()
    if ctx.is_machine() {
        return;
    }

    // Create ErrorPanel from error code with additional context
    let entry = code.entry();
    let mut panel = ErrorPanel::error(&entry.code, &entry.message).message(error.to_string());

    // Add error chain
    let mut source = error.source();
    while let Some(err) = source {
        panel = panel.caused_by(err.to_string(), None);
        source = err.source();
    }

    // Add remediation from catalog
    for step in entry.remediation {
        panel = panel.suggestion(step);
    }

    // Render the panel
    panel.render(*ctx);
}

/// Convert any error to an ErrorPanel.
///
/// This function attempts to extract structured information from the error,
/// falling back to generic error display if specific handling isn't available.
pub fn error_to_panel<E: std::error::Error>(error: &E) -> ErrorPanel {
    let error_string = error.to_string();

    // Try to extract error code from the error message (RCH-Exxx pattern)
    let (code, title) = extract_error_info(&error_string);

    let mut panel = ErrorPanel::error(&code, &title);

    // Set the full error message
    if title != error_string {
        panel = panel.message(error_string);
    }

    // Add error chain as caused_by
    let mut source = error.source();
    while let Some(err) = source {
        panel = panel.caused_by(err.to_string(), None);
        source = err.source();
    }

    panel
}

/// Extract error code and title from an error message.
///
/// Looks for patterns like "RCH-E042: Something failed" and extracts
/// the code and message separately.
fn extract_error_info(message: &str) -> (String, String) {
    // Try to match RCH-Exxx pattern
    if let Some(caps) = extract_rch_code(message) {
        return caps;
    }

    // Default to generic error
    ("RCH-E500".to_string(), message.to_string())
}

/// Extract RCH error code from message if present.
fn extract_rch_code(message: &str) -> Option<(String, String)> {
    // Look for "RCH-Exxx" pattern
    let prefix = "RCH-E";
    if let Some(start) = message.find(prefix) {
        let code_start = start;
        let after_prefix = start + prefix.len();

        // Find the end of the error code (digits)
        let code_end = message[after_prefix..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .count()
            + after_prefix;

        if code_end > after_prefix {
            let code = message[code_start..code_end].to_string();

            // Extract the message after the code
            let rest = &message[code_end..];
            let title = if let Some(stripped) = rest.strip_prefix(": ") {
                stripped.to_string()
            } else if let Some(stripped) = rest.strip_prefix("] ") {
                stripped.to_string()
            } else {
                rest.trim_start_matches(&[' ', ':', ']'][..]).to_string()
            };

            return Some((
                code,
                if title.is_empty() {
                    message.to_string()
                } else {
                    title
                },
            ));
        }
    }

    None
}

/// Convert an anyhow::Error to an ErrorPanel.
pub fn anyhow_to_panel(error: &anyhow::Error) -> ErrorPanel {
    let error_string = error.to_string();
    let (code, title) = extract_error_info(&error_string);

    let mut panel = ErrorPanel::error(&code, &title);

    // Set the full error message
    if title != error_string {
        panel = panel.message(error_string);
    }

    // Add error chain
    for cause in error.chain().skip(1) {
        panel = panel.caused_by(cause.to_string(), None);
    }

    panel
}

/// Display an anyhow::Error using ErrorPanel.
pub fn display_anyhow_error(error: &anyhow::Error, ctx: &OutputContext) {
    // Log full error chain to debug log
    tracing::debug!("Error occurred: {}", error);
    for cause in error.chain().skip(1) {
        tracing::debug!("  Caused by: {}", cause);
    }

    // For machine output, do nothing - caller should use to_json()
    if ctx.is_machine() {
        return;
    }

    let panel = anyhow_to_panel(error);
    panel.render(*ctx);
}

/// Get JSON representation of an error for machine output.
pub fn error_to_json<E: std::error::Error>(error: &E) -> serde_json::Result<String> {
    let panel = error_to_panel(error);
    panel.to_json()
}

/// Get JSON representation of an anyhow::Error for machine output.
pub fn anyhow_to_json(error: &anyhow::Error) -> serde_json::Result<String> {
    let panel = anyhow_to_panel(error);
    panel.to_json()
}

/// Trait for errors that can be converted to ErrorPanel.
pub trait IntoErrorPanel {
    /// Convert this error into an ErrorPanel.
    fn into_panel(self) -> ErrorPanel;
}

impl<E: std::error::Error> IntoErrorPanel for E {
    fn into_panel(self) -> ErrorPanel {
        error_to_panel(&self)
    }
}

/// Extension trait for Result types to display errors.
#[allow(clippy::result_unit_err)]
pub trait ResultExt<T> {
    /// Display the error if present using the given output context.
    fn display_err(self, ctx: &OutputContext) -> Result<T, ()>;

    /// Display the error if present and exit with the given code.
    fn display_and_exit(self, ctx: &OutputContext, exit_code: i32) -> T;
}

impl<T, E: std::error::Error> ResultExt<T> for Result<T, E> {
    fn display_err(self, ctx: &OutputContext) -> Result<T, ()> {
        match self {
            Ok(v) => Ok(v),
            Err(e) => {
                display_error(&e, ctx);
                Err(())
            }
        }
    }

    fn display_and_exit(self, ctx: &OutputContext, exit_code: i32) -> T {
        match self {
            Ok(v) => v,
            Err(e) => {
                display_error(&e, ctx);
                std::process::exit(exit_code);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_extract_rch_code_with_colon() {
        let result = extract_rch_code("RCH-E042: Worker Connection Failed");
        assert_eq!(
            result,
            Some((
                "RCH-E042".to_string(),
                "Worker Connection Failed".to_string()
            ))
        );
    }

    #[test]
    fn test_extract_rch_code_with_bracket() {
        let result = extract_rch_code("[RCH-E100] SSH failed");
        assert_eq!(
            result,
            Some(("RCH-E100".to_string(), "SSH failed".to_string()))
        );
    }

    #[test]
    fn test_extract_rch_code_no_code() {
        let result = extract_rch_code("Some random error message");
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_error_info_with_code() {
        let (code, title) = extract_error_info("RCH-E502: Daemon not running");
        assert_eq!(code, "RCH-E502");
        assert_eq!(title, "Daemon not running");
    }

    #[test]
    fn test_extract_error_info_without_code() {
        let (code, title) = extract_error_info("Something went wrong");
        assert_eq!(code, "RCH-E500");
        assert_eq!(title, "Something went wrong");
    }

    #[test]
    fn test_error_to_panel_simple() {
        let err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let panel = error_to_panel(&err);

        assert_eq!(panel.code, "RCH-E500");
        assert!(panel.title.contains("not found"));
    }

    #[test]
    fn test_error_to_panel_with_rch_code() {
        // Create a custom error type that includes RCH code
        #[derive(Debug)]
        struct RchTestError;
        impl std::fmt::Display for RchTestError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "RCH-E042: Test error message")
            }
        }
        impl std::error::Error for RchTestError {}

        let err = RchTestError;
        let panel = error_to_panel(&err);

        assert_eq!(panel.code, "RCH-E042");
        assert_eq!(panel.title, "Test error message");
    }

    #[test]
    fn test_error_to_panel_with_source() {
        let inner = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let outer = io::Error::other(inner.to_string());

        let panel = error_to_panel(&outer);

        // Should capture the error message
        assert!(panel.title.contains("access denied"));
    }

    #[test]
    fn test_display_error_machine_mode_silent() {
        let err = io::Error::new(io::ErrorKind::NotFound, "test error");
        let ctx = OutputContext::Machine;

        // Should not panic and not produce output
        display_error(&err, &ctx);
    }

    #[test]
    fn test_display_error_plain_mode() {
        let err = io::Error::new(io::ErrorKind::NotFound, "test error");
        let ctx = OutputContext::Plain;

        // Should not panic
        display_error(&err, &ctx);
    }

    #[test]
    fn test_anyhow_to_panel() {
        let err = anyhow::anyhow!("RCH-E100: SSH connection failed");
        let panel = anyhow_to_panel(&err);

        assert_eq!(panel.code, "RCH-E100");
        assert_eq!(panel.title, "SSH connection failed");
    }

    #[test]
    fn test_anyhow_to_panel_with_context() {
        let err = anyhow::anyhow!("inner error").context("outer context");
        let panel = anyhow_to_panel(&err);

        // Should capture the outer context
        assert!(panel.title.contains("outer context") || panel.message.is_some());
    }

    #[test]
    fn test_error_to_json() {
        let err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let json = error_to_json(&err).expect("JSON serialization failed");

        assert!(json.contains("RCH-E500"));
        assert!(json.contains("not found"));
    }

    #[test]
    fn test_anyhow_to_json() {
        let err = anyhow::anyhow!("test error");
        let json = anyhow_to_json(&err).expect("JSON serialization failed");

        assert!(json.contains("RCH-E500"));
        assert!(json.contains("test error"));
    }

    #[test]
    fn test_result_ext_display_err_ok() {
        let result: Result<i32, io::Error> = Ok(42);
        let ctx = OutputContext::Plain;

        assert_eq!(result.display_err(&ctx), Ok(42));
    }

    #[test]
    fn test_result_ext_display_err_error() {
        let result: Result<i32, io::Error> =
            Err(io::Error::new(io::ErrorKind::NotFound, "not found"));
        let ctx = OutputContext::Plain;

        assert_eq!(result.display_err(&ctx), Err(()));
    }
}
