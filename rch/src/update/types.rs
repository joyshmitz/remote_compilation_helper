//! Core types for the update system.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// Error types for update operations.
#[derive(Error, Debug)]
pub enum UpdateError {
    #[error("Failed to check for updates: {0}")]
    CheckFailed(String),

    #[error("Failed to download release: {0}")]
    DownloadFailed(String),

    #[error("Checksum verification failed: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("Installation failed: {0}")]
    InstallFailed(String),

    #[error("Update lock held by another process")]
    LockHeld,

    #[error("No backup available for rollback")]
    NoBackupAvailable,

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Invalid version format: {0}")]
    InvalidVersion(String),

    #[error("Unsupported platform: {0}")]
    UnsupportedPlatform(String),
}

/// Release channel for updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    #[default]
    Stable,
    Beta,
    Nightly,
}

impl FromStr for Channel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "stable" => Ok(Channel::Stable),
            "beta" => Ok(Channel::Beta),
            "nightly" => Ok(Channel::Nightly),
            _ => Err(format!("Unknown channel: {}", s)),
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Channel::Stable => write!(f, "stable"),
            Channel::Beta => write!(f, "beta"),
            Channel::Nightly => write!(f, "nightly"),
        }
    }
}

/// Semantic version with comparison support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub prerelease: Option<String>,
    raw: String,
}

impl Version {
    /// Parse a version string (e.g., "0.1.0", "v0.1.0", "0.1.0-beta.1").
    pub fn parse(s: &str) -> Result<Self, UpdateError> {
        let s = s.strip_prefix('v').unwrap_or(s);
        let raw = s.to_string();

        // Split prerelease suffix
        let (version_part, prerelease) = if let Some(idx) = s.find('-') {
            (&s[..idx], Some(s[idx + 1..].to_string()))
        } else {
            (s, None)
        };

        let parts: Vec<&str> = version_part.split('.').collect();
        if parts.len() < 2 || parts.len() > 3 {
            return Err(UpdateError::InvalidVersion(raw));
        }

        let major = parts[0]
            .parse()
            .map_err(|_| UpdateError::InvalidVersion(raw.clone()))?;
        let minor = parts[1]
            .parse()
            .map_err(|_| UpdateError::InvalidVersion(raw.clone()))?;
        let patch = if parts.len() > 2 {
            parts[2]
                .parse()
                .map_err(|_| UpdateError::InvalidVersion(raw.clone()))?
        } else {
            0
        };

        Ok(Self {
            major,
            minor,
            patch,
            prerelease,
            raw,
        })
    }

    /// Check if this is a prerelease version.
    #[allow(dead_code)]
    pub fn is_prerelease(&self) -> bool {
        self.prerelease.is_some()
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref pre) = self.prerelease {
            write!(f, "{}.{}.{}-{}", self.major, self.minor, self.patch, pre)
        } else {
            write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
        }
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major
            && self.minor == other.minor
            && self.patch == other.patch
            && self.prerelease == other.prerelease
    }
}

impl Eq for Version {}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.major.cmp(&other.major) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.minor.cmp(&other.minor) {
            Ordering::Equal => {}
            ord => return ord,
        }
        match self.patch.cmp(&other.patch) {
            Ordering::Equal => {}
            ord => return ord,
        }

        // Prerelease versions are less than release versions
        match (&self.prerelease, &other.prerelease) {
            (None, None) => Ordering::Equal,
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (Some(a), Some(b)) => a.cmp(b),
        }
    }
}

/// Information about a GitHub release.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub name: String,
    pub prerelease: bool,
    pub draft: bool,
    pub html_url: String,
    pub body: Option<String>,
    pub assets: Vec<ReleaseAsset>,
    pub published_at: Option<String>,
}

/// A release asset (downloadable file).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
    pub content_type: String,
}

#[allow(dead_code)]
impl ReleaseAsset {
    /// Check if this asset is for the current platform.
    pub fn is_for_current_platform(&self) -> bool {
        let target = current_target();
        self.name.contains(&target)
    }
}

/// Get the current target triple.
pub fn current_target() -> String {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;

    let arch_str = arch;

    let os_str = match os {
        "linux" => "unknown-linux-gnu",
        "macos" => "apple-darwin",
        "windows" => "pc-windows-msvc",
        _ => os,
    };

    format!("{}-{}", arch_str, os_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parse() {
        let v = Version::parse("0.1.0").unwrap();
        assert_eq!(v.major, 0);
        assert_eq!(v.minor, 1);
        assert_eq!(v.patch, 0);
        assert!(v.prerelease.is_none());
    }

    #[test]
    fn test_version_parse_with_v_prefix() {
        let v = Version::parse("v1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_version_parse_prerelease() {
        let v = Version::parse("0.2.0-beta.1").unwrap();
        assert_eq!(v.major, 0);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 0);
        assert_eq!(v.prerelease, Some("beta.1".to_string()));
        assert!(v.is_prerelease());
    }

    #[test]
    fn test_version_comparison() {
        let v1 = Version::parse("0.1.0").unwrap();
        let v2 = Version::parse("0.2.0").unwrap();
        let v3 = Version::parse("0.2.0-beta.1").unwrap();

        assert!(v1 < v2);
        assert!(v3 < v2); // prerelease < release
        assert!(v1 < v3);
    }

    #[test]
    fn test_version_display() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v.to_string(), "1.2.3");

        let v_pre = Version::parse("1.2.3-rc.1").unwrap();
        assert_eq!(v_pre.to_string(), "1.2.3-rc.1");
    }

    #[test]
    fn test_channel_from_str() {
        assert_eq!(Channel::from_str("stable").unwrap(), Channel::Stable);
        assert_eq!(Channel::from_str("BETA").unwrap(), Channel::Beta);
        assert_eq!(Channel::from_str("Nightly").unwrap(), Channel::Nightly);
        assert!(Channel::from_str("unknown").is_err());
    }

    #[test]
    fn test_current_target() {
        let target = current_target();
        assert!(!target.is_empty());
        // Should contain architecture
        assert!(target.contains("x86_64") || target.contains("aarch64") || target.contains("arm"));
    }
}
