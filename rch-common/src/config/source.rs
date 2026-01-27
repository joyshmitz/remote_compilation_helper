//! Configuration source tracking.
//!
//! Tracks where each configuration value came from for debugging.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// Where a configuration value originated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigSource {
    /// Built-in default value.
    Default,
    /// User-level config file (~/.config/rch/config.toml).
    UserConfig,
    /// Project-level config file (.rch/config.toml).
    ProjectConfig,
    /// Loaded from .env or .rch.env file.
    DotEnv,
    /// From a profile (dev/prod/test).
    Profile,
    /// Environment variable.
    Environment,
    /// Command-line argument (highest precedence).
    CommandLine,
}

impl ConfigSource {
    /// Get the precedence level (higher = takes priority).
    pub fn precedence(&self) -> u8 {
        match self {
            ConfigSource::Default => 0,
            ConfigSource::UserConfig => 1,
            ConfigSource::ProjectConfig => 2,
            ConfigSource::DotEnv => 3,
            ConfigSource::Profile => 4,
            ConfigSource::Environment => 5,
            ConfigSource::CommandLine => 6,
        }
    }

    /// Get a human-readable name for this source.
    pub fn display_name(&self) -> &'static str {
        match self {
            ConfigSource::Default => "default",
            ConfigSource::UserConfig => "user config",
            ConfigSource::ProjectConfig => "project config",
            ConfigSource::DotEnv => ".env file",
            ConfigSource::Profile => "profile",
            ConfigSource::Environment => "environment",
            ConfigSource::CommandLine => "command line",
        }
    }
}

impl fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Detailed source for a specific configuration value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigValueSource {
    /// Built-in default value.
    Default,
    /// User-level config file (~/.config/rch/config.toml).
    UserConfig(PathBuf),
    /// Project-level config file (.rch/config.toml).
    ProjectConfig(PathBuf),
    /// Environment variable name.
    EnvVar(String),
}

impl ConfigValueSource {
    /// Render a human-friendly label for CLI output.
    pub fn label(&self) -> String {
        match self {
            ConfigValueSource::Default => "default".to_string(),
            ConfigValueSource::UserConfig(path) => {
                format!("user:{}", path.display())
            }
            ConfigValueSource::ProjectConfig(path) => {
                format!("project:{}", path.display())
            }
            ConfigValueSource::EnvVar(name) => format!("env:{}", name),
        }
    }
}

impl fmt::Display for ConfigValueSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// A configuration value with its source.
#[derive(Debug, Clone)]
pub struct Sourced<T> {
    /// The actual value.
    pub value: T,
    /// Where this value came from.
    pub source: ConfigSource,
    /// Optional environment variable name if from environment.
    pub env_var: Option<String>,
}

impl<T> Sourced<T> {
    /// Create a new sourced value.
    pub fn new(value: T, source: ConfigSource) -> Self {
        Self {
            value,
            source,
            env_var: None,
        }
    }

    /// Create a sourced value from an environment variable.
    pub fn from_env(value: T, var_name: impl Into<String>) -> Self {
        Self {
            value,
            source: ConfigSource::Environment,
            env_var: Some(var_name.into()),
        }
    }

    /// Create a default sourced value.
    pub fn default_value(value: T) -> Self {
        Self::new(value, ConfigSource::Default)
    }

    /// Map the value while preserving source.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Sourced<U> {
        Sourced {
            value: f(self.value),
            source: self.source,
            env_var: self.env_var,
        }
    }

    /// Merge with another sourced value, taking the higher precedence one.
    pub fn merge(self, other: Self) -> Self {
        if other.source.precedence() >= self.source.precedence() {
            other
        } else {
            self
        }
    }
}

impl<T: Default> Default for Sourced<T> {
    fn default() -> Self {
        Self::default_value(T::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_precedence() {
        assert!(ConfigSource::Environment.precedence() > ConfigSource::UserConfig.precedence());
        assert!(ConfigSource::CommandLine.precedence() > ConfigSource::Environment.precedence());
        assert!(ConfigSource::Default.precedence() < ConfigSource::ProjectConfig.precedence());
    }

    #[test]
    fn test_sourced_merge() {
        let default = Sourced::new(10, ConfigSource::Default);
        let env = Sourced::new(20, ConfigSource::Environment);

        let merged = default.merge(env);
        assert_eq!(merged.value, 20);
        assert_eq!(merged.source, ConfigSource::Environment);
    }

    #[test]
    fn test_sourced_map() {
        let sourced = Sourced::from_env("42".to_string(), "MY_VAR");
        let mapped = sourced.map(|s| s.parse::<i32>().unwrap());

        assert_eq!(mapped.value, 42);
        assert_eq!(mapped.source, ConfigSource::Environment);
        assert_eq!(mapped.env_var.as_deref(), Some("MY_VAR"));
    }

    // ========================
    // ConfigSource tests
    // ========================

    #[test]
    fn test_config_source_precedence_order() {
        let sources = [
            ConfigSource::Default,
            ConfigSource::UserConfig,
            ConfigSource::ProjectConfig,
            ConfigSource::DotEnv,
            ConfigSource::Profile,
            ConfigSource::Environment,
            ConfigSource::CommandLine,
        ];

        for i in 0..sources.len() - 1 {
            assert!(
                sources[i].precedence() < sources[i + 1].precedence(),
                "{:?} should have lower precedence than {:?}",
                sources[i],
                sources[i + 1]
            );
        }
    }

    #[test]
    fn test_config_source_display_name_all_variants() {
        assert_eq!(ConfigSource::Default.display_name(), "default");
        assert_eq!(ConfigSource::UserConfig.display_name(), "user config");
        assert_eq!(ConfigSource::ProjectConfig.display_name(), "project config");
        assert_eq!(ConfigSource::DotEnv.display_name(), ".env file");
        assert_eq!(ConfigSource::Profile.display_name(), "profile");
        assert_eq!(ConfigSource::Environment.display_name(), "environment");
        assert_eq!(ConfigSource::CommandLine.display_name(), "command line");
    }

    #[test]
    fn test_config_source_display_trait() {
        assert_eq!(format!("{}", ConfigSource::Default), "default");
        assert_eq!(format!("{}", ConfigSource::CommandLine), "command line");
    }

    #[test]
    fn test_config_source_serialization() {
        let source = ConfigSource::Environment;
        let json = serde_json::to_string(&source).unwrap();
        assert_eq!(json, "\"environment\"");

        let deserialized: ConfigSource = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ConfigSource::Environment);
    }

    #[test]
    fn test_config_source_all_variants_serialize_roundtrip() {
        let sources = [
            ConfigSource::Default,
            ConfigSource::UserConfig,
            ConfigSource::ProjectConfig,
            ConfigSource::DotEnv,
            ConfigSource::Profile,
            ConfigSource::Environment,
            ConfigSource::CommandLine,
        ];

        for source in sources {
            let json = serde_json::to_string(&source).unwrap();
            let restored: ConfigSource = serde_json::from_str(&json).unwrap();
            assert_eq!(restored, source);
        }
    }

    #[test]
    fn test_config_source_copy_trait() {
        let source = ConfigSource::Environment;
        let copied = source;
        assert_eq!(source, copied);
    }

    // ========================
    // ConfigValueSource tests
    // ========================

    #[test]
    fn test_config_value_source_default_label() {
        let source = ConfigValueSource::Default;
        assert_eq!(source.label(), "default");
    }

    #[test]
    fn test_config_value_source_user_config_label() {
        let source =
            ConfigValueSource::UserConfig(PathBuf::from("/home/user/.config/rch/config.toml"));
        assert!(source.label().starts_with("user:"));
        assert!(source.label().contains("config.toml"));
    }

    #[test]
    fn test_config_value_source_project_config_label() {
        let source = ConfigValueSource::ProjectConfig(PathBuf::from("/project/.rch/config.toml"));
        assert!(source.label().starts_with("project:"));
        assert!(source.label().contains("config.toml"));
    }

    #[test]
    fn test_config_value_source_env_var_label() {
        let source = ConfigValueSource::EnvVar("RCH_WORKERS".to_string());
        assert_eq!(source.label(), "env:RCH_WORKERS");
    }

    #[test]
    fn test_config_value_source_display_trait() {
        let source = ConfigValueSource::EnvVar("MY_VAR".to_string());
        assert_eq!(format!("{}", source), "env:MY_VAR");
    }

    #[test]
    fn test_config_value_source_serialization_default() {
        let source = ConfigValueSource::Default;
        let json = serde_json::to_string(&source).unwrap();
        let restored: ConfigValueSource = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, ConfigValueSource::Default);
    }

    #[test]
    fn test_config_value_source_serialization_env_var() {
        let source = ConfigValueSource::EnvVar("TEST_VAR".to_string());
        let json = serde_json::to_string(&source).unwrap();
        let restored: ConfigValueSource = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, source);
    }

    #[test]
    fn test_config_value_source_serialization_user_config() {
        let source = ConfigValueSource::UserConfig(PathBuf::from("/test/path"));
        let json = serde_json::to_string(&source).unwrap();
        let restored: ConfigValueSource = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, source);
    }

    #[test]
    fn test_config_value_source_equality() {
        let source1 = ConfigValueSource::EnvVar("VAR1".to_string());
        let source2 = ConfigValueSource::EnvVar("VAR1".to_string());
        let source3 = ConfigValueSource::EnvVar("VAR2".to_string());

        assert_eq!(source1, source2);
        assert_ne!(source1, source3);
    }

    // ========================
    // Sourced<T> tests
    // ========================

    #[test]
    fn test_sourced_new() {
        let sourced = Sourced::new(42, ConfigSource::UserConfig);
        assert_eq!(sourced.value, 42);
        assert_eq!(sourced.source, ConfigSource::UserConfig);
        assert!(sourced.env_var.is_none());
    }

    #[test]
    fn test_sourced_from_env() {
        let sourced = Sourced::from_env("value".to_string(), "MY_ENV_VAR");
        assert_eq!(sourced.value, "value");
        assert_eq!(sourced.source, ConfigSource::Environment);
        assert_eq!(sourced.env_var.as_deref(), Some("MY_ENV_VAR"));
    }

    #[test]
    fn test_sourced_default_value() {
        let sourced = Sourced::default_value(100);
        assert_eq!(sourced.value, 100);
        assert_eq!(sourced.source, ConfigSource::Default);
        assert!(sourced.env_var.is_none());
    }

    #[test]
    fn test_sourced_default_trait() {
        let sourced: Sourced<i32> = Sourced::default();
        assert_eq!(sourced.value, 0);
        assert_eq!(sourced.source, ConfigSource::Default);
    }

    #[test]
    fn test_sourced_default_trait_string() {
        let sourced: Sourced<String> = Sourced::default();
        assert_eq!(sourced.value, "");
        assert_eq!(sourced.source, ConfigSource::Default);
    }

    #[test]
    fn test_sourced_merge_higher_precedence_wins() {
        let lower = Sourced::new(10, ConfigSource::Default);
        let higher = Sourced::new(20, ConfigSource::CommandLine);

        let result = lower.merge(higher);
        assert_eq!(result.value, 20);
        assert_eq!(result.source, ConfigSource::CommandLine);
    }

    #[test]
    fn test_sourced_merge_equal_precedence_takes_other() {
        let first = Sourced::new(10, ConfigSource::Environment);
        let second = Sourced::new(20, ConfigSource::Environment);

        let result = first.merge(second);
        assert_eq!(result.value, 20);
    }

    #[test]
    fn test_sourced_merge_lower_precedence_keeps_self() {
        let higher = Sourced::new(10, ConfigSource::CommandLine);
        let lower = Sourced::new(20, ConfigSource::Default);

        let result = higher.merge(lower);
        assert_eq!(result.value, 10);
        assert_eq!(result.source, ConfigSource::CommandLine);
    }

    #[test]
    fn test_sourced_merge_preserves_env_var() {
        let default = Sourced::new(10, ConfigSource::Default);
        let env = Sourced::from_env(20, "MY_VAR");

        let result = default.merge(env);
        assert_eq!(result.env_var.as_deref(), Some("MY_VAR"));
    }

    #[test]
    fn test_sourced_map_preserves_source() {
        let sourced = Sourced::new(42, ConfigSource::ProjectConfig);
        let mapped = sourced.map(|v| v.to_string());

        assert_eq!(mapped.value, "42");
        assert_eq!(mapped.source, ConfigSource::ProjectConfig);
    }

    #[test]
    fn test_sourced_map_preserves_env_var() {
        let sourced = Sourced::from_env(42, "NUMBER");
        let mapped = sourced.map(|v| v * 2);

        assert_eq!(mapped.value, 84);
        assert_eq!(mapped.env_var.as_deref(), Some("NUMBER"));
    }

    #[test]
    fn test_sourced_map_chain() {
        let sourced = Sourced::new("hello".to_string(), ConfigSource::UserConfig);
        let mapped = sourced.map(|s| s.len()).map(|n| n * 2);

        assert_eq!(mapped.value, 10);
        assert_eq!(mapped.source, ConfigSource::UserConfig);
    }

    #[test]
    fn test_sourced_with_complex_type() {
        #[derive(Debug, Clone, PartialEq)]
        struct Config {
            timeout: u64,
            retries: u32,
        }

        let config = Config {
            timeout: 30,
            retries: 3,
        };
        let sourced = Sourced::new(config.clone(), ConfigSource::Environment);

        assert_eq!(sourced.value.timeout, 30);
        assert_eq!(sourced.value.retries, 3);

        let mapped = sourced.map(|c| c.timeout);
        assert_eq!(mapped.value, 30);
    }
}
