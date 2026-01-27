//! UI Integration Tests for RCH.
//!
//! Comprehensive tests for terminal UI components including:
//! - Context detection
//! - Component rendering
//! - Snapshot tests
//! - Performance benchmarks

mod support;

use rch_common::ui::{ErrorPanel, ErrorSeverity, Icons, OutputContext, RchTheme};
use std::time::Instant;

// ============================================================================
// CONTEXT DETECTION TESTS
// ============================================================================

mod context_tests {
    use super::*;

    #[test]
    fn test_output_context_supports_rich_only_interactive() {
        assert!(OutputContext::Interactive.supports_rich());
        assert!(!OutputContext::Plain.supports_rich());
        assert!(!OutputContext::Hook.supports_rich());
        assert!(!OutputContext::Machine.supports_rich());
        assert!(!OutputContext::Colored.supports_rich());
    }

    #[test]
    fn test_output_context_supports_color() {
        assert!(OutputContext::Interactive.supports_color());
        assert!(OutputContext::Colored.supports_color());
        assert!(!OutputContext::Plain.supports_color());
        assert!(!OutputContext::Hook.supports_color());
        assert!(!OutputContext::Machine.supports_color());
    }

    #[test]
    fn test_output_context_is_machine() {
        assert!(OutputContext::Hook.is_machine());
        assert!(OutputContext::Machine.is_machine());
        assert!(!OutputContext::Interactive.is_machine());
        assert!(!OutputContext::Colored.is_machine());
        assert!(!OutputContext::Plain.is_machine());
    }

    #[test]
    fn test_output_context_constructors() {
        assert_eq!(OutputContext::plain(), OutputContext::Plain);
        assert_eq!(OutputContext::interactive(), OutputContext::Interactive);
        assert_eq!(OutputContext::machine(), OutputContext::Machine);
    }

    #[test]
    fn test_output_context_display() {
        assert_eq!(OutputContext::Hook.to_string(), "hook");
        assert_eq!(OutputContext::Machine.to_string(), "machine");
        assert_eq!(OutputContext::Interactive.to_string(), "interactive");
        assert_eq!(OutputContext::Colored.to_string(), "colored");
        assert_eq!(OutputContext::Plain.to_string(), "plain");
    }
}

// ============================================================================
// COMPONENT RENDERING TESTS
// ============================================================================

mod component_tests {
    use super::*;

    #[test]
    fn test_error_panel_creation() {
        let panel = ErrorPanel::new("RCH-E042", "Test Error");
        assert_eq!(panel.code, "RCH-E042");
        assert_eq!(panel.title, "Test Error");
        assert_eq!(panel.severity, ErrorSeverity::Error);
    }

    #[test]
    fn test_error_panel_builder() {
        let panel = ErrorPanel::error("RCH-E001", "Connection Failed")
            .message("Could not connect to worker")
            .context("Host", "192.168.1.10")
            .context("Port", "22")
            .suggestion("Check if worker is online")
            .suggestion("Verify SSH key");

        assert_eq!(panel.severity, ErrorSeverity::Error);
        assert_eq!(
            panel.message.as_deref(),
            Some("Could not connect to worker")
        );
        assert_eq!(panel.context.len(), 2);
        assert_eq!(panel.suggestions.len(), 2);
    }

    #[test]
    fn test_error_panel_severities() {
        let error = ErrorPanel::error("E001", "Error");
        assert_eq!(error.severity, ErrorSeverity::Error);

        let warning = ErrorPanel::warning("W001", "Warning");
        assert_eq!(warning.severity, ErrorSeverity::Warning);

        let info = ErrorPanel::info("I001", "Info");
        assert_eq!(info.severity, ErrorSeverity::Info);
    }

    #[test]
    fn test_error_panel_message_truncation() {
        let long_message = "a".repeat(1000);
        let panel = ErrorPanel::new("E001", "Test").message_truncated(long_message, 100);

        assert!(panel.truncated);
        assert!(panel.message.as_ref().unwrap().len() <= 100);
        assert!(panel.message.as_ref().unwrap().ends_with("..."));
    }

    #[test]
    fn test_error_panel_no_truncation_needed() {
        let short_message = "Short message";
        let panel = ErrorPanel::new("E001", "Test").message_truncated(short_message, 100);

        assert!(!panel.truncated);
        assert_eq!(panel.message.as_deref(), Some("Short message"));
    }

    #[test]
    fn test_error_panel_json_serialization() {
        let panel = ErrorPanel::error("RCH-E001", "Test Error")
            .message("Test message")
            .context("Key", "Value");

        let json = panel.to_json().expect("JSON serialization failed");
        assert!(json.contains("RCH-E001"));
        assert!(json.contains("Test Error"));
        assert!(json.contains("Test message"));
    }

    #[test]
    fn test_error_panel_render_machine_mode_silent() {
        let panel = ErrorPanel::error("E001", "Test");
        // Should not panic in machine mode
        panel.render(OutputContext::Machine);
    }

    #[test]
    fn test_error_panel_render_plain_mode() {
        let panel = ErrorPanel::error("E001", "Test")
            .message("Message")
            .context("Key", "Value")
            .suggestion("Do something");

        // Should not panic in plain mode
        panel.render(OutputContext::Plain);
    }

    #[test]
    fn test_icons_provide_fallbacks() {
        // Interactive mode should use Unicode
        let check_interactive = Icons::check(OutputContext::Interactive);
        assert!(!check_interactive.is_empty());

        // Plain mode should use ASCII fallback
        let check_plain = Icons::check(OutputContext::Plain);
        assert!(!check_plain.is_empty());

        // Both should provide something displayable
        assert!(check_interactive.chars().count() > 0);
        assert!(check_plain.chars().count() > 0);
    }

    #[test]
    fn test_theme_colors_are_valid_hex() {
        let colors = [
            RchTheme::PRIMARY,
            RchTheme::SECONDARY,
            RchTheme::ACCENT,
            RchTheme::SUCCESS,
            RchTheme::WARNING,
            RchTheme::ERROR,
            RchTheme::INFO,
        ];

        for color in colors {
            assert!(color.starts_with('#'), "Color should start with #: {color}");
            assert_eq!(color.len(), 7, "Color should be 7 chars: {color}");
            assert!(
                color[1..].chars().all(|c| c.is_ascii_hexdigit()),
                "Color should be valid hex: {color}"
            );
        }
    }
}

// ============================================================================
// MOCK TERMINAL TESTS
// ============================================================================

mod mock_terminal_tests {
    use super::support::*;

    #[test]
    fn test_mock_terminal_creation() {
        let term = MockTerminal::new(120, 40);
        assert_eq!(term.width, 120);
        assert_eq!(term.height, 40);
        assert!(term.supports_color);
        assert!(term.supports_unicode);
    }

    #[test]
    fn test_mock_terminal_modifiers() {
        let term = MockTerminal::new(80, 24).with_no_color().with_ascii_only();
        assert!(!term.supports_color);
        assert!(!term.supports_unicode);
    }

    #[test]
    fn test_mock_terminal_write_and_read() {
        let mut term = MockTerminal::new(80, 24);
        term.write("Hello, world!").unwrap();
        assert_eq!(term.output(), "Hello, world!");
    }

    #[test]
    fn test_mock_terminal_ansi_detection() {
        let mut term = MockTerminal::new(80, 24);
        term.write("plain text").unwrap();
        assert!(!term.has_ansi_codes());

        term.clear();
        term.write("\x1b[32mgreen\x1b[0m").unwrap();
        assert!(term.has_ansi_codes());
    }

    #[test]
    fn test_strip_ansi_codes() {
        let with_ansi = "\x1b[31mRed\x1b[0m Text";
        let plain = strip_ansi_codes(with_ansi);
        assert_eq!(plain, "Red Text");
    }
}

// ============================================================================
// SNAPSHOT TESTS
// ============================================================================

mod snapshot_tests {
    use super::support::*;

    #[test]
    fn test_normalize_removes_timestamps() {
        let input = "Started at 2024-01-15T12:30:45";
        let result = normalize_for_snapshot(input);
        assert_eq!(result, "Started at <TIMESTAMP>");
    }

    #[test]
    fn test_normalize_removes_job_ids() {
        let input = "Job j-a3f2 completed";
        let result = normalize_for_snapshot(input);
        assert_eq!(result, "Job j-XXXX completed");
    }

    #[test]
    fn test_normalize_removes_trailing_whitespace() {
        let input = "line1   \nline2    ";
        let result = normalize_for_snapshot(input);
        assert_eq!(result, "line1\nline2");
    }

    #[test]
    fn test_normalize_plain_strips_ansi() {
        let input = "\x1b[32mGreen\x1b[0m at 2024-01-15T12:30:45";
        let result = normalize_plain(input);
        assert_eq!(result, "Green at <TIMESTAMP>");
    }
}

// ============================================================================
// FACTORY TESTS
// ============================================================================

mod factory_tests {
    use super::support::*;

    #[test]
    fn test_mock_status_data() {
        let data = mock_status_data();
        assert!(data.connected);
        assert_eq!(data.workers_online, 3);
        assert_eq!(data.active_jobs.len(), 1);
    }

    #[test]
    fn test_mock_worker_data() {
        let workers = mock_worker_data();
        assert_eq!(workers.len(), 3);

        // Varied statuses
        assert_eq!(workers[0].status, MockWorkerStatus::Online);
        assert_eq!(workers[1].status, MockWorkerStatus::Busy);
        assert_eq!(workers[2].status, MockWorkerStatus::Offline);
    }

    #[test]
    fn test_mock_error_scenarios() {
        let scenarios = mock_error_scenarios();
        assert!(!scenarios.is_empty());

        for scenario in &scenarios {
            assert!(!scenario.code.is_empty());
            assert!(!scenario.message.is_empty());
            assert!(!scenario.suggestions.is_empty());
        }
    }

    #[test]
    fn test_fixed_timestamp_is_deterministic() {
        let ts1 = fixed_timestamp();
        let ts2 = fixed_timestamp();
        assert_eq!(ts1, ts2);
    }
}

// ============================================================================
// PERFORMANCE TESTS
// ============================================================================

mod performance_tests {
    use super::*;
    use support::*;

    #[test]
    fn test_error_panel_creation_performance() {
        let start = Instant::now();
        for _ in 0..1000 {
            let _ = ErrorPanel::error("RCH-E001", "Test Error")
                .message("Test message")
                .context("Key1", "Value1")
                .context("Key2", "Value2")
                .suggestion("Suggestion 1")
                .suggestion("Suggestion 2");
        }
        let duration = start.elapsed();

        // 1000 panel creations should take < 100ms
        assert!(
            duration.as_millis() < 100,
            "ErrorPanel creation too slow: {:?} for 1000 iterations",
            duration
        );
    }

    #[test]
    fn test_mock_terminal_performance() {
        let start = Instant::now();
        for _ in 0..1000 {
            let mut term = MockTerminal::new(80, 24);
            term.write("Test output line 1\n").unwrap();
            term.write("Test output line 2\n").unwrap();
            let _ = term.output();
            let _ = term.plain_output();
            let _ = term.line_count();
        }
        let duration = start.elapsed();

        // 1000 terminal operations should take < 100ms
        assert!(
            duration.as_millis() < 100,
            "MockTerminal operations too slow: {:?} for 1000 iterations",
            duration
        );
    }

    #[test]
    fn test_ansi_stripping_performance() {
        let ansi_text = "\x1b[31mRed\x1b[0m \x1b[32mGreen\x1b[0m \x1b[34mBlue\x1b[0m";
        let start = Instant::now();
        for _ in 0..10000 {
            let _ = strip_ansi_codes(ansi_text);
        }
        let duration = start.elapsed();

        // 10000 strip operations should take < 50ms
        assert!(
            duration.as_millis() < 50,
            "ANSI stripping too slow: {:?} for 10000 iterations",
            duration
        );
    }

    #[test]
    fn test_json_serialization_performance() {
        let panel = ErrorPanel::error("RCH-E001", "Test Error")
            .message("Test message with some content")
            .context("Host", "192.168.1.10")
            .context("Port", "22")
            .suggestion("Check connectivity")
            .suggestion("Verify credentials");

        let start = Instant::now();
        for _ in 0..1000 {
            let _ = panel.to_json();
        }
        let duration = start.elapsed();

        // 1000 JSON serializations should take < 100ms
        assert!(
            duration.as_millis() < 100,
            "JSON serialization too slow: {:?} for 1000 iterations",
            duration
        );
    }
}

// ============================================================================
// ERROR SEVERITY TESTS
// ============================================================================

mod severity_tests {
    use super::*;

    #[test]
    fn test_severity_colors() {
        assert_eq!(ErrorSeverity::Error.color(), RchTheme::ERROR);
        assert_eq!(ErrorSeverity::Warning.color(), RchTheme::WARNING);
        assert_eq!(ErrorSeverity::Info.color(), RchTheme::INFO);
    }

    #[test]
    fn test_severity_names() {
        assert_eq!(ErrorSeverity::Error.name(), "ERROR");
        assert_eq!(ErrorSeverity::Warning.name(), "WARN");
        assert_eq!(ErrorSeverity::Info.name(), "INFO");
    }

    #[test]
    fn test_severity_icons_plain() {
        let ctx = OutputContext::Plain;
        // Should not panic and return non-empty strings
        assert!(!ErrorSeverity::Error.icon(ctx).is_empty());
        assert!(!ErrorSeverity::Warning.icon(ctx).is_empty());
        assert!(!ErrorSeverity::Info.icon(ctx).is_empty());
    }

    #[test]
    fn test_severity_default() {
        let severity = ErrorSeverity::default();
        assert_eq!(severity, ErrorSeverity::Error);
    }
}
