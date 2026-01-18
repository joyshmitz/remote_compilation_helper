//! Worker selection algorithm.

#![allow(dead_code)] // Scaffold code - methods will be used in future beads

use crate::metrics::latency::{DecisionTimer, DecisionType};
use crate::workers::{WorkerPool, WorkerState};
use rch_common::{
    CircuitBreakerConfig, CircuitState, RequiredRuntime, SelectionReason, SelectionRequest,
};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::Arc;
use tracing::debug;

/// Weights for the selection scoring algorithm.
#[derive(Debug, Clone)]
pub struct SelectionWeights {
    /// Weight for available slots (0.0-1.0).
    pub slots: f64,
    /// Weight for speed score (0.0-1.0).
    pub speed: f64,
    /// Weight for project locality (0.0-1.0).
    pub locality: f64,
    /// Weight for worker priority (0.0-1.0).
    pub priority: f64,
    /// Penalty for half-open circuit workers (multiplier 0.0-1.0).
    pub half_open_penalty: f64,
}

impl Default for SelectionWeights {
    fn default() -> Self {
        Self {
            slots: 0.4,
            speed: 0.5,
            locality: 0.1,
            priority: 0.1,
            half_open_penalty: 0.5, // Half-open workers score at 50% of their normal value
        }
    }
}

/// Result of worker selection with reason.
pub struct SelectionResult {
    /// Selected worker, if available.
    pub worker: Option<Arc<WorkerState>>,
    /// Reason for the selection result.
    pub reason: SelectionReason,
}

/// Select the best worker for a request, considering circuit breaker state.
///
/// Workers with open circuits are excluded. Half-open workers are only
/// considered if they have probe budget available, and receive a scoring
/// penalty.
pub async fn select_worker(
    pool: &WorkerPool,
    request: &SelectionRequest,
    weights: &SelectionWeights,
) -> Option<Arc<WorkerState>> {
    let config = CircuitBreakerConfig::default();
    select_worker_with_config(pool, request, weights, &config)
        .await
        .worker
}

/// Select the best worker with explicit circuit breaker config.
///
/// Returns both the selected worker and the reason for selection result.
/// Tracks selection latency against the <10ms budget from AGENTS.md.
pub async fn select_worker_with_config(
    pool: &WorkerPool,
    request: &SelectionRequest,
    weights: &SelectionWeights,
    circuit_config: &CircuitBreakerConfig,
) -> SelectionResult {
    // Track worker selection latency (budget: <10ms, panic: 50ms)
    let _timer = DecisionTimer::new(DecisionType::WorkerSelection);

    let workers = pool.healthy_workers().await;

    if workers.is_empty() {
        // Check if there are any workers at all
        if pool.is_empty() {
            return SelectionResult {
                worker: None,
                reason: SelectionReason::NoWorkersConfigured,
            };
        }

        // All workers are unhealthy - check if it's due to unreachability or circuits
        let all_workers = pool.all_workers().await;
        let mut all_circuits_open = true;
        let mut all_unreachable = true;

        for worker in &all_workers {
            if let Some(state) = worker.circuit_state().await {
                if state != CircuitState::Open {
                    all_circuits_open = false;
                }
            }
            if worker.status().await != rch_common::WorkerStatus::Unreachable {
                all_unreachable = false;
            }
        }

        if all_circuits_open {
            return SelectionResult {
                worker: None,
                reason: SelectionReason::AllCircuitsOpen,
            };
        }

        if all_unreachable {
            return SelectionResult {
                worker: None,
                reason: SelectionReason::AllWorkersUnreachable,
            };
        }

        return SelectionResult {
            worker: None,
            reason: SelectionReason::AllWorkersUnreachable,
        };
    }

    let preferred_set: HashSet<&str> = request
        .preferred_workers
        .iter()
        .map(|id| id.as_str())
        .collect();
    let has_preferred = !preferred_set.is_empty();

    // Filter workers by circuit state and slot availability
    let mut eligible: Vec<(Arc<WorkerState>, CircuitState)> = Vec::new();
    let mut preferred: Vec<(Arc<WorkerState>, CircuitState)> = Vec::new();
    let mut all_circuits_open = true;
    let mut any_has_slots = false;
    let mut any_has_runtime = false;

    for worker in workers {
        let circuit_state = worker.circuit_state().await.unwrap_or(CircuitState::Closed);

        match circuit_state {
            CircuitState::Open => {
                debug!("Worker {} excluded: circuit open", worker.config.id);
                continue;
            }
            CircuitState::HalfOpen => {
                // Only allow if probe budget available
                if !worker.can_probe(circuit_config).await {
                    debug!(
                        "Worker {} excluded: half-open, no probe budget",
                        worker.config.id
                    );
                    continue;
                }
                all_circuits_open = false;
            }
            CircuitState::Closed => {
                all_circuits_open = false;
            }
        }

        // Check slot availability
        if worker.available_slots() < request.estimated_cores {
            debug!(
                "Worker {} excluded: insufficient slots ({} < {})",
                worker.config.id,
                worker.available_slots(),
                request.estimated_cores
            );
            continue;
        }

        // Check required runtime capability
        let has_required_runtime = match &request.required_runtime {
            RequiredRuntime::None => true,
            RequiredRuntime::Rust => worker.has_rust().await,
            RequiredRuntime::Bun => worker.has_bun().await,
            RequiredRuntime::Node => worker.has_node().await,
        };

        if !has_required_runtime {
            debug!(
                "Worker {} excluded: missing required runtime {:?}",
                worker.config.id, request.required_runtime
            );
            continue;
        }

        any_has_runtime = true;
        any_has_slots = true;

        // Compute score with circuit state penalty
        if has_preferred && preferred_set.contains(worker.config.id.as_str()) {
            preferred.push((worker.clone(), circuit_state));
        }

        eligible.push((worker, circuit_state));
    }

    let mut candidates = if has_preferred && !preferred.is_empty() {
        preferred
    } else {
        if has_preferred {
            debug!("Preferred workers not eligible; falling back to all eligible workers");
        }
        eligible
    };

    if candidates.is_empty() {
        // Check if no workers have required runtime (before other checks)
        if !any_has_runtime && request.required_runtime != RequiredRuntime::None {
            return SelectionResult {
                worker: None,
                reason: SelectionReason::NoWorkersWithRuntime(format!(
                    "{:?}",
                    request.required_runtime
                )),
            };
        }

        if all_circuits_open {
            return SelectionResult {
                worker: None,
                reason: SelectionReason::AllCircuitsOpen,
            };
        }

        if !any_has_slots {
            return SelectionResult {
                worker: None,
                reason: SelectionReason::AllWorkersBusy,
            };
        }

        return SelectionResult {
            worker: None,
            reason: SelectionReason::AllWorkersBusy,
        };
    }

    let (min_priority, max_priority) = priority_range(&candidates);
    let mut scored: Vec<(Arc<WorkerState>, CircuitState, f64)> =
        Vec::with_capacity(candidates.len());

    for (worker, circuit_state) in candidates.drain(..) {
        let priority_score = normalize_priority(worker.config.priority, min_priority, max_priority);
        let base_score = compute_score(&worker, request, weights, priority_score);
        let final_score = if circuit_state == CircuitState::HalfOpen {
            base_score * weights.half_open_penalty
        } else {
            base_score
        };

        debug!(
            "Worker {} candidate: circuit={:?}, base_score={:.3}, final_score={:.3}, priority_score={:.2}",
            worker.config.id, circuit_state, base_score, final_score, priority_score
        );

        scored.push((worker, circuit_state, final_score));
    }

    // Select worker with highest score
    scored.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(Ordering::Equal));

    let (selected_worker, circuit_state, score) = scored.into_iter().next().unwrap();

    // If selecting a half-open worker, start the probe
    if circuit_state == CircuitState::HalfOpen {
        selected_worker.start_probe(circuit_config).await;
        debug!(
            "Worker {} selected (half-open probe started), score={:.3}",
            selected_worker.config.id, score
        );
    } else {
        debug!(
            "Worker {} selected, score={:.3}",
            selected_worker.config.id, score
        );
    }

    SelectionResult {
        worker: Some(selected_worker),
        reason: SelectionReason::Success,
    }
}

/// Compute a selection score for a worker.
fn compute_score(
    worker: &WorkerState,
    request: &SelectionRequest,
    weights: &SelectionWeights,
    priority_score: f64,
) -> f64 {
    // Slot availability score (0.0-1.0)
    let slot_score = worker.available_slots() as f64 / worker.config.total_slots as f64;

    // Speed score (already 0-100, normalize to 0-1)
    let speed_score = worker.speed_score / 100.0;

    // Locality score (1.0 if project is cached, 0.5 otherwise)
    let locality_score = if worker.has_cached_project(&request.project) {
        1.0
    } else {
        0.5
    };

    // Base weighted score (0.0-1.0)
    let base_score = weights.slots * slot_score
        + weights.speed * speed_score
        + weights.locality * locality_score;

    // Priority acts as a mild multiplier on the base score.
    let priority_factor = 1.0 + (weights.priority * priority_score);
    let score = base_score * priority_factor;

    debug!(
        "Worker {} score: {:.3} (slots: {:.2}, speed: {:.2}, locality: {:.2}, priority: {:.2})",
        worker.config.id, score, slot_score, speed_score, locality_score, priority_score
    );

    score
}

fn priority_range(candidates: &[(Arc<WorkerState>, CircuitState)]) -> (u32, u32) {
    let mut min_priority = u32::MAX;
    let mut max_priority = 0u32;

    for (worker, _) in candidates {
        min_priority = min_priority.min(worker.config.priority);
        max_priority = max_priority.max(worker.config.priority);
    }

    if min_priority == u32::MAX {
        (0, 0)
    } else {
        (min_priority, max_priority)
    }
}

fn normalize_priority(priority: u32, min_priority: u32, max_priority: u32) -> f64 {
    if max_priority == min_priority {
        1.0
    } else {
        (priority.saturating_sub(min_priority)) as f64 / (max_priority - min_priority) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workers::WorkerState;
    use rch_common::WorkerStatus;
    use rch_common::{RequiredRuntime, WorkerConfig, WorkerId};

    fn make_worker(id: &str, total_slots: u32, speed: f64) -> WorkerState {
        let config = WorkerConfig {
            id: WorkerId::new(id),
            host: "localhost".to_string(),
            user: "user".to_string(),
            identity_file: "~/.ssh/id_rsa".to_string(),
            total_slots,
            priority: 100,
            tags: vec![],
        };
        let mut state = WorkerState::new(config);
        state.speed_score = speed;
        state
    }

    #[test]
    fn test_selection_score() {
        let worker = make_worker("test", 16, 80.0);
        let request = SelectionRequest {
            project: "myproject".to_string(),
            command: None,
            estimated_cores: 4,
            preferred_workers: vec![],
            toolchain: None,
            required_runtime: RequiredRuntime::default(),
            classification_duration_us: None,
        };
        let weights = SelectionWeights::default();

        let score = compute_score(&worker, &request, &weights, 0.5);
        assert!(score > 0.0);
        assert!(score <= 1.5);
    }

    #[tokio::test]
    async fn test_select_worker_ignores_unhealthy() {
        let pool = WorkerPool::new();
        pool.add_worker(make_worker("healthy", 8, 50.0).config)
            .await;
        pool.add_worker(make_worker("unreachable", 16, 90.0).config)
            .await;

        // Mark the second worker unreachable
        pool.set_status(&WorkerId::new("unreachable"), WorkerStatus::Unreachable)
            .await;

        let request = SelectionRequest {
            project: "myproject".to_string(),
            command: None,
            estimated_cores: 2,
            preferred_workers: vec![],
            toolchain: None,
            required_runtime: RequiredRuntime::default(),
            classification_duration_us: None,
        };
        let weights = SelectionWeights::default();

        let selected = select_worker(&pool, &request, &weights).await;
        let selected = selected.expect("Expected a healthy worker to be selected");
        assert_eq!(selected.config.id.as_str(), "healthy");
    }

    #[tokio::test]
    async fn test_select_worker_respects_slot_availability() {
        let pool = WorkerPool::new();
        pool.add_worker(make_worker("full", 4, 70.0).config).await;
        pool.add_worker(make_worker("available", 8, 50.0).config)
            .await;

        // Reserve all slots on the first worker
        let full_worker = pool.get(&WorkerId::new("full")).await.unwrap();
        assert!(full_worker.reserve_slots(4));
        assert_eq!(full_worker.available_slots(), 0);

        let request = SelectionRequest {
            project: "myproject".to_string(),
            command: None,
            estimated_cores: 2,
            preferred_workers: vec![],
            toolchain: None,
            required_runtime: RequiredRuntime::default(),
            classification_duration_us: None,
        };
        let weights = SelectionWeights::default();

        let selected = select_worker(&pool, &request, &weights).await;
        let selected = selected.expect("Expected a worker with available slots");
        assert_eq!(selected.config.id.as_str(), "available");
    }

    #[tokio::test]
    async fn test_select_worker_ignores_open_circuit() {
        let pool = WorkerPool::new();
        pool.add_worker(make_worker("closed", 8, 50.0).config).await;
        pool.add_worker(make_worker("open", 16, 90.0).config).await;

        // Open the circuit on the second worker
        let open_worker = pool.get(&WorkerId::new("open")).await.unwrap();
        open_worker.open_circuit().await;

        let request = SelectionRequest {
            project: "myproject".to_string(),
            command: None,
            estimated_cores: 2,
            preferred_workers: vec![],
            toolchain: None,
            required_runtime: RequiredRuntime::default(),
            classification_duration_us: None,
        };
        let weights = SelectionWeights::default();
        let config = CircuitBreakerConfig::default();

        let result = select_worker_with_config(&pool, &request, &weights, &config).await;
        let selected = result.worker.expect("Expected a worker to be selected");
        assert_eq!(result.reason, SelectionReason::Success);
        assert_eq!(selected.config.id.as_str(), "closed");
    }

    #[tokio::test]
    async fn test_select_worker_returns_all_circuits_open() {
        let pool = WorkerPool::new();
        pool.add_worker(make_worker("open1", 8, 50.0).config).await;
        pool.add_worker(make_worker("open2", 16, 90.0).config).await;

        // Open all circuits
        let worker1 = pool.get(&WorkerId::new("open1")).await.unwrap();
        let worker2 = pool.get(&WorkerId::new("open2")).await.unwrap();
        worker1.open_circuit().await;
        worker2.open_circuit().await;

        let request = SelectionRequest {
            project: "myproject".to_string(),
            command: None,
            estimated_cores: 2,
            preferred_workers: vec![],
            toolchain: None,
            required_runtime: RequiredRuntime::default(),
            classification_duration_us: None,
        };
        let weights = SelectionWeights::default();
        let config = CircuitBreakerConfig::default();

        let result = select_worker_with_config(&pool, &request, &weights, &config).await;
        assert!(result.worker.is_none());
        assert_eq!(result.reason, SelectionReason::AllCircuitsOpen);
    }

    #[tokio::test]
    async fn test_select_worker_allows_half_open_with_probe_budget() {
        let pool = WorkerPool::new();
        pool.add_worker(make_worker("half_open", 8, 50.0).config)
            .await;

        // Put worker in half-open state
        let worker = pool.get(&WorkerId::new("half_open")).await.unwrap();
        worker.open_circuit().await;
        worker.half_open_circuit().await;

        let request = SelectionRequest {
            project: "myproject".to_string(),
            command: None,
            estimated_cores: 2,
            preferred_workers: vec![],
            toolchain: None,
            required_runtime: RequiredRuntime::default(),
            classification_duration_us: None,
        };
        let weights = SelectionWeights::default();
        let config = CircuitBreakerConfig {
            half_open_max_probes: 1,
            ..Default::default()
        };

        let result = select_worker_with_config(&pool, &request, &weights, &config).await;
        let selected = result
            .worker
            .expect("Expected half-open worker to be selected");
        assert_eq!(result.reason, SelectionReason::Success);
        assert_eq!(selected.config.id.as_str(), "half_open");
    }

    #[tokio::test]
    async fn test_select_worker_excludes_half_open_at_probe_limit() {
        let pool = WorkerPool::new();
        pool.add_worker(make_worker("half_open", 8, 50.0).config)
            .await;
        pool.add_worker(make_worker("closed", 4, 40.0).config).await;

        // Put worker in half-open state and exhaust probe budget
        let half_open_worker = pool.get(&WorkerId::new("half_open")).await.unwrap();
        half_open_worker.open_circuit().await;
        half_open_worker.half_open_circuit().await;

        let config = CircuitBreakerConfig {
            half_open_max_probes: 1,
            ..Default::default()
        };

        // Start a probe to exhaust the budget
        half_open_worker.start_probe(&config).await;

        let request = SelectionRequest {
            project: "myproject".to_string(),
            command: None,
            estimated_cores: 2,
            preferred_workers: vec![],
            toolchain: None,
            required_runtime: RequiredRuntime::default(),
            classification_duration_us: None,
        };
        let weights = SelectionWeights::default();

        let result = select_worker_with_config(&pool, &request, &weights, &config).await;
        let selected = result
            .worker
            .expect("Expected closed worker to be selected");
        assert_eq!(result.reason, SelectionReason::Success);
        // Should select the closed worker since half-open is at probe limit
        assert_eq!(selected.config.id.as_str(), "closed");
    }

    #[tokio::test]
    async fn test_select_worker_prefers_closed_over_half_open() {
        let pool = WorkerPool::new();
        pool.add_worker(make_worker("half_open", 16, 90.0).config)
            .await;
        pool.add_worker(make_worker("closed", 8, 50.0).config).await;

        // Put first worker in half-open state (normally would be preferred due to higher speed)
        let half_open_worker = pool.get(&WorkerId::new("half_open")).await.unwrap();
        half_open_worker.open_circuit().await;
        half_open_worker.half_open_circuit().await;

        let request = SelectionRequest {
            project: "myproject".to_string(),
            command: None,
            estimated_cores: 2,
            preferred_workers: vec![],
            toolchain: None,
            required_runtime: RequiredRuntime::default(),
            classification_duration_us: None,
        };
        let weights = SelectionWeights::default();
        let config = CircuitBreakerConfig::default();

        let result = select_worker_with_config(&pool, &request, &weights, &config).await;
        let selected = result.worker.expect("Expected a worker to be selected");
        assert_eq!(result.reason, SelectionReason::Success);
        // Should prefer closed worker due to half-open penalty
        assert_eq!(selected.config.id.as_str(), "closed");
    }

    #[tokio::test]
    async fn test_select_worker_prefers_preferred_list() {
        let pool = WorkerPool::new();
        pool.add_worker(make_worker("preferred", 8, 30.0).config)
            .await;
        pool.add_worker(make_worker("other", 8, 90.0).config).await;

        let request = SelectionRequest {
            project: "myproject".to_string(),
            command: None,
            estimated_cores: 2,
            preferred_workers: vec![WorkerId::new("preferred")],
            toolchain: None,
            required_runtime: RequiredRuntime::default(),
            classification_duration_us: None,
        };
        let weights = SelectionWeights::default();
        let config = CircuitBreakerConfig::default();

        let result = select_worker_with_config(&pool, &request, &weights, &config).await;
        let selected = result.worker.expect("Expected a worker to be selected");
        assert_eq!(selected.config.id.as_str(), "preferred");
    }

    #[tokio::test]
    async fn test_select_worker_falls_back_when_preferred_unavailable() {
        let pool = WorkerPool::new();
        pool.add_worker(make_worker("available", 8, 60.0).config)
            .await;

        let request = SelectionRequest {
            project: "myproject".to_string(),
            command: None,
            estimated_cores: 2,
            preferred_workers: vec![WorkerId::new("missing")],
            toolchain: None,
            required_runtime: RequiredRuntime::default(),
            classification_duration_us: None,
        };
        let weights = SelectionWeights::default();
        let config = CircuitBreakerConfig::default();

        let result = select_worker_with_config(&pool, &request, &weights, &config).await;
        let selected = result.worker.expect("Expected fallback to eligible worker");
        assert_eq!(selected.config.id.as_str(), "available");
    }

    #[tokio::test]
    async fn test_select_worker_prefers_higher_priority() {
        let pool = WorkerPool::new();
        let mut high = make_worker("high", 8, 70.0);
        high.config.priority = 200;
        let mut low = make_worker("low", 8, 70.0);
        low.config.priority = 50;

        pool.add_worker(high.config).await;
        pool.add_worker(low.config).await;

        let request = SelectionRequest {
            project: "myproject".to_string(),
            command: None,
            estimated_cores: 2,
            preferred_workers: vec![],
            toolchain: None,
            required_runtime: RequiredRuntime::default(),
            classification_duration_us: None,
        };
        let weights = SelectionWeights::default();
        let config = CircuitBreakerConfig::default();

        let result = select_worker_with_config(&pool, &request, &weights, &config).await;
        let selected = result.worker.expect("Expected a worker to be selected");
        assert_eq!(selected.config.id.as_str(), "high");
    }

    #[tokio::test]
    async fn test_half_open_penalty_applied() {
        // Test that the half-open penalty is correctly applied
        let weights = SelectionWeights {
            slots: 0.0,
            speed: 1.0,
            locality: 0.0,
            priority: 0.0,
            half_open_penalty: 0.5,
        };

        // Worker with 80% speed score
        // Closed: 0.8 (80/100)
        // Half-open: 0.8 * 0.5 = 0.4
        let pool = WorkerPool::new();
        pool.add_worker(make_worker("half_open", 16, 80.0).config)
            .await;
        pool.add_worker(make_worker("closed", 8, 50.0).config).await;

        // Put first worker in half-open state
        let half_open_worker = pool.get(&WorkerId::new("half_open")).await.unwrap();
        half_open_worker.open_circuit().await;
        half_open_worker.half_open_circuit().await;

        let request = SelectionRequest {
            project: "myproject".to_string(),
            command: None,
            estimated_cores: 2,
            preferred_workers: vec![],
            toolchain: None,
            required_runtime: RequiredRuntime::default(),
            classification_duration_us: None,
        };
        let config = CircuitBreakerConfig::default();

        let result = select_worker_with_config(&pool, &request, &weights, &config).await;
        let selected = result.worker.expect("Expected a worker to be selected");
        // closed worker has 50/100 = 0.5 score
        // half-open worker has 80/100 * 0.5 = 0.4 score
        // So closed should win
        assert_eq!(selected.config.id.as_str(), "closed");
    }
}
