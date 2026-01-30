//! Telemetry collection CLI for RCH workers.
#![forbid(unsafe_code)]

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use rch_telemetry::collect::{collect_telemetry, resolve_worker_id};
use rch_telemetry::{LogConfig, init_logging};

#[derive(Parser)]
#[command(name = "rch-telemetry", about = "Telemetry collection for RCH workers")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Collect a telemetry snapshot and print it
    Collect {
        /// Output format (json or pretty)
        #[arg(long, default_value = "json")]
        format: OutputFormat,

        /// Sampling window in milliseconds for rate-based metrics
        #[arg(long, default_value_t = 200)]
        sample_ms: u64,

        /// Disable disk telemetry collection
        #[arg(long)]
        no_disk: bool,

        /// Disable network telemetry collection
        #[arg(long)]
        no_network: bool,

        /// Override worker ID (defaults to RCH_WORKER_ID or HOSTNAME)
        #[arg(long)]
        worker_id: Option<String>,
    },
}

#[derive(ValueEnum, Clone, Copy)]
enum OutputFormat {
    Json,
    Pretty,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut log_config = LogConfig::from_env("info").with_stderr();
    if cli.verbose {
        log_config = log_config.with_level("debug");
    }
    let _logging_guards = init_logging(&log_config)?;

    match cli.command {
        Commands::Collect {
            format,
            sample_ms,
            no_disk,
            no_network,
            worker_id,
        } => {
            let worker_id = resolve_worker_id(worker_id);
            let telemetry = collect_telemetry(sample_ms, !no_disk, !no_network, worker_id)?;

            let output = match format {
                OutputFormat::Json => telemetry.to_json()?,
                OutputFormat::Pretty => telemetry.to_json_pretty()?,
            };

            println!("{}", output);
        }
    }

    Ok(())
}
