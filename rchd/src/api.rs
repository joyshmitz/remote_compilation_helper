//! Unix socket API for hook-daemon communication.
//!
//! Implements a simple HTTP-like protocol over Unix socket:
//! - Request: `GET /select-worker?project=X&cores=N\n`
//! - Response: JSON `SelectionResponse` or error

use crate::selection::{SelectionWeights, select_worker};
use crate::workers::WorkerPool;
use anyhow::{Result, anyhow};
use rch_common::{
    SelectedWorker, SelectionReason, SelectionRequest, SelectionResponse, WorkerStatus,
};
use serde::Serialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::{debug, warn};

/// Parsed API request variants.
#[derive(Debug)]
enum ApiRequest {
    SelectWorker(SelectionRequest),
    Status,
}

/// Response payload for GET /status.
#[derive(Debug, Serialize)]
struct DaemonStatus {
    workers_total: usize,
    workers_healthy: usize,
    workers_degraded: usize,
    workers_unreachable: usize,
    workers_draining: usize,
    workers_disabled: usize,
    slots_total: u32,
    slots_available: u32,
}

/// Handle an incoming connection on the Unix socket.
pub async fn handle_connection(stream: UnixStream, pool: WorkerPool) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    // Read the request line
    let n = reader.read_line(&mut line).await?;
    if n == 0 {
        return Ok(()); // Connection closed
    }

    let line = line.trim();
    debug!("Received request: {}", line);

    // Parse and handle the request
    let response_json = match parse_request(line) {
        Ok(ApiRequest::SelectWorker(request)) => {
            let response = handle_select_worker(&pool, request).await?;
            serde_json::to_string(&response)?
        }
        Ok(ApiRequest::Status) => {
            let status = handle_status(&pool).await?;
            serde_json::to_string(&status)?
        }
        Err(e) => return Err(e),
    };

    // Send the response
    let response_bytes = format!(
        "HTTP/1.0 200 OK\r\nContent-Type: application/json\r\n\r\n{}\n",
        response_json
    );

    writer.write_all(response_bytes.as_bytes()).await?;
    writer.flush().await?;

    Ok(())
}

/// Parse a request line into an ApiRequest.
fn parse_request(line: &str) -> Result<ApiRequest> {
    // Expected format: GET /select-worker?project=X&cores=N
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(anyhow!("Invalid request format"));
    }

    let method = parts[0];
    let path = parts[1];

    if method != "GET" {
        return Err(anyhow!("Only GET method supported"));
    }

    if path == "/status" {
        return Ok(ApiRequest::Status);
    }

    if !path.starts_with("/select-worker") {
        return Err(anyhow!("Unknown endpoint: {}", path));
    }

    // Parse query parameters
    let query = path.strip_prefix("/select-worker").unwrap_or("");
    let query = query.strip_prefix('?').unwrap_or("");

    let mut project = None;
    let mut cores = None;

    for param in query.split('&') {
        if param.is_empty() {
            continue;
        }
        let mut kv = param.splitn(2, '=');
        let key = kv.next().unwrap_or("");
        let value = kv.next().unwrap_or("");

        match key {
            "project" => project = Some(urlencoding_decode(value)),
            "cores" => cores = value.parse().ok(),
            _ => {} // Ignore unknown parameters
        }
    }

    let project = project.ok_or_else(|| anyhow!("Missing 'project' parameter"))?;
    let estimated_cores = cores.unwrap_or(1);

    Ok(ApiRequest::SelectWorker(SelectionRequest {
        project,
        estimated_cores,
        preferred_workers: vec![],
        toolchain: None,
    }))
}

/// URL percent-decoding.
///
/// Decodes %XX hex sequences to their original characters.
fn urlencoding_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            // Try to read two hex digits
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                    continue;
                }
            }
            // Invalid encoding, keep original
            result.push('%');
            result.push_str(&hex);
        } else if c == '+' {
            // + is space in application/x-www-form-urlencoded
            result.push(' ');
        } else {
            result.push(c);
        }
    }

    result
}

/// Handle a select-worker request.
async fn handle_select_worker(
    pool: &WorkerPool,
    request: SelectionRequest,
) -> Result<SelectionResponse> {
    debug!(
        "Selecting worker for project '{}' with {} cores",
        request.project, request.estimated_cores
    );

    // Check if any workers are configured
    let all_workers = pool.all_workers().await;
    if all_workers.is_empty() {
        debug!("No workers configured");
        return Ok(SelectionResponse {
            worker: None,
            reason: SelectionReason::NoWorkersConfigured,
        });
    }

    // Check if any workers are healthy
    let healthy_workers = pool.healthy_workers().await;
    if healthy_workers.is_empty() {
        debug!("All workers unreachable");
        return Ok(SelectionResponse {
            worker: None,
            reason: SelectionReason::AllWorkersUnreachable,
        });
    }

    let weights = SelectionWeights::default();
    let selected = select_worker(pool, &request, &weights).await;

    let Some(worker) = selected else {
        // Workers exist and are healthy, but none have enough slots
        debug!(
            "All workers busy (no worker has {} available slots)",
            request.estimated_cores
        );
        return Ok(SelectionResponse {
            worker: None,
            reason: SelectionReason::AllWorkersBusy,
        });
    };

    // Reserve the slots
    if !worker.reserve_slots(request.estimated_cores) {
        warn!(
            "Failed to reserve {} slots on {} (race condition)",
            request.estimated_cores, worker.config.id
        );
        // This can happen if another request reserved the slots between
        // selection and reservation. Treat as all workers busy.
        return Ok(SelectionResponse {
            worker: None,
            reason: SelectionReason::AllWorkersBusy,
        });
    }

    Ok(SelectionResponse {
        worker: Some(SelectedWorker {
            id: worker.config.id.clone(),
            host: worker.config.host.clone(),
            user: worker.config.user.clone(),
            identity_file: worker.config.identity_file.clone(),
            slots_available: worker.available_slots(),
            speed_score: worker.speed_score,
        }),
        reason: SelectionReason::Success,
    })
}

/// Handle a status request.
async fn handle_status(pool: &WorkerPool) -> Result<DaemonStatus> {
    let workers = pool.all_workers().await;

    let mut workers_healthy = 0;
    let mut workers_degraded = 0;
    let mut workers_unreachable = 0;
    let mut workers_draining = 0;
    let mut workers_disabled = 0;
    let mut slots_total = 0u32;
    let mut slots_available = 0u32;

    for worker in &workers {
        let status = worker.status().await;
        match status {
            WorkerStatus::Healthy => workers_healthy += 1,
            WorkerStatus::Degraded => workers_degraded += 1,
            WorkerStatus::Unreachable => workers_unreachable += 1,
            WorkerStatus::Draining => workers_draining += 1,
            WorkerStatus::Disabled => workers_disabled += 1,
        }

        slots_total = slots_total.saturating_add(worker.config.total_slots);
        slots_available = slots_available.saturating_add(worker.available_slots());
    }

    Ok(DaemonStatus {
        workers_total: workers.len(),
        workers_healthy,
        workers_degraded,
        workers_unreachable,
        workers_draining,
        workers_disabled,
        slots_total,
        slots_available,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_request_basic() {
        let req = parse_request("GET /select-worker?project=myproject&cores=4").unwrap();
        let ApiRequest::SelectWorker(req) = req else {
            panic!("expected select-worker request");
        };
        assert_eq!(req.project, "myproject");
        assert_eq!(req.estimated_cores, 4);
    }

    #[test]
    fn test_parse_request_project_only() {
        let req = parse_request("GET /select-worker?project=test").unwrap();
        let ApiRequest::SelectWorker(req) = req else {
            panic!("expected select-worker request");
        };
        assert_eq!(req.project, "test");
        assert_eq!(req.estimated_cores, 1); // Default
    }

    #[test]
    fn test_parse_request_with_spaces() {
        let req = parse_request("GET /select-worker?project=my%20project&cores=2").unwrap();
        let ApiRequest::SelectWorker(req) = req else {
            panic!("expected select-worker request");
        };
        assert_eq!(req.project, "my project");
        assert_eq!(req.estimated_cores, 2);
    }

    #[test]
    fn test_parse_request_status() {
        let req = parse_request("GET /status").unwrap();
        match req {
            ApiRequest::Status => {}
            _ => panic!("expected status request"),
        }
    }

    #[test]
    fn test_parse_request_missing_project() {
        let result = parse_request("GET /select-worker?cores=4");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_request_invalid_method() {
        let result = parse_request("POST /select-worker?project=test");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_request_unknown_endpoint() {
        let result = parse_request("GET /unknown?project=test");
        assert!(result.is_err());
    }

    #[test]
    fn test_urlencoding_decode_basic() {
        assert_eq!(urlencoding_decode("hello%20world"), "hello world");
        assert_eq!(urlencoding_decode("path%2Fto%2Ffile"), "path/to/file");
        assert_eq!(urlencoding_decode("foo%3Abar"), "foo:bar");
    }

    #[test]
    fn test_urlencoding_decode_special_chars() {
        assert_eq!(urlencoding_decode("a%26b%3Dc"), "a&b=c");
        assert_eq!(urlencoding_decode("100%25"), "100%");
        assert_eq!(urlencoding_decode("hello%2Bworld"), "hello+world");
    }

    #[test]
    fn test_urlencoding_decode_plus_as_space() {
        assert_eq!(urlencoding_decode("hello+world"), "hello world");
    }

    #[test]
    fn test_urlencoding_decode_no_encoding() {
        assert_eq!(urlencoding_decode("simple"), "simple");
        assert_eq!(
            urlencoding_decode("with-dash_underscore"),
            "with-dash_underscore"
        );
    }

    #[test]
    fn test_urlencoding_decode_invalid() {
        // Invalid hex should be preserved
        assert_eq!(urlencoding_decode("foo%GGbar"), "foo%GGbar");
        // Incomplete sequence at end
        assert_eq!(urlencoding_decode("foo%2"), "foo%2");
    }

    // =========================================================================
    // Selection response tests - reason field scenarios
    // =========================================================================

    use rch_common::{WorkerConfig, WorkerId, WorkerStatus};

    fn make_test_worker(id: &str, total_slots: u32) -> WorkerConfig {
        WorkerConfig {
            id: WorkerId::new(id),
            host: "localhost".to_string(),
            user: "user".to_string(),
            identity_file: "~/.ssh/id_rsa".to_string(),
            total_slots,
            priority: 100,
            tags: vec![],
        }
    }

    #[tokio::test]
    async fn test_handle_select_worker_no_workers_configured() {
        let pool = WorkerPool::new();
        let request = SelectionRequest {
            project: "test".to_string(),
            estimated_cores: 4,
            preferred_workers: vec![],
            toolchain: None,
        };

        let response = handle_select_worker(&pool, request).await.unwrap();
        assert!(response.worker.is_none());
        assert_eq!(response.reason, SelectionReason::NoWorkersConfigured);
    }

    #[tokio::test]
    async fn test_handle_select_worker_all_unreachable() {
        let pool = WorkerPool::new();
        pool.add_worker(make_test_worker("worker1", 8)).await;
        pool.add_worker(make_test_worker("worker2", 8)).await;

        // Mark all workers as unreachable
        pool.set_status(&WorkerId::new("worker1"), WorkerStatus::Unreachable)
            .await;
        pool.set_status(&WorkerId::new("worker2"), WorkerStatus::Unreachable)
            .await;

        let request = SelectionRequest {
            project: "test".to_string(),
            estimated_cores: 2,
            preferred_workers: vec![],
            toolchain: None,
        };

        let response = handle_select_worker(&pool, request).await.unwrap();
        assert!(response.worker.is_none());
        assert_eq!(response.reason, SelectionReason::AllWorkersUnreachable);
    }

    #[tokio::test]
    async fn test_handle_select_worker_all_busy() {
        let pool = WorkerPool::new();
        pool.add_worker(make_test_worker("worker1", 4)).await;

        // Reserve all slots
        let worker = pool.get(&WorkerId::new("worker1")).await.unwrap();
        worker.reserve_slots(4);

        let request = SelectionRequest {
            project: "test".to_string(),
            estimated_cores: 2, // Request more than available
            preferred_workers: vec![],
            toolchain: None,
        };

        let response = handle_select_worker(&pool, request).await.unwrap();
        assert!(response.worker.is_none());
        assert_eq!(response.reason, SelectionReason::AllWorkersBusy);
    }

    #[tokio::test]
    async fn test_handle_select_worker_success() {
        let pool = WorkerPool::new();
        pool.add_worker(make_test_worker("worker1", 16)).await;

        let request = SelectionRequest {
            project: "test".to_string(),
            estimated_cores: 4,
            preferred_workers: vec![],
            toolchain: None,
        };

        let response = handle_select_worker(&pool, request).await.unwrap();
        assert!(response.worker.is_some());
        assert_eq!(response.reason, SelectionReason::Success);

        let worker = response.worker.unwrap();
        assert_eq!(worker.id.as_str(), "worker1");
        // 16 total - 4 reserved = 12 available after reservation
        assert_eq!(worker.slots_available, 12);
    }

    #[tokio::test]
    async fn test_handle_select_worker_not_enough_slots() {
        let pool = WorkerPool::new();
        pool.add_worker(make_test_worker("worker1", 4)).await;

        let request = SelectionRequest {
            project: "test".to_string(),
            estimated_cores: 8, // Request more than total slots
            preferred_workers: vec![],
            toolchain: None,
        };

        let response = handle_select_worker(&pool, request).await.unwrap();
        assert!(response.worker.is_none());
        assert_eq!(response.reason, SelectionReason::AllWorkersBusy);
    }

    #[tokio::test]
    async fn test_handle_status_summary() {
        let pool = WorkerPool::new();
        pool.add_worker(make_test_worker("worker1", 4)).await;
        pool.add_worker(make_test_worker("worker2", 8)).await;

        // Set worker2 to degraded to verify status counts
        pool.set_status(&WorkerId::new("worker2"), WorkerStatus::Degraded)
            .await;

        // Reserve some slots
        let worker1 = pool.get(&WorkerId::new("worker1")).await.unwrap();
        let worker2 = pool.get(&WorkerId::new("worker2")).await.unwrap();
        assert!(worker1.reserve_slots(2));
        assert!(worker2.reserve_slots(3));

        let status = handle_status(&pool).await.unwrap();

        assert_eq!(status.workers_total, 2);
        assert_eq!(status.workers_healthy, 1);
        assert_eq!(status.workers_degraded, 1);
        assert_eq!(status.workers_unreachable, 0);
        assert_eq!(status.workers_draining, 0);
        assert_eq!(status.workers_disabled, 0);
        assert_eq!(status.slots_total, 12);
        assert_eq!(status.slots_available, 7);
    }
}
