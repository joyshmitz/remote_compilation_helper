//! PreToolUse hook implementation.
//!
//! Handles incoming hook requests from Claude Code, classifies commands,
//! and routes compilation commands to remote workers.

use anyhow::Result;
use rch_common::{classify_command, HookInput, HookOutput, SelectionResponse};
use std::io::{self, BufRead, Write};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::{debug, info, warn};

/// Run the hook, reading from stdin and writing to stdout.
pub async fn run_hook() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    // Read all input from stdin
    let mut input = String::new();
    for line in stdin.lock().lines() {
        input.push_str(&line?);
        input.push('\n');
    }

    let input = input.trim();
    if input.is_empty() {
        // No input - just allow
        return Ok(());
    }

    // Parse the hook input
    let hook_input: HookInput = match serde_json::from_str(input) {
        Ok(hi) => hi,
        Err(e) => {
            warn!("Failed to parse hook input: {}", e);
            // On parse error, allow the command (fail-open)
            return Ok(());
        }
    };

    // Process the hook request
    let output = process_hook(hook_input).await;

    // Write output
    if let HookOutput::Deny(_) = &output {
        let json = serde_json::to_string(&output)?;
        writeln!(stdout, "{}", json)?;
    }
    // For Allow, we output nothing (empty stdout = allow)

    Ok(())
}

/// Process a hook request and return the output.
async fn process_hook(input: HookInput) -> HookOutput {
    // Tier 0: Only process Bash tool
    if input.tool_name != "Bash" {
        debug!("Non-Bash tool: {}, allowing", input.tool_name);
        return HookOutput::allow();
    }

    let command = &input.tool_input.command;
    debug!("Processing command: {}", command);

    // Classify the command using 5-tier system
    let classification = classify_command(command);

    if !classification.is_compilation {
        debug!(
            "Not a compilation command: {} ({})",
            command, classification.reason
        );
        return HookOutput::allow();
    }

    info!(
        "Compilation detected: {:?} (confidence: {:.2})",
        classification.kind, classification.confidence
    );

    // Check confidence threshold
    // TODO: Load from config
    let confidence_threshold = 0.85;
    if classification.confidence < confidence_threshold {
        debug!(
            "Confidence {:.2} below threshold {:.2}, allowing local execution",
            classification.confidence, confidence_threshold
        );
        return HookOutput::allow();
    }

    // Query daemon for a worker
    let socket_path = "/tmp/rch.sock";
    let project = extract_project_name();

    match query_daemon(socket_path, &project, 4).await {
        Ok(worker) => {
            info!(
                "Selected worker: {} at {}@{} ({} slots, speed {:.1})",
                worker.worker, worker.user, worker.host, worker.slots_available, worker.speed_score
            );
            // TODO: Execute remote compilation pipeline with selected worker
            // For now, allow local execution after logging worker selection
            info!("Remote execution pipeline not yet implemented, allowing local");
            HookOutput::allow()
        }
        Err(e) => {
            warn!("Failed to query daemon: {}, allowing local execution", e);
            HookOutput::allow()
        }
    }
}

/// Query the daemon for a worker.
async fn query_daemon(
    socket_path: &str,
    project: &str,
    cores: u32,
) -> Result<SelectionResponse> {
    // Check if socket exists
    if !Path::new(socket_path).exists() {
        return Err(anyhow::anyhow!("Daemon socket not found: {}", socket_path));
    }

    // Connect to daemon
    let stream = UnixStream::connect(socket_path).await?;
    let (reader, mut writer) = stream.into_split();

    // Send request
    let request = format!(
        "GET /select-worker?project={}&cores={}\n",
        urlencoding_encode(project),
        cores
    );
    writer.write_all(request.as_bytes()).await?;
    writer.flush().await?;

    // Read response (skip HTTP headers)
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut body = String::new();
    let mut in_body = false;

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }
        if in_body {
            body.push_str(&line);
        } else if line.trim().is_empty() {
            in_body = true;
        }
    }

    // Parse response
    let response: SelectionResponse = serde_json::from_str(body.trim())?;
    Ok(response)
}

/// Simple URL encoding for project names.
fn urlencoding_encode(s: &str) -> String {
    s.replace(' ', "%20")
        .replace('/', "%2F")
        .replace(':', "%3A")
}

/// Extract project name from current working directory.
fn extract_project_name() -> String {
    std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rch_common::ToolInput;

    #[tokio::test]
    async fn test_non_bash_allowed() {
        let input = HookInput {
            tool_name: "Read".to_string(),
            tool_input: ToolInput {
                command: "anything".to_string(),
                description: None,
            },
            session_id: None,
        };

        let output = process_hook(input).await;
        assert!(output.is_allow());
    }

    #[tokio::test]
    async fn test_non_compilation_allowed() {
        let input = HookInput {
            tool_name: "Bash".to_string(),
            tool_input: ToolInput {
                command: "ls -la".to_string(),
                description: None,
            },
            session_id: None,
        };

        let output = process_hook(input).await;
        assert!(output.is_allow());
    }

    #[tokio::test]
    async fn test_compilation_detected() {
        let input = HookInput {
            tool_name: "Bash".to_string(),
            tool_input: ToolInput {
                command: "cargo build --release".to_string(),
                description: None,
            },
            session_id: None,
        };

        // Currently allows because remote execution not implemented
        let output = process_hook(input).await;
        assert!(output.is_allow());
    }
}
