//! Rust toolchain information types.
//!
//! Types for representing Rust toolchain information used in the RCH protocol
//! for toolchain synchronization between local and remote workers.

use serde::{Deserialize, Serialize};

/// Detected Rust toolchain information.
///
/// Contains the channel, optional date, and full version string needed
/// to ensure worker uses the same toolchain as the local project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolchainInfo {
    /// The channel: "stable", "beta", "nightly", or specific version like "1.75.0".
    pub channel: String,
    /// Optional date for nightly/beta: "2024-01-15".
    pub date: Option<String>,
    /// Full version string from rustc --version.
    pub full_version: String,
}

impl ToolchainInfo {
    /// Create a new ToolchainInfo.
    pub fn new(
        channel: impl Into<String>,
        date: Option<String>,
        full_version: impl Into<String>,
    ) -> Self {
        Self {
            channel: channel.into(),
            date,
            full_version: full_version.into(),
        }
    }

    /// Format for rustup run command.
    ///
    /// Returns the toolchain identifier suitable for `rustup run <toolchain>`.
    pub fn rustup_toolchain(&self) -> String {
        match &self.date {
            Some(date) => format!("{}-{}", self.channel, date),
            None => self.channel.clone(),
        }
    }

    /// Check if this is a nightly toolchain.
    pub fn is_nightly(&self) -> bool {
        self.channel == "nightly"
    }

    /// Check if this is a beta toolchain.
    pub fn is_beta(&self) -> bool {
        self.channel == "beta"
    }

    /// Check if this is a stable toolchain.
    pub fn is_stable(&self) -> bool {
        self.channel == "stable"
    }
}

impl std::fmt::Display for ToolchainInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.rustup_toolchain())
    }
}

/// Wrap a command to run with a specific toolchain.
///
/// If toolchain is provided, wraps the command with `rustup run <toolchain>`.
/// Otherwise returns the original command unchanged.
pub fn wrap_command_with_toolchain(command: &str, toolchain: Option<&ToolchainInfo>) -> String {
    match toolchain {
        Some(tc) => format!("rustup run {} {}", tc.rustup_toolchain(), command),
        None => command.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toolchain_info_new() {
        let tc = ToolchainInfo::new(
            "nightly",
            Some("2024-01-15".to_string()),
            "rustc 1.76.0-nightly",
        );
        assert_eq!(tc.channel, "nightly");
        assert_eq!(tc.date, Some("2024-01-15".to_string()));
    }

    #[test]
    fn test_rustup_toolchain_with_date() {
        let tc = ToolchainInfo {
            channel: "nightly".to_string(),
            date: Some("2024-01-15".to_string()),
            full_version: "".to_string(),
        };
        assert_eq!(tc.rustup_toolchain(), "nightly-2024-01-15");
    }

    #[test]
    fn test_rustup_toolchain_without_date() {
        let tc = ToolchainInfo {
            channel: "stable".to_string(),
            date: None,
            full_version: "".to_string(),
        };
        assert_eq!(tc.rustup_toolchain(), "stable");
    }

    #[test]
    fn test_toolchain_info_display() {
        let tc = ToolchainInfo {
            channel: "nightly".to_string(),
            date: Some("2024-01-15".to_string()),
            full_version: "".to_string(),
        };
        assert_eq!(tc.to_string(), "nightly-2024-01-15");
    }

    #[test]
    fn test_is_nightly() {
        let tc = ToolchainInfo::new("nightly", None, "");
        assert!(tc.is_nightly());
        assert!(!tc.is_stable());
        assert!(!tc.is_beta());
    }

    #[test]
    fn test_is_stable() {
        let tc = ToolchainInfo::new("stable", None, "");
        assert!(tc.is_stable());
        assert!(!tc.is_nightly());
        assert!(!tc.is_beta());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let tc = ToolchainInfo {
            channel: "nightly".to_string(),
            date: Some("2024-01-15".to_string()),
            full_version: "rustc 1.76.0-nightly (abc123 2024-01-15)".to_string(),
        };
        let json = serde_json::to_string(&tc).unwrap();
        let parsed: ToolchainInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(tc, parsed);
    }

    #[test]
    fn test_wrap_command_with_toolchain() {
        let tc = ToolchainInfo::new("nightly", Some("2024-01-15".to_string()), "");
        let wrapped = wrap_command_with_toolchain("cargo build", Some(&tc));
        assert_eq!(wrapped, "rustup run nightly-2024-01-15 cargo build");
    }

    #[test]
    fn test_wrap_command_no_toolchain() {
        let wrapped = wrap_command_with_toolchain("cargo build", None);
        assert_eq!(wrapped, "cargo build");
    }

    #[test]
    fn test_wrap_command_stable_toolchain() {
        let tc = ToolchainInfo::new("stable", None, "");
        let wrapped = wrap_command_with_toolchain("cargo test", Some(&tc));
        assert_eq!(wrapped, "rustup run stable cargo test");
    }

    // === Additional command wrapping tests for toolchain synchronization ===

    #[test]
    fn test_is_beta() {
        let tc = ToolchainInfo::new("beta", None, "");
        assert!(tc.is_beta());
        assert!(!tc.is_stable());
        assert!(!tc.is_nightly());
    }

    #[test]
    fn test_wrap_command_beta_with_date() {
        let tc = ToolchainInfo::new("beta", Some("2024-02-01".to_string()), "");
        let wrapped = wrap_command_with_toolchain("cargo check", Some(&tc));
        assert_eq!(wrapped, "rustup run beta-2024-02-01 cargo check");
    }

    #[test]
    fn test_wrap_command_specific_version() {
        // Specific version like "1.75.0" without date
        let tc = ToolchainInfo::new("1.75.0", None, "rustc 1.75.0");
        let wrapped = wrap_command_with_toolchain("cargo build --release", Some(&tc));
        assert_eq!(wrapped, "rustup run 1.75.0 cargo build --release");
    }

    #[test]
    fn test_wrap_command_with_complex_args() {
        let tc = ToolchainInfo::new("nightly", None, "");
        let cmd = "cargo build --features \"feature1,feature2\" --target x86_64-unknown-linux-gnu";
        let wrapped = wrap_command_with_toolchain(cmd, Some(&tc));
        assert_eq!(
            wrapped,
            "rustup run nightly cargo build --features \"feature1,feature2\" --target x86_64-unknown-linux-gnu"
        );
    }

    #[test]
    fn test_wrap_command_with_path_spaces() {
        let tc = ToolchainInfo::new("stable", None, "");
        let cmd = "cargo build --manifest-path \"/path/with spaces/Cargo.toml\"";
        let wrapped = wrap_command_with_toolchain(cmd, Some(&tc));
        assert_eq!(
            wrapped,
            "rustup run stable cargo build --manifest-path \"/path/with spaces/Cargo.toml\""
        );
    }

    #[test]
    fn test_wrap_command_empty_string() {
        let tc = ToolchainInfo::new("stable", None, "");
        let wrapped = wrap_command_with_toolchain("", Some(&tc));
        assert_eq!(wrapped, "rustup run stable ");
    }

    #[test]
    fn test_wrap_command_empty_string_no_toolchain() {
        let wrapped = wrap_command_with_toolchain("", None);
        assert_eq!(wrapped, "");
    }

    #[test]
    fn test_wrap_command_clippy() {
        let tc = ToolchainInfo::new("nightly", Some("2024-03-15".to_string()), "");
        let wrapped = wrap_command_with_toolchain("cargo clippy -- -D warnings", Some(&tc));
        assert_eq!(
            wrapped,
            "rustup run nightly-2024-03-15 cargo clippy -- -D warnings"
        );
    }

    #[test]
    fn test_wrap_command_rustfmt() {
        let tc = ToolchainInfo::new("nightly", None, "");
        let wrapped = wrap_command_with_toolchain("cargo fmt --check", Some(&tc));
        assert_eq!(wrapped, "rustup run nightly cargo fmt --check");
    }

    #[test]
    fn test_wrap_command_test_with_filter() {
        let tc = ToolchainInfo::new("stable", None, "");
        let wrapped = wrap_command_with_toolchain("cargo test test_name -- --nocapture", Some(&tc));
        assert_eq!(
            wrapped,
            "rustup run stable cargo test test_name -- --nocapture"
        );
    }

    #[test]
    fn test_rustup_toolchain_beta_with_date() {
        let tc = ToolchainInfo {
            channel: "beta".to_string(),
            date: Some("2024-02-01".to_string()),
            full_version: "".to_string(),
        };
        assert_eq!(tc.rustup_toolchain(), "beta-2024-02-01");
    }

    #[test]
    fn test_rustup_toolchain_specific_version() {
        let tc = ToolchainInfo {
            channel: "1.75.0".to_string(),
            date: None,
            full_version: "rustc 1.75.0".to_string(),
        };
        assert_eq!(tc.rustup_toolchain(), "1.75.0");
    }

    #[test]
    fn test_display_stable() {
        let tc = ToolchainInfo::new("stable", None, "");
        assert_eq!(tc.to_string(), "stable");
    }

    #[test]
    fn test_display_beta_with_date() {
        let tc = ToolchainInfo::new("beta", Some("2024-02-01".to_string()), "");
        assert_eq!(tc.to_string(), "beta-2024-02-01");
    }

    #[test]
    fn test_serialization_stable() {
        let tc = ToolchainInfo::new("stable", None, "rustc 1.75.0 (hash 2024-01-01)");
        let json = serde_json::to_string(&tc).unwrap();
        assert!(json.contains("\"channel\":\"stable\""));
        assert!(json.contains("\"date\":null"));
        let parsed: ToolchainInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(tc, parsed);
    }

    #[test]
    fn test_serialization_beta_with_date() {
        let tc = ToolchainInfo::new("beta", Some("2024-02-01".to_string()), "rustc 1.76.0-beta");
        let json = serde_json::to_string(&tc).unwrap();
        let parsed: ToolchainInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(tc, parsed);
        assert_eq!(parsed.date, Some("2024-02-01".to_string()));
    }

    #[test]
    fn test_toolchain_info_clone() {
        let tc = ToolchainInfo::new("nightly", Some("2024-01-15".to_string()), "rustc 1.76.0");
        let cloned = tc.clone();
        assert_eq!(tc, cloned);
        assert_eq!(tc.channel, cloned.channel);
        assert_eq!(tc.date, cloned.date);
        assert_eq!(tc.full_version, cloned.full_version);
    }

    #[test]
    fn test_toolchain_info_equality() {
        let tc1 = ToolchainInfo::new("nightly", Some("2024-01-15".to_string()), "rustc 1.76.0");
        let tc2 = ToolchainInfo::new("nightly", Some("2024-01-15".to_string()), "rustc 1.76.0");
        let tc3 = ToolchainInfo::new("nightly", Some("2024-01-16".to_string()), "rustc 1.76.0");
        assert_eq!(tc1, tc2);
        assert_ne!(tc1, tc3);
    }

    #[test]
    fn test_channel_detection_methods_consistency() {
        // Only one of the is_* methods should return true for a given toolchain
        for channel in &["stable", "beta", "nightly", "1.75.0", "custom"] {
            let tc = ToolchainInfo::new(*channel, None, "");
            let count = [tc.is_stable(), tc.is_beta(), tc.is_nightly()]
                .iter()
                .filter(|&&b| b)
                .count();
            // Should have at most one true (custom channels have none)
            assert!(count <= 1, "Multiple channel types for: {}", channel);
        }
    }
}
