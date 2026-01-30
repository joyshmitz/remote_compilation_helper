//! Background cleanup for active builds with dead hooks.

use crate::DaemonContext;
use rch_common::WorkerId;
use std::path::Path;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, warn};

pub struct ActiveBuildCleanup {
    context: DaemonContext,
}

impl ActiveBuildCleanup {
    pub fn new(context: DaemonContext) -> Self {
        Self { context }
    }

    pub fn start(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(5));
            loop {
                ticker.tick().await;
                self.check_active_builds().await;
            }
        })
    }

    async fn check_active_builds(&self) {
        let active_builds = self.context.history.active_builds();
        if active_builds.is_empty() {
            return;
        }

        for build in active_builds {
            // Check if hook process is alive
            if build.hook_pid > 0 && !is_process_alive(build.hook_pid) {
                warn!(
                    "Hook process {} for build {} (project: {}) appears dead. Cleaning up.",
                    build.hook_pid, build.id, build.project_id
                );

                // Release slots
                self.context
                    .pool
                    .release_slots(&WorkerId::new(&build.worker_id), build.slots)
                    .await;

                // Cancel build in history
                if self
                    .context
                    .history
                    .cancel_active_build(build.id, None)
                    .is_some()
                    && !cfg!(test)
                {
                    crate::metrics::dec_active_builds("remote");
                    crate::metrics::inc_build_total("cancelled", "remote");
                }

                // Emit event
                self.context.events.emit(
                    "build_cancelled",
                    &serde_json::json!({
                        "build_id": build.id,
                        "project_id": build.project_id,
                        "reason": "hook_died",
                        "worker_id": build.worker_id,
                    }),
                );
            }
        }
    }
}

fn is_process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }

    // Check /proc first (Linux only) - efficient check without syscall overhead
    if cfg!(target_os = "linux") {
        return Path::new(&format!("/proc/{}", pid)).exists();
    }

    // Fallback to kill -0 for other Unix systems
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}
