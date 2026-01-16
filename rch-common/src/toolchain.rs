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
}
