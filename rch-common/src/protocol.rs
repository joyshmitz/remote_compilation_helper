//! Claude Code hook protocol definitions.
//!
//! Defines the JSON structures for PreToolUse hook input/output.

use serde::{Deserialize, Serialize};

/// Input received from Claude Code PreToolUse hook.
#[derive(Debug, Clone, Deserialize)]
pub struct HookInput {
    /// The tool being invoked (e.g., "Bash", "Read", "Write").
    pub tool_name: String,
    /// Tool-specific input.
    pub tool_input: ToolInput,
    /// Optional session ID.
    #[serde(default)]
    pub session_id: Option<String>,
}

/// Tool-specific input for Bash commands.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolInput {
    /// The command to execute.
    pub command: String,
    /// Optional description of what the command does.
    #[serde(default)]
    pub description: Option<String>,
}

/// Output sent back to Claude Code from PreToolUse hook.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum HookOutput {
    /// Allow the command to proceed (empty object or no output).
    Allow(AllowOutput),
    /// Deny the command with a reason.
    Deny(DenyOutput),
}

/// Empty output to allow command execution.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AllowOutput {}

/// Output to deny/block command execution.
#[derive(Debug, Clone, Serialize)]
pub struct DenyOutput {
    #[serde(rename = "hookSpecificOutput")]
    pub hook_specific_output: HookSpecificOutput,
}

#[derive(Debug, Clone, Serialize)]
pub struct HookSpecificOutput {
    #[serde(rename = "hookEventName")]
    pub hook_event_name: String,
    #[serde(rename = "permissionDecision")]
    pub permission_decision: String,
    #[serde(rename = "permissionDecisionReason")]
    pub permission_decision_reason: String,
}

impl HookOutput {
    /// Create an allow output (command proceeds normally).
    pub fn allow() -> Self {
        Self::Allow(AllowOutput {})
    }

    /// Create a deny output with a reason.
    pub fn deny(reason: impl Into<String>) -> Self {
        Self::Deny(DenyOutput {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: "PreToolUse".to_string(),
                permission_decision: "deny".to_string(),
                permission_decision_reason: reason.into(),
            },
        })
    }

    /// Check if this output allows the command.
    pub fn is_allow(&self) -> bool {
        matches!(self, Self::Allow(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hook_input() {
        let json = r#"{
            "tool_name": "Bash",
            "tool_input": {
                "command": "cargo build --release",
                "description": "Build the project"
            },
            "session_id": "abc123"
        }"#;

        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.tool_name, "Bash");
        assert_eq!(input.tool_input.command, "cargo build --release");
        assert_eq!(input.session_id, Some("abc123".to_string()));
    }

    #[test]
    fn test_allow_output() {
        let output = HookOutput::allow();
        let json = serde_json::to_string(&output).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_deny_output() {
        let output = HookOutput::deny("Remote execution failed");
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("permissionDecision"));
        assert!(json.contains("deny"));
    }

    #[test]
    fn test_parse_hook_input_minimal() {
        // Parse without optional fields
        let json = r#"{
            "tool_name": "Bash",
            "tool_input": {
                "command": "ls -la"
            }
        }"#;

        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.tool_name, "Bash");
        assert_eq!(input.tool_input.command, "ls -la");
        assert!(input.tool_input.description.is_none());
        assert!(input.session_id.is_none());
    }

    #[test]
    fn test_parse_hook_input_with_empty_description() {
        let json = r#"{
            "tool_name": "Read",
            "tool_input": {
                "command": "cat file.txt",
                "description": ""
            }
        }"#;

        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.tool_name, "Read");
        assert_eq!(input.tool_input.description, Some("".to_string()));
    }

    #[test]
    fn test_hook_output_is_allow_true() {
        let output = HookOutput::allow();
        assert!(output.is_allow());
    }

    #[test]
    fn test_hook_output_is_allow_false_for_deny() {
        let output = HookOutput::deny("blocked");
        assert!(!output.is_allow());
    }

    #[test]
    fn test_deny_output_preserves_reason() {
        let reason = "Command not allowed: security violation";
        let output = HookOutput::deny(reason);

        if let HookOutput::Deny(deny) = output {
            assert_eq!(
                deny.hook_specific_output.permission_decision_reason,
                reason
            );
            assert_eq!(deny.hook_specific_output.permission_decision, "deny");
            assert_eq!(deny.hook_specific_output.hook_event_name, "PreToolUse");
        } else {
            panic!("Expected Deny variant");
        }
    }

    #[test]
    fn test_deny_output_with_empty_reason() {
        let output = HookOutput::deny("");
        if let HookOutput::Deny(deny) = output {
            assert_eq!(deny.hook_specific_output.permission_decision_reason, "");
        } else {
            panic!("Expected Deny variant");
        }
    }

    #[test]
    fn test_allow_output_default() {
        let output = AllowOutput::default();
        let json = serde_json::to_string(&output).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_deny_output_json_structure() {
        let output = HookOutput::deny("test reason");
        let json = serde_json::to_string(&output).unwrap();

        // Verify the exact JSON structure expected by Claude Code
        assert!(json.contains("hookSpecificOutput"));
        assert!(json.contains("hookEventName"));
        assert!(json.contains("PreToolUse"));
        assert!(json.contains("permissionDecision"));
        assert!(json.contains("\"deny\""));
        assert!(json.contains("permissionDecisionReason"));
        assert!(json.contains("test reason"));
    }

    #[test]
    fn test_hook_input_clone() {
        let original = HookInput {
            tool_name: "Bash".to_string(),
            tool_input: ToolInput {
                command: "cargo test".to_string(),
                description: Some("Run tests".to_string()),
            },
            session_id: Some("session-123".to_string()),
        };

        let cloned = original.clone();
        assert_eq!(original.tool_name, cloned.tool_name);
        assert_eq!(original.tool_input.command, cloned.tool_input.command);
        assert_eq!(original.session_id, cloned.session_id);
    }

    #[test]
    fn test_tool_input_clone() {
        let original = ToolInput {
            command: "make build".to_string(),
            description: None,
        };

        let cloned = original.clone();
        assert_eq!(original.command, cloned.command);
        assert_eq!(original.description, cloned.description);
    }

    #[test]
    fn test_hook_output_clone_allow() {
        let original = HookOutput::allow();
        let cloned = original.clone();
        assert!(cloned.is_allow());
    }

    #[test]
    fn test_hook_output_clone_deny() {
        let original = HookOutput::deny("cloned reason");
        let cloned = original.clone();
        assert!(!cloned.is_allow());
    }

    #[test]
    fn test_deny_output_from_string() {
        // Test the Into<String> conversion
        let output = HookOutput::deny(String::from("owned reason"));
        if let HookOutput::Deny(deny) = output {
            assert_eq!(
                deny.hook_specific_output.permission_decision_reason,
                "owned reason"
            );
        } else {
            panic!("Expected Deny variant");
        }
    }

    #[test]
    fn test_parse_hook_input_different_tools() {
        let tools = ["Bash", "Read", "Write", "Edit", "Glob", "Grep"];

        for tool in tools {
            let json = format!(
                r#"{{"tool_name": "{}", "tool_input": {{"command": "test"}}}}"#,
                tool
            );
            let input: HookInput = serde_json::from_str(&json).unwrap();
            assert_eq!(input.tool_name, tool);
        }
    }

    #[test]
    fn test_parse_hook_input_unicode_command() {
        let json = r#"{
            "tool_name": "Bash",
            "tool_input": {
                "command": "echo '日本語 测试 émojis 🦀'"
            }
        }"#;

        let input: HookInput = serde_json::from_str(json).unwrap();
        assert!(input.tool_input.command.contains("日本語"));
        assert!(input.tool_input.command.contains("🦀"));
    }

    #[test]
    fn test_parse_hook_input_special_characters() {
        let json = r#"{
            "tool_name": "Bash",
            "tool_input": {
                "command": "echo \"hello\\nworld\" | grep 'pattern'"
            }
        }"#;

        let input: HookInput = serde_json::from_str(json).unwrap();
        assert!(input.tool_input.command.contains("echo"));
        assert!(input.tool_input.command.contains("grep"));
    }

    #[test]
    fn test_hook_specific_output_debug() {
        let output = HookSpecificOutput {
            hook_event_name: "PreToolUse".to_string(),
            permission_decision: "deny".to_string(),
            permission_decision_reason: "test".to_string(),
        };

        // Verify Debug trait works
        let debug_str = format!("{:?}", output);
        assert!(debug_str.contains("HookSpecificOutput"));
        assert!(debug_str.contains("PreToolUse"));
    }

    #[test]
    fn test_deny_output_debug() {
        let output = DenyOutput {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: "PreToolUse".to_string(),
                permission_decision: "deny".to_string(),
                permission_decision_reason: "test".to_string(),
            },
        };

        let debug_str = format!("{:?}", output);
        assert!(debug_str.contains("DenyOutput"));
    }

    #[test]
    fn test_allow_output_debug() {
        let output = AllowOutput {};
        let debug_str = format!("{:?}", output);
        assert!(debug_str.contains("AllowOutput"));
    }
}
