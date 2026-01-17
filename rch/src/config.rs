//! Configuration loading for RCH.

use anyhow::{Context, Result};
use directories::ProjectDirs;
use rch_common::RchConfig;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::debug;

#[cfg(test)]
use std::sync::{Mutex, OnceLock};

#[cfg(test)]
fn test_config_override() -> &'static Mutex<Option<RchConfig>> {
    static OVERRIDE: OnceLock<Mutex<Option<RchConfig>>> = OnceLock::new();
    OVERRIDE.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
pub fn set_test_config_override(config: Option<RchConfig>) {
    *test_config_override().lock().unwrap() = config;
}

/// Get the user config directory.
pub fn config_dir() -> Option<PathBuf> {
    ProjectDirs::from("com", "rch", "rch").map(|dirs| dirs.config_dir().to_path_buf())
}

/// Load configuration from all sources.
pub fn load_config() -> Result<RchConfig> {
    #[cfg(test)]
    if let Some(config) = test_config_override().lock().unwrap().clone() {
        return Ok(config);
    }

    // Start with defaults
    let mut config = RchConfig::default();

    // Try to load user config
    if let Some(config_dir) = config_dir() {
        let config_path = config_dir.join("config.toml");
        if config_path.exists() {
            debug!("Loading user config from {:?}", config_path);
            let content = std::fs::read_to_string(&config_path)?;
            let user_config: RchConfig = toml::from_str(&content)?;
            config = user_config;
        }
    }

    // Try to load project config
    let project_config_path = PathBuf::from(".rch/config.toml");
    if project_config_path.exists() {
        debug!("Loading project config from {:?}", project_config_path);
        let content = std::fs::read_to_string(&project_config_path)?;
        let project_config: RchConfig = toml::from_str(&content)?;
        // Merge project config (project overrides user)
        config = merge_config(config, project_config);
    }

    // Apply environment variable overrides
    config = apply_env_overrides(config);

    Ok(config)
}

/// Merge two configs, with the second overriding the first.
///
/// Only fields in the overlay that differ from their default values
/// will override the base config. This allows partial config files
/// (e.g., project configs that only set one field) to work correctly
/// without clobbering the entire section from the user config.
fn merge_config(mut base: RchConfig, overlay: RchConfig) -> RchConfig {
    let default = RchConfig::default();

    // Merge general section
    merge_general(&mut base.general, &overlay.general, &default.general);

    // Merge compilation section
    merge_compilation(
        &mut base.compilation,
        &overlay.compilation,
        &default.compilation,
    );

    // Merge transfer section
    merge_transfer(&mut base.transfer, &overlay.transfer, &default.transfer);

    // Merge circuit breaker section
    merge_circuit(&mut base.circuit, &overlay.circuit, &default.circuit);

    base
}

/// Merge GeneralConfig fields.
fn merge_general(
    base: &mut rch_common::GeneralConfig,
    overlay: &rch_common::GeneralConfig,
    default: &rch_common::GeneralConfig,
) {
    // Only override if overlay differs from default
    if overlay.enabled != default.enabled {
        base.enabled = overlay.enabled;
    }
    if overlay.log_level != default.log_level {
        base.log_level.clone_from(&overlay.log_level);
    }
    if overlay.socket_path != default.socket_path {
        base.socket_path.clone_from(&overlay.socket_path);
    }
}

/// Merge CompilationConfig fields.
fn merge_compilation(
    base: &mut rch_common::CompilationConfig,
    overlay: &rch_common::CompilationConfig,
    default: &rch_common::CompilationConfig,
) {
    // Use float comparison with small epsilon for confidence_threshold
    const EPSILON: f64 = 0.0001;
    if (overlay.confidence_threshold - default.confidence_threshold).abs() > EPSILON {
        base.confidence_threshold = overlay.confidence_threshold;
    }
    if overlay.min_local_time_ms != default.min_local_time_ms {
        base.min_local_time_ms = overlay.min_local_time_ms;
    }
}

/// Merge TransferConfig fields.
fn merge_transfer(
    base: &mut rch_common::TransferConfig,
    overlay: &rch_common::TransferConfig,
    default: &rch_common::TransferConfig,
) {
    if overlay.compression_level != default.compression_level {
        base.compression_level = overlay.compression_level;
    }
    // For exclude_patterns, if overlay specifies any patterns that differ
    // from default, use the overlay's list entirely (append semantics
    // would be confusing)
    if overlay.exclude_patterns != default.exclude_patterns {
        base.exclude_patterns.clone_from(&overlay.exclude_patterns);
    }
}

/// Merge CircuitBreakerConfig fields.
fn merge_circuit(
    base: &mut rch_common::CircuitBreakerConfig,
    overlay: &rch_common::CircuitBreakerConfig,
    default: &rch_common::CircuitBreakerConfig,
) {
    if overlay.failure_threshold != default.failure_threshold {
        base.failure_threshold = overlay.failure_threshold;
    }
    if overlay.success_threshold != default.success_threshold {
        base.success_threshold = overlay.success_threshold;
    }
    const EPSILON: f64 = 0.0001;
    if (overlay.error_rate_threshold - default.error_rate_threshold).abs() > EPSILON {
        base.error_rate_threshold = overlay.error_rate_threshold;
    }
    if overlay.window_secs != default.window_secs {
        base.window_secs = overlay.window_secs;
    }
    if overlay.open_cooldown_secs != default.open_cooldown_secs {
        base.open_cooldown_secs = overlay.open_cooldown_secs;
    }
    if overlay.half_open_max_probes != default.half_open_max_probes {
        base.half_open_max_probes = overlay.half_open_max_probes;
    }
}

/// Apply environment variable overrides.
fn apply_env_overrides(mut config: RchConfig) -> RchConfig {
    if let Ok(val) = std::env::var("RCH_ENABLED") {
        config.general.enabled = val.parse().unwrap_or(true);
    }

    if let Ok(val) = std::env::var("RCH_LOG_LEVEL") {
        config.general.log_level = val;
    }

    if let Ok(val) = std::env::var("RCH_SOCKET_PATH") {
        config.general.socket_path = val;
    }

    if let Ok(val) = std::env::var("RCH_CONFIDENCE_THRESHOLD") {
        if let Ok(threshold) = val.parse() {
            config.compilation.confidence_threshold = threshold;
        }
    }

    if let Ok(val) = std::env::var("RCH_COMPRESSION") {
        if let Ok(level) = val.parse() {
            config.transfer.compression_level = level;
        }
    }

    config
}

/// Workers configuration file structure.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkersConfig {
    /// List of worker definitions.
    #[serde(default)]
    pub workers: Vec<WorkerEntry>,
}

/// Single worker entry in configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerEntry {
    /// Unique identifier for this worker.
    pub id: String,
    /// SSH hostname or IP address.
    pub host: String,
    /// SSH username.
    #[serde(default = "default_user")]
    pub user: String,
    /// Path to SSH private key.
    #[serde(default = "default_identity_file")]
    pub identity_file: String,
    /// Total CPU slots available on this worker.
    #[serde(default = "default_slots")]
    pub total_slots: u32,
    /// Priority for worker selection (higher = preferred).
    #[serde(default = "default_priority")]
    pub priority: u32,
    /// Optional tags for filtering.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Whether this worker is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_user() -> String {
    "ubuntu".to_string()
}

fn default_identity_file() -> String {
    "~/.ssh/id_rsa".to_string()
}

fn default_slots() -> u32 {
    8
}

fn default_priority() -> u32 {
    100
}

fn default_true() -> bool {
    true
}

/// Load workers configuration from file.
#[allow(dead_code)] // Scaffolded for future CLI usage
pub fn load_workers_config(path: Option<&Path>) -> Result<WorkersConfig> {
    let config_path = match path {
        Some(p) => p.to_path_buf(),
        None => {
            let dir = config_dir().context("Could not determine config directory")?;
            dir.join("workers.toml")
        }
    };

    if !config_path.exists() {
        debug!("Workers config not found at {:?}", config_path);
        return Ok(WorkersConfig::default());
    }

    debug!("Loading workers config from {:?}", config_path);
    let contents = std::fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read workers config from {:?}", config_path))?;

    let config: WorkersConfig = toml::from_str(&contents)
        .with_context(|| format!("Failed to parse workers config from {:?}", config_path))?;

    debug!("Loaded {} worker definitions", config.workers.len());
    Ok(config)
}

/// Generate an example project config.
#[allow(dead_code)] // Used by future CLI scaffolding
pub fn example_project_config() -> String {
    r#"# RCH Project Configuration
# Place this file at .rch/config.toml in your project root

[general]
enabled = true
# Uncomment to use a custom socket path
# socket_path = "/tmp/rch.sock"

[compilation]
# Minimum confidence score to intercept (0.0-1.0)
confidence_threshold = 0.85
# Skip interception if estimated local time < this (ms)
min_local_time_ms = 2000

[transfer]
# zstd compression level (1-19)
compression_level = 3
# Additional patterns to exclude from transfer
exclude_patterns = [
    "target/",
    ".git/objects/",
    "node_modules/",
    "*.rlib",
    "*.rmeta",
]
"#
    .to_string()
}

/// Generate an example workers config.
#[allow(dead_code)] // Used by future CLI scaffolding
pub fn example_workers_config() -> String {
    r#"# RCH Workers Configuration
# Place this file at ~/.config/rch/workers.toml

[[workers]]
id = "worker1"
host = "192.168.1.100"
user = "ubuntu"
identity_file = "~/.ssh/id_rsa"
total_slots = 16
priority = 100
tags = ["rust", "fast"]
enabled = true

[[workers]]
id = "worker2"
host = "192.168.1.101"
user = "ubuntu"
identity_file = "~/.ssh/id_rsa"
total_slots = 8
priority = 80
tags = ["rust"]
enabled = true
"#
    .to_string()
}

/// Validate a config file.
#[allow(dead_code)] // Used by future CLI scaffolding
pub fn validate_config(path: &Path) -> Result<()> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config from {:?}", path))?;

    // Try parsing as RchConfig first
    if toml::from_str::<RchConfig>(&contents).is_ok() {
        return Ok(());
    }

    // Try parsing as WorkersConfig
    if toml::from_str::<WorkersConfig>(&contents).is_ok() {
        return Ok(());
    }

    anyhow::bail!("Config file is not valid RCH or workers configuration")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::info;

    #[test]
    fn test_default_config() {
        info!("TEST: test_default_config");
        let config = RchConfig::default();
        info!(
            "INPUT: default config - enabled={}, log_level={}, confidence={}",
            config.general.enabled,
            config.general.log_level,
            config.compilation.confidence_threshold
        );
        assert!(config.general.enabled);
        assert_eq!(config.general.log_level, "info");
        assert_eq!(config.compilation.confidence_threshold, 0.85);
        info!("PASS: Default config values correct");
    }

    #[test]
    fn test_example_project_config_valid() {
        info!("TEST: test_example_project_config_valid");
        let toml_str = example_project_config();
        let _: RchConfig = toml::from_str(&toml_str).expect("Example project config should parse");
        info!("PASS: Example project config parses successfully");
    }

    #[test]
    fn test_example_workers_config_valid() {
        info!("TEST: test_example_workers_config_valid");
        let toml_str = example_workers_config();
        let _: WorkersConfig =
            toml::from_str(&toml_str).expect("Example workers config should parse");
        info!("PASS: Example workers config parses successfully");
    }

    // ========================================================================
    // Merge Config Tests - Issue remote_compilation_helper-f0t.1
    // ========================================================================

    #[test]
    fn test_merge_preserves_base_fields() {
        info!("TEST: test_merge_preserves_base_fields");

        // Base config with non-default values
        let mut base = RchConfig::default();
        base.compilation.confidence_threshold = 0.90;
        base.compilation.min_local_time_ms = 5000;
        base.general.log_level = "debug".to_string();

        info!(
            "INPUT base: confidence={}, min_local_time_ms={}, log_level={}",
            base.compilation.confidence_threshold,
            base.compilation.min_local_time_ms,
            base.general.log_level
        );

        // Overlay that only changes confidence_threshold (leaves others at default)
        let overlay_toml = r#"
            [compilation]
            confidence_threshold = 0.70
        "#;
        let overlay: RchConfig = toml::from_str(overlay_toml).expect("Parse overlay");

        info!(
            "INPUT overlay: confidence={}, min_local_time_ms={} (default)",
            overlay.compilation.confidence_threshold, overlay.compilation.min_local_time_ms
        );

        let merged = merge_config(base, overlay);

        info!(
            "RESULT: confidence={}, min_local_time_ms={}, log_level={}",
            merged.compilation.confidence_threshold,
            merged.compilation.min_local_time_ms,
            merged.general.log_level
        );

        // Confidence should be overridden (0.70 != 0.85 default)
        assert!(
            (merged.compilation.confidence_threshold - 0.70).abs() < 0.0001,
            "Expected confidence_threshold=0.70, got {}",
            merged.compilation.confidence_threshold
        );

        // min_local_time_ms should be preserved from base (overlay has default 2000)
        assert_eq!(
            merged.compilation.min_local_time_ms, 5000,
            "Expected min_local_time_ms=5000, got {}",
            merged.compilation.min_local_time_ms
        );

        // log_level should be preserved from base (overlay has default "info")
        assert_eq!(
            merged.general.log_level, "debug",
            "Expected log_level=debug, got {}",
            merged.general.log_level
        );

        info!("PASS: Base fields preserved when overlay uses defaults");
    }

    #[test]
    fn test_merge_overlay_wins() {
        info!("TEST: test_merge_overlay_wins");

        let base = RchConfig::default();
        info!(
            "INPUT base (default): confidence={}, log_level={}",
            base.compilation.confidence_threshold, base.general.log_level
        );

        // Overlay with explicit non-default values
        let overlay_toml = r#"
            [general]
            log_level = "warn"
            socket_path = "/custom/rch.sock"

            [compilation]
            confidence_threshold = 0.95
            min_local_time_ms = 3000
        "#;
        let overlay: RchConfig = toml::from_str(overlay_toml).expect("Parse overlay");
        info!(
            "INPUT overlay: confidence={}, log_level={}, socket_path={}",
            overlay.compilation.confidence_threshold,
            overlay.general.log_level,
            overlay.general.socket_path
        );

        let merged = merge_config(base, overlay);

        info!(
            "RESULT: confidence={}, log_level={}, socket_path={}, min_local_time_ms={}",
            merged.compilation.confidence_threshold,
            merged.general.log_level,
            merged.general.socket_path,
            merged.compilation.min_local_time_ms
        );

        // All overlay fields should win
        assert!(
            (merged.compilation.confidence_threshold - 0.95).abs() < 0.0001,
            "Expected confidence_threshold=0.95"
        );
        assert_eq!(merged.general.log_level, "warn");
        assert_eq!(merged.general.socket_path, "/custom/rch.sock");
        assert_eq!(merged.compilation.min_local_time_ms, 3000);

        info!("PASS: Overlay non-default values override base");
    }

    #[test]
    fn test_merge_nested_sections() {
        info!("TEST: test_merge_nested_sections");

        // Base with non-default circuit breaker settings
        let mut base = RchConfig::default();
        base.circuit.failure_threshold = 10;
        base.circuit.window_secs = 120;

        info!(
            "INPUT base: failure_threshold={}, window_secs={}",
            base.circuit.failure_threshold, base.circuit.window_secs
        );

        // Overlay only changes one circuit field
        let overlay_toml = r#"
            [circuit]
            success_threshold = 5
        "#;
        let overlay: RchConfig = toml::from_str(overlay_toml).expect("Parse overlay");

        info!(
            "INPUT overlay: success_threshold={}, failure_threshold={} (default)",
            overlay.circuit.success_threshold, overlay.circuit.failure_threshold
        );

        let merged = merge_config(base, overlay);

        info!(
            "RESULT: failure_threshold={}, window_secs={}, success_threshold={}",
            merged.circuit.failure_threshold,
            merged.circuit.window_secs,
            merged.circuit.success_threshold
        );

        // Base values should be preserved
        assert_eq!(
            merged.circuit.failure_threshold, 10,
            "failure_threshold should be preserved from base"
        );
        assert_eq!(
            merged.circuit.window_secs, 120,
            "window_secs should be preserved from base"
        );

        // Overlay value should be applied
        assert_eq!(
            merged.circuit.success_threshold, 5,
            "success_threshold should be from overlay"
        );

        info!("PASS: Nested section fields merge correctly");
    }

    #[test]
    fn test_merge_empty_overlay() {
        info!("TEST: test_merge_empty_overlay");

        // Base with custom values
        let mut base = RchConfig::default();
        base.compilation.confidence_threshold = 0.99;
        base.general.log_level = "trace".to_string();
        base.transfer.compression_level = 10;

        info!(
            "INPUT base: confidence={}, log_level={}, compression={}",
            base.compilation.confidence_threshold,
            base.general.log_level,
            base.transfer.compression_level
        );

        // Empty overlay (all defaults)
        let overlay = RchConfig::default();
        info!("INPUT overlay: all defaults");

        let merged = merge_config(base.clone(), overlay);

        info!(
            "RESULT: confidence={}, log_level={}, compression={}",
            merged.compilation.confidence_threshold,
            merged.general.log_level,
            merged.transfer.compression_level
        );

        // Everything should remain from base since overlay is all defaults
        assert!(
            (merged.compilation.confidence_threshold - 0.99).abs() < 0.0001,
            "confidence_threshold should be preserved"
        );
        assert_eq!(
            merged.general.log_level, "trace",
            "log_level should be preserved"
        );
        assert_eq!(
            merged.transfer.compression_level, 10,
            "compression_level should be preserved"
        );

        info!("PASS: Empty overlay is no-op");
    }

    #[test]
    fn test_merge_transfer_exclude_patterns() {
        info!("TEST: test_merge_transfer_exclude_patterns");

        let base = RchConfig::default();
        info!(
            "INPUT base: exclude_patterns count={}",
            base.transfer.exclude_patterns.len()
        );

        // Overlay with custom exclude patterns
        let overlay_toml = r#"
            [transfer]
            exclude_patterns = [
                "target/",
                "custom_exclude/",
                "*.log"
            ]
        "#;
        let overlay: RchConfig = toml::from_str(overlay_toml).expect("Parse overlay");
        info!(
            "INPUT overlay: exclude_patterns={:?}",
            overlay.transfer.exclude_patterns
        );

        let merged = merge_config(base, overlay);

        info!(
            "RESULT: exclude_patterns={:?}",
            merged.transfer.exclude_patterns
        );

        // Custom patterns should replace defaults (not merge)
        assert_eq!(merged.transfer.exclude_patterns.len(), 3);
        assert!(
            merged
                .transfer
                .exclude_patterns
                .contains(&"target/".to_string())
        );
        assert!(
            merged
                .transfer
                .exclude_patterns
                .contains(&"custom_exclude/".to_string())
        );
        assert!(
            merged
                .transfer
                .exclude_patterns
                .contains(&"*.log".to_string())
        );

        info!("PASS: Custom exclude_patterns replace defaults");
    }

    #[test]
    fn test_merge_boolean_enabled_field() {
        info!("TEST: test_merge_boolean_enabled_field");

        // Base has enabled=true (default)
        let base = RchConfig::default();
        assert!(base.general.enabled, "Base should have enabled=true");

        // Overlay explicitly disables
        let overlay_toml = r#"
            [general]
            enabled = false
        "#;
        let overlay: RchConfig = toml::from_str(overlay_toml).expect("Parse overlay");

        info!(
            "INPUT base: enabled={}, INPUT overlay: enabled={}",
            base.general.enabled, overlay.general.enabled
        );

        let merged = merge_config(base, overlay);

        info!("RESULT: enabled={}", merged.general.enabled);

        // enabled=false differs from default (true), so it should be applied
        assert!(
            !merged.general.enabled,
            "enabled=false should be applied from overlay"
        );

        info!("PASS: Boolean enabled=false overrides enabled=true");
    }

    #[test]
    fn test_merge_real_world_project_config() {
        info!("TEST: test_merge_real_world_project_config");

        // User config with customizations
        let user_toml = r#"
            [general]
            log_level = "debug"

            [compilation]
            confidence_threshold = 0.90
            min_local_time_ms = 5000

            [transfer]
            compression_level = 6
        "#;
        let base: RchConfig = toml::from_str(user_toml).expect("Parse user config");

        info!(
            "INPUT user: log_level={}, confidence={}, min_local_time={}, compression={}",
            base.general.log_level,
            base.compilation.confidence_threshold,
            base.compilation.min_local_time_ms,
            base.transfer.compression_level
        );

        // Project config only overrides confidence
        let project_toml = r#"
            [compilation]
            confidence_threshold = 0.70
        "#;
        let overlay: RchConfig = toml::from_str(project_toml).expect("Parse project config");

        info!(
            "INPUT project: confidence={} (others are defaults)",
            overlay.compilation.confidence_threshold
        );

        let merged = merge_config(base, overlay);

        info!(
            "RESULT: log_level={}, confidence={}, min_local_time={}, compression={}",
            merged.general.log_level,
            merged.compilation.confidence_threshold,
            merged.compilation.min_local_time_ms,
            merged.transfer.compression_level
        );

        // Project config only changed confidence_threshold
        assert!(
            (merged.compilation.confidence_threshold - 0.70).abs() < 0.0001,
            "confidence_threshold should be from project"
        );

        // All other user values preserved
        assert_eq!(
            merged.general.log_level, "debug",
            "log_level from user preserved"
        );
        assert_eq!(
            merged.compilation.min_local_time_ms, 5000,
            "min_local_time_ms from user preserved"
        );
        assert_eq!(
            merged.transfer.compression_level, 6,
            "compression_level from user preserved"
        );

        info!("PASS: Real-world project config scenario works correctly");
    }
}
