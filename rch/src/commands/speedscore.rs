//! SpeedScore commands.
//!
//! This module contains the CLI command for querying and displaying
//! worker SpeedScore benchmarks.

use crate::error::ConfigError;
use crate::status_types::{
    SpeedScoreHistoryResponseFromApi, SpeedScoreListResponseFromApi, SpeedScoreResponseFromApi,
    SpeedScoreViewFromApi, extract_json_body,
};
use crate::ui::context::OutputContext;
use crate::ui::theme::StatusIndicator;
use anyhow::{Context, Result};
use rch_common::ApiResponse;

use super::{load_workers_from_config, send_daemon_command};

// =============================================================================
// SpeedScore Query Helpers
// =============================================================================

/// Query the daemon for all worker SpeedScores.
async fn query_speedscore_list() -> Result<SpeedScoreListResponseFromApi> {
    let response = send_daemon_command("GET /speedscores\n").await?;
    let json = extract_json_body(&response)
        .ok_or_else(|| anyhow::anyhow!("Invalid response format from daemon"))?;
    let scores: SpeedScoreListResponseFromApi =
        serde_json::from_str(json).context("Failed to parse SpeedScore list response")?;
    Ok(scores)
}

/// Query the daemon for a single worker's SpeedScore.
async fn query_speedscore(worker_id: &str) -> Result<SpeedScoreResponseFromApi> {
    let command = format!("GET /speedscore/{}\n", worker_id);
    let response = send_daemon_command(&command).await?;
    let json = extract_json_body(&response)
        .ok_or_else(|| anyhow::anyhow!("Invalid response format from daemon"))?;
    let score: SpeedScoreResponseFromApi =
        serde_json::from_str(json).context("Failed to parse SpeedScore response")?;
    Ok(score)
}

/// Query the daemon for a worker's SpeedScore history.
async fn query_speedscore_history(
    worker_id: &str,
    days: u32,
    limit: usize,
) -> Result<SpeedScoreHistoryResponseFromApi> {
    let command = format!(
        "GET /speedscore/{}/history?days={}&limit={}\n",
        worker_id, days, limit
    );
    let response = send_daemon_command(&command).await?;
    let json = extract_json_body(&response)
        .ok_or_else(|| anyhow::anyhow!("Invalid response format from daemon"))?;
    let history: SpeedScoreHistoryResponseFromApi =
        serde_json::from_str(json).context("Failed to parse SpeedScore history response")?;
    Ok(history)
}

// =============================================================================
// Display Helpers
// =============================================================================

/// Format a SpeedScore value with color based on rating.
fn format_score(score: f64, style: &crate::ui::theme::Theme) -> String {
    match score {
        x if x >= 90.0 => style.success(&format!("{:.1}", x)).to_string(),
        x if x >= 75.0 => style.success(&format!("{:.1}", x)).to_string(),
        x if x >= 60.0 => style.info(&format!("{:.1}", x)).to_string(),
        x if x >= 45.0 => style.warning(&format!("{:.1}", x)).to_string(),
        x => style.error(&format!("{:.1}", x)).to_string(),
    }
}

/// Display SpeedScore details in verbose mode.
fn display_speedscore_verbose(score: &SpeedScoreViewFromApi, style: &crate::ui::theme::Theme) {
    println!(
        "    {} {} {}",
        style.key("CPU"),
        style.muted(":"),
        format_score(score.cpu_score, style)
    );
    println!(
        "    {} {} {}",
        style.key("Memory"),
        style.muted(":"),
        format_score(score.memory_score, style)
    );
    println!(
        "    {} {} {}",
        style.key("Disk"),
        style.muted(":"),
        format_score(score.disk_score, style)
    );
    println!(
        "    {} {} {}",
        style.key("Network"),
        style.muted(":"),
        format_score(score.network_score, style)
    );
    println!(
        "    {} {} {}",
        style.key("Compilation"),
        style.muted(":"),
        format_score(score.compilation_score, style)
    );
}

// =============================================================================
// SpeedScore Command
// =============================================================================

/// SpeedScore CLI command entry point.
pub async fn speedscore(
    worker: Option<String>,
    all: bool,
    history: bool,
    days: u32,
    limit: usize,
    ctx: &OutputContext,
) -> Result<()> {
    let verbose = ctx.is_verbose();
    let style = ctx.theme();

    // Validate arguments
    if all && worker.is_some() {
        return Err(ConfigError::InvalidValue {
            field: "worker".to_string(),
            reason: "Cannot specify both --all and a worker ID".to_string(),
            suggestion: "Remove either --all or the worker ID".to_string(),
        }
        .into());
    }
    if history && worker.is_none() && !all {
        return Err(ConfigError::InvalidValue {
            field: "--history".to_string(),
            reason: "--history requires a worker ID or --all".to_string(),
            suggestion: "Add a worker ID or use --all".to_string(),
        }
        .into());
    }
    if !all && worker.is_none() {
        return Err(ConfigError::InvalidValue {
            field: "worker".to_string(),
            reason: "No worker specified".to_string(),
            suggestion: "Specify a worker ID or use --all to show all workers".to_string(),
        }
        .into());
    }

    // Handle history mode
    if history {
        if all {
            // Show brief history for all workers
            let workers = load_workers_from_config()?;
            if ctx.is_json() {
                let mut all_history = Vec::new();
                for w in &workers {
                    if let Ok(h) = query_speedscore_history(w.id.as_str(), days, limit).await {
                        all_history.push(h);
                    }
                }
                let _ = ctx.json(&ApiResponse::ok("speedscore history", all_history));
                return Ok(());
            }

            println!(
                "{}",
                style.format_header("SpeedScore History (All Workers)")
            );
            println!();

            for w in &workers {
                match query_speedscore_history(w.id.as_str(), days, limit).await {
                    Ok(history_resp) => {
                        println!(
                            "  {} {} ({} entries)",
                            style.symbols.bullet_filled,
                            style.highlight(w.id.as_str()),
                            history_resp.history.len()
                        );
                        if let Some(latest) = history_resp.history.first() {
                            println!(
                                "    {} {} → {}",
                                style.muted("Latest:"),
                                format_score(latest.total, style),
                                style.muted(&latest.measured_at)
                            );
                        }
                        if history_resp.history.len() > 1 {
                            let oldest = history_resp.history.last().unwrap();
                            let newest = history_resp.history.first().unwrap();
                            let trend = newest.total - oldest.total;
                            let trend_str = if trend > 0.0 {
                                style.success(&format!("+{:.1}", trend))
                            } else if trend < 0.0 {
                                style.error(&format!("{:.1}", trend))
                            } else {
                                style.muted("±0.0")
                            };
                            println!(
                                "    {} {} (over {} entries)",
                                style.muted("Trend:"),
                                trend_str,
                                history_resp.history.len()
                            );
                        }
                        println!();
                    }
                    Err(e) => {
                        println!(
                            "  {} {} - {}",
                            StatusIndicator::Error.display(style),
                            style.highlight(w.id.as_str()),
                            style.error(&format!("Failed: {}", e))
                        );
                        println!();
                    }
                }
            }
            return Ok(());
        }

        // Show detailed history for single worker
        let worker_id = worker.as_ref().unwrap();
        let history_resp = query_speedscore_history(worker_id, days, limit).await?;

        if ctx.is_json() {
            let _ = ctx.json(&ApiResponse::ok("speedscore history", history_resp));
            return Ok(());
        }

        println!(
            "{}",
            style.format_header(&format!("SpeedScore History: {}", worker_id))
        );
        println!();

        if history_resp.history.is_empty() {
            println!(
                "  {} No history available for worker '{}'",
                StatusIndicator::Info.display(style),
                worker_id
            );
            return Ok(());
        }

        println!(
            "  {} {} entries (last {} days)",
            style.muted("Showing"),
            history_resp.history.len(),
            days
        );
        println!();

        for entry in &history_resp.history {
            println!(
                "  {} {} {} {}",
                style.symbols.bullet_filled,
                format_score(entry.total, style),
                style.muted(entry.rating()),
                style.muted(&format!("({})", entry.measured_at))
            );
            if verbose {
                display_speedscore_verbose(entry, style);
            }
        }

        // Show trend if multiple entries
        if history_resp.history.len() > 1 {
            let oldest = history_resp.history.last().unwrap();
            let newest = history_resp.history.first().unwrap();
            let trend = newest.total - oldest.total;
            let trend_str = if trend > 0.0 {
                style.success(&format!("+{:.1}", trend)).to_string()
            } else if trend < 0.0 {
                style.error(&format!("{:.1}", trend)).to_string()
            } else {
                style.muted("±0.0").to_string()
            };
            println!();
            println!(
                "  {} {} over {} entries",
                style.muted("Trend:"),
                trend_str,
                history_resp.history.len()
            );
        }

        return Ok(());
    }

    // Handle --all mode (show all workers)
    if all {
        let scores = query_speedscore_list().await?;

        if ctx.is_json() {
            let _ = ctx.json(&ApiResponse::ok("speedscore list", scores));
            return Ok(());
        }

        println!("{}", style.format_header("Worker SpeedScores"));
        println!();

        if scores.workers.is_empty() {
            println!(
                "  {} No workers found",
                StatusIndicator::Info.display(style)
            );
            return Ok(());
        }

        for entry in &scores.workers {
            let status_indicator = match entry.status.status.as_str() {
                "healthy" => StatusIndicator::Success.display(style),
                "degraded" => StatusIndicator::Warning.display(style),
                _ => StatusIndicator::Error.display(style),
            };

            let score_display = if let Some(ref score) = entry.speedscore {
                format!(
                    "{} {}",
                    format_score(score.total, style),
                    style.muted(&format!("({})", score.rating()))
                )
            } else {
                style.muted("N/A").to_string()
            };

            println!(
                "  {} {} {} {}",
                status_indicator,
                style.highlight(&entry.worker_id),
                style.muted(":"),
                score_display
            );

            if verbose && let Some(ref score) = entry.speedscore {
                display_speedscore_verbose(score, style);
            }
        }

        println!();
        println!(
            "{} {} worker(s)",
            style.muted("Total:"),
            style.highlight(&scores.workers.len().to_string())
        );

        return Ok(());
    }

    // Show single worker SpeedScore
    let worker_id = worker.as_ref().unwrap();
    let score_resp = query_speedscore(worker_id).await?;

    if ctx.is_json() {
        let _ = ctx.json(&ApiResponse::ok("speedscore", score_resp));
        return Ok(());
    }

    println!(
        "{}",
        style.format_header(&format!("SpeedScore: {}", worker_id))
    );
    println!();

    match score_resp.speedscore {
        Some(score) => {
            println!(
                "  {} {} {} {}",
                style.key("Total"),
                style.muted(":"),
                format_score(score.total, style),
                style.muted(&format!("({})", score.rating()))
            );
            println!(
                "  {} {} {}",
                style.key("Measured"),
                style.muted(":"),
                style.muted(&score.measured_at)
            );
            println!();

            if verbose {
                println!("{}", style.highlight("  Component Breakdown:"));
                println!(
                    "    {} {} {} {}",
                    style.key("CPU (30%)"),
                    style.muted(":"),
                    format_score(score.cpu_score, style),
                    style.muted("- processor benchmark")
                );
                println!(
                    "    {} {} {} {}",
                    style.key("Memory (15%)"),
                    style.muted(":"),
                    format_score(score.memory_score, style),
                    style.muted("- memory bandwidth")
                );
                println!(
                    "    {} {} {} {}",
                    style.key("Disk (20%)"),
                    style.muted(":"),
                    format_score(score.disk_score, style),
                    style.muted("- I/O throughput")
                );
                println!(
                    "    {} {} {} {}",
                    style.key("Network (15%)"),
                    style.muted(":"),
                    format_score(score.network_score, style),
                    style.muted("- latency & bandwidth")
                );
                println!(
                    "    {} {} {} {}",
                    style.key("Compile (20%)"),
                    style.muted(":"),
                    format_score(score.compilation_score, style),
                    style.muted("- build performance")
                );
            } else {
                println!(
                    "  {} Use {} for component breakdown",
                    style.muted("Tip:"),
                    style.highlight("--verbose")
                );
            }
        }
        None => {
            let msg = score_resp
                .message
                .unwrap_or_else(|| "No SpeedScore available".to_string());
            println!(
                "  {} {}",
                StatusIndicator::Info.display(style),
                style.muted(&msg)
            );
            println!();
            println!(
                "  {} Run {} to generate a score",
                style.muted("Tip:"),
                style.highlight("rch workers benchmark")
            );
        }
    }

    Ok(())
}
