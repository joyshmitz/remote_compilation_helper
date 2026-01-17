//! Agent type definitions.
//!
//! Defines the core types for AI coding agent detection and configuration.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// Supported AI coding agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentKind {
    /// Claude Code (Anthropic's official CLI)
    ClaudeCode,
    /// Gemini CLI (Google's CLI)
    GeminiCli,
    /// Codex CLI (OpenAI)
    CodexCli,
    /// Cursor (AI-powered IDE)
    Cursor,
    /// Continue.dev (VS Code extension)
    ContinueDev,
    /// Windsurf (Codeium)
    Windsurf,
    /// Aider (terminal-based assistant)
    Aider,
    /// Cline (VS Code extension)
    Cline,
}

impl AgentKind {
    /// All known agent kinds.
    pub const ALL: &'static [AgentKind] = &[
        AgentKind::ClaudeCode,
        AgentKind::GeminiCli,
        AgentKind::CodexCli,
        AgentKind::Cursor,
        AgentKind::ContinueDev,
        AgentKind::Windsurf,
        AgentKind::Aider,
        AgentKind::Cline,
    ];

    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            AgentKind::ClaudeCode => "Claude Code",
            AgentKind::GeminiCli => "Gemini CLI",
            AgentKind::CodexCli => "Codex CLI",
            AgentKind::Cursor => "Cursor",
            AgentKind::ContinueDev => "Continue.dev",
            AgentKind::Windsurf => "Windsurf",
            AgentKind::Aider => "Aider",
            AgentKind::Cline => "Cline",
        }
    }

    /// CLI identifier (kebab-case).
    pub fn id(&self) -> &'static str {
        match self {
            AgentKind::ClaudeCode => "claude-code",
            AgentKind::GeminiCli => "gemini-cli",
            AgentKind::CodexCli => "codex-cli",
            AgentKind::Cursor => "cursor",
            AgentKind::ContinueDev => "continue-dev",
            AgentKind::Windsurf => "windsurf",
            AgentKind::Aider => "aider",
            AgentKind::Cline => "cline",
        }
    }

    /// Parse from CLI identifier.
    pub fn from_id(id: &str) -> Option<AgentKind> {
        match id.to_lowercase().as_str() {
            "claude-code" | "claude" => Some(AgentKind::ClaudeCode),
            "gemini-cli" | "gemini" => Some(AgentKind::GeminiCli),
            "codex-cli" | "codex" => Some(AgentKind::CodexCli),
            "cursor" => Some(AgentKind::Cursor),
            "continue-dev" | "continue" => Some(AgentKind::ContinueDev),
            "windsurf" => Some(AgentKind::Windsurf),
            "aider" => Some(AgentKind::Aider),
            "cline" => Some(AgentKind::Cline),
            _ => None,
        }
    }

    /// Level of hook support for this agent.
    pub fn hook_support(&self) -> HookSupport {
        match self {
            AgentKind::ClaudeCode => HookSupport::Full,
            AgentKind::GeminiCli => HookSupport::Full,
            AgentKind::CodexCli => HookSupport::Full,
            AgentKind::ContinueDev => HookSupport::Partial,
            AgentKind::Cursor => HookSupport::DetectionOnly,
            AgentKind::Windsurf => HookSupport::DetectionOnly,
            AgentKind::Aider => HookSupport::DetectionOnly,
            AgentKind::Cline => HookSupport::DetectionOnly,
        }
    }

    /// Default configuration path for this agent.
    pub fn default_config_path(&self) -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        match self {
            AgentKind::ClaudeCode => Some(home.join(".claude").join("settings.json")),
            AgentKind::GeminiCli => Some(home.join(".gemini").join("settings.json")),
            AgentKind::CodexCli => Some(home.join(".codex").join("config.toml")),
            AgentKind::ContinueDev => Some(home.join(".continue").join("config.json")),
            AgentKind::Cursor => Some(home.join(".cursor")),
            AgentKind::Windsurf => Some(home.join(".windsurf")),
            AgentKind::Aider => Some(home.join(".aider.conf.yml")),
            AgentKind::Cline => None, // VS Code extension, managed differently
        }
    }
}

impl fmt::Display for AgentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Level of hook support for an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookSupport {
    /// Full PreToolUse hook support with JSON protocol.
    Full,
    /// Partial support (some hook functionality).
    Partial,
    /// Can detect but cannot install hooks.
    DetectionOnly,
    /// No hook support at all.
    None,
}

impl HookSupport {
    /// Human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            HookSupport::Full => "Full hook support (PreToolUse)",
            HookSupport::Partial => "Partial hook support",
            HookSupport::DetectionOnly => "Detection only",
            HookSupport::None => "Not supported",
        }
    }

    /// Whether hooks can be installed for this support level.
    pub fn can_install_hook(&self) -> bool {
        matches!(self, HookSupport::Full | HookSupport::Partial)
    }
}

impl fmt::Display for HookSupport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// Information about a detected AI coding agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedAgent {
    /// The type of agent.
    pub kind: AgentKind,
    /// Version string if detected.
    pub version: Option<String>,
    /// Path to the agent's configuration file/directory.
    pub config_path: Option<PathBuf>,
    /// Level of hook support.
    pub hook_support: HookSupport,
    /// How the agent was detected.
    pub detection_method: DetectionMethod,
}

impl DetectedAgent {
    /// Create a new detected agent.
    pub fn new(kind: AgentKind) -> Self {
        Self {
            kind,
            version: None,
            config_path: kind.default_config_path(),
            hook_support: kind.hook_support(),
            detection_method: DetectionMethod::NotDetected,
        }
    }

    /// Set the version.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Set the config path.
    pub fn with_config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = Some(path.into());
        self
    }

    /// Set the detection method.
    pub fn with_detection_method(mut self, method: DetectionMethod) -> Self {
        self.detection_method = method;
        self
    }
}

/// How an agent was detected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionMethod {
    /// Found via PATH lookup (which command).
    PathLookup,
    /// Found via config file existence.
    ConfigFile,
    /// Found via process detection.
    ProcessDetection,
    /// Not actually detected (placeholder).
    NotDetected,
}

impl fmt::Display for DetectionMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DetectionMethod::PathLookup => write!(f, "PATH lookup"),
            DetectionMethod::ConfigFile => write!(f, "config file"),
            DetectionMethod::ProcessDetection => write!(f, "process detection"),
            DetectionMethod::NotDetected => write!(f, "not detected"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_kind_all() {
        assert_eq!(AgentKind::ALL.len(), 8);
    }

    #[test]
    fn test_agent_kind_from_id() {
        assert_eq!(
            AgentKind::from_id("claude-code"),
            Some(AgentKind::ClaudeCode)
        );
        assert_eq!(AgentKind::from_id("claude"), Some(AgentKind::ClaudeCode));
        assert_eq!(
            AgentKind::from_id("CLAUDE-CODE"),
            Some(AgentKind::ClaudeCode)
        );
        assert_eq!(AgentKind::from_id("unknown"), None);
    }

    #[test]
    fn test_agent_kind_roundtrip() {
        for kind in AgentKind::ALL {
            let id = kind.id();
            assert_eq!(AgentKind::from_id(id), Some(*kind));
        }
    }

    #[test]
    fn test_hook_support_can_install() {
        assert!(HookSupport::Full.can_install_hook());
        assert!(HookSupport::Partial.can_install_hook());
        assert!(!HookSupport::DetectionOnly.can_install_hook());
        assert!(!HookSupport::None.can_install_hook());
    }

    #[test]
    fn test_detected_agent_builder() {
        let agent = DetectedAgent::new(AgentKind::ClaudeCode)
            .with_version("1.0.0")
            .with_detection_method(DetectionMethod::PathLookup);

        assert_eq!(agent.kind, AgentKind::ClaudeCode);
        assert_eq!(agent.version, Some("1.0.0".to_string()));
        assert_eq!(agent.detection_method, DetectionMethod::PathLookup);
    }
}
