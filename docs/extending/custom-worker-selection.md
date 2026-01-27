# Custom Worker Selection

This guide explains how to customize RCH's worker selection algorithm.

## Overview

Worker selection determines which remote worker handles a compilation request. RCH provides **five built-in selection strategies** with configurable weights.

### Selection Strategies

| Strategy | Best For | Description |
|----------|----------|-------------|
| **Priority** | Backwards compatibility, manual control | Sort by priority, select first available |
| **Fastest** | Performance-critical builds, homogeneous workers | Select worker with highest SpeedScore |
| **Balanced** | Default recommendation, diverse worker pools | Balance SpeedScore, load, health, cache affinity |
| **CacheAffinity** | Incremental builds, large codebases | Prefer workers with warm caches for the project |
| **FairFastest** | Many workers, avoid hot-spotting | Weighted random selection favoring fast workers |

### Default Weights (Balanced Strategy)

The balanced strategy uses these default weights:
- Speed score: 0.5
- Available slots: 0.4
- Health/success rate: 0.3
- Cache affinity: 0.2
- Network latency: 0.1
- Worker priority: 0.1
- Half-open circuit penalty: 0.5 (50% score reduction)

## Selection Algorithm

### Filtering (All Strategies)

Workers are filtered before scoring:
1. Circuit breaker must be closed (or half-open with probe budget)
2. Worker must have at least one available slot
3. Worker must have required runtime (Rust, Bun, etc.)
4. Worker must not be draining

## Customization Options

### Strategy Selection

Choose your selection strategy in the configuration:

```toml
# ~/.config/rch/config.toml
[selection]
strategy = "balanced"  # priority | fastest | balanced | cache_affinity | fair_fastest
```

### Weight Customization (Balanced Strategy)

Adjust weights to tune the balanced strategy:

```toml
[selection.weights]
speedscore = 0.5       # Weight for SpeedScore (performance)
slots = 0.4            # Weight for available capacity
health = 0.3           # Weight for health/success rate
cache = 0.2            # Weight for cache affinity
network = 0.1          # Weight for network latency
priority = 0.1         # Weight for worker priority
half_open_penalty = 0.5  # Penalty multiplier for half-open circuit breakers

# Example: Prioritize cache for incremental builds
# speedscore = 0.3
# slots = 0.2
# health = 0.2
# cache = 0.5
# network = 0.1
# priority = 0.0
```

### Project-Level Preferences

Specify preferred workers per project:

```toml
# .rch/config.toml
[general]
preferred_workers = ["fast-worker", "local-worker"]
```

### Tag-Based Selection

Filter workers by tags:

```toml
# Worker definition
[[workers]]
id = "gpu-worker"
tags = ["gpu", "cuda"]

# Project config - require GPU
[selection]
required_tags = ["gpu"]

# Or prefer SSD workers
[selection]
preferred_tags = ["ssd"]
```

## Built-in Selection Strategies

### Priority Strategy

The original selection behavior for backwards compatibility:

```rust
// Sort workers by priority, select first with available slots
fn select_by_priority(workers: &[&WorkerState]) -> Option<WorkerId> {
    workers.iter()
        .min_by_key(|w| w.priority)
        .map(|w| w.id.clone())
}
```

### Fastest Strategy

Select the worker with the highest SpeedScore:

```rust
fn select_by_speedscore(workers: &[&WorkerState]) -> Option<WorkerId> {
    workers.iter()
        .max_by(|a, b| {
            let score_a = a.speedscore.unwrap_or(50.0);
            let score_b = b.speedscore.unwrap_or(50.0);
            score_a.partial_cmp(&score_b).unwrap_or(Ordering::Equal)
        })
        .map(|w| w.id.clone())
}
```

### Balanced Strategy (Recommended)

Computes an effective score balancing multiple factors:

```rust
fn compute_balanced_score(worker: &WorkerState, request: &SelectionRequest) -> f64 {
    // Score components (all normalized to 0-1)
    let speed_score = worker.get_speed_score() / 100.0;
    let slot_score = worker.available_slots() as f64 / worker.total_slots() as f64;
    let load_factor = 1.0 - (utilization * 0.5);  // Penalize heavily loaded workers
    let health_score = 1.0 - worker.error_rate();
    let cache_score = cache_tracker.estimate_warmth(&worker.id, &request.project);
    let network_score = network_tracker.get_score(&worker.id);
    let priority_score = normalize_priority(worker.priority, min_priority, max_priority);

    // Apply configurable weights
    let base_score = weights.speedscore * speed_score
        + weights.slots * slot_score * load_factor
        + weights.health * health_score
        + weights.cache * cache_score
        + weights.network * network_score
        + weights.priority * priority_score;

    // Apply half-open penalty if applicable
    if circuit_state == CircuitState::HalfOpen {
        base_score * weights.half_open_penalty
    } else {
        base_score
    }
}
```

### CacheAffinity Strategy

Prioritizes workers with warm caches:

- Build < 1 hour ago: 1.0 warmth
- Build 1-6 hours ago: 0.5-1.0 (linear decay)
- Build 6-24 hours ago: 0.2-0.5 (linear decay)
- Build > 24 hours ago: 0.0

Test completions receive a 1.5x boost since test dependencies are warmer.

### FairFastest Strategy

Weighted random selection that favors fast workers while ensuring all workers get some load:

```rust
fn select_fair_fastest(workers: &[&WorkerState], history: &SelectionHistory) -> Option<WorkerId> {
    let weights: Vec<f64> = workers.iter().map(|w| {
        let speed = w.speedscore.unwrap_or(50.0);
        let recent = history.recent_selections(&w.id, Duration::from_secs(300));
        // Reduce weight for recently selected workers
        speed / (1.0 + recent as f64)
    }).collect();

    // Weighted random selection
    weighted_random_select(workers, &weights)
}
```

This prevents "hot-spotting" where the fastest worker becomes overloaded.

## Advanced Customizations

### Time-Based Selection

Prefer different workers at different times:

```rust
pub struct TimeBasedStrategy {
    daytime_workers: Vec<WorkerId>,
    nighttime_workers: Vec<WorkerId>,
}

impl SelectionStrategy for TimeBasedStrategy {
    fn select(&self, workers: &[Arc<WorkerState>], request: &SelectionRequest) -> Option<SelectedWorker> {
        let hour = chrono::Local::now().hour();
        let preferred = if hour >= 9 && hour < 18 {
            &self.daytime_workers  // Office hours: use cloud workers
        } else {
            &self.nighttime_workers  // Off hours: use office machines
        };

        // Filter to preferred, then select by load
        workers
            .iter()
            .filter(|w| w.can_accept_build() && preferred.contains(&w.id))
            .max_by_key(|w| w.available_slots())
            .map(|w| w.to_selected())
            .or_else(|| {
                // Fallback to any available worker
                workers.iter()
                    .filter(|w| w.can_accept_build())
                    .max_by_key(|w| w.available_slots())
                    .map(|w| w.to_selected())
            })
    }
}
```

### Cost-Aware Selection

If workers have different costs:

```rust
pub struct CostAwareStrategy {
    max_cost_per_build: f64,
}

impl SelectionStrategy for CostAwareStrategy {
    fn select(&self, workers: &[Arc<WorkerState>], request: &SelectionRequest) -> Option<SelectedWorker> {
        // Estimate build cost based on worker rate and estimated duration
        let estimated_duration = estimate_build_duration(&request);

        workers
            .iter()
            .filter(|w| {
                w.can_accept_build() &&
                w.cost_per_hour() * estimated_duration.as_secs_f64() / 3600.0 <= self.max_cost_per_build
            })
            .min_by(|a, b| {
                let cost_a = a.cost_per_hour();
                let cost_b = b.cost_per_hour();
                cost_a.partial_cmp(&cost_b).unwrap()
            })
            .map(|w| w.to_selected())
    }
}
```

Worker config with cost:
```toml
[[workers]]
id = "cheap-worker"
cost_per_hour = 0.05

[[workers]]
id = "fast-worker"
cost_per_hour = 0.50
```

### Locality-Aware Selection

Prefer workers in same region:

```rust
pub struct LocalityStrategy {
    local_region: String,
}

impl SelectionStrategy for LocalityStrategy {
    fn select(&self, workers: &[Arc<WorkerState>], request: &SelectionRequest) -> Option<SelectedWorker> {
        // Tier 1: Same region
        let local = workers.iter()
            .filter(|w| w.can_accept_build() && w.region == self.local_region);

        if let Some(worker) = local.max_by_key(|w| w.available_slots()) {
            return Some(worker.to_selected());
        }

        // Tier 2: Any region
        workers.iter()
            .filter(|w| w.can_accept_build())
            .max_by_key(|w| w.available_slots())
            .map(|w| w.to_selected())
    }
}
```

## Testing Selection Strategies

### Unit Tests

The actual selection tests use `WorkerSelector` and `WorkerPool`:

```rust
#[tokio::test]
async fn test_cache_affinity_prefers_warm_cache() {
    let pool = WorkerPool::new();

    // Add a worker with warm cache
    pool.add_worker(make_worker("warm-cache", 8, 50.0).config.read().await.clone())
        .await;
    pool.add_worker(make_worker("cold-cache", 8, 80.0).config.read().await.clone())
        .await;

    let selector = WorkerSelector::new();

    // Record recent build on warm-cache worker
    selector.record_build("warm-cache", "my-project", false).await;

    let request = SelectionRequest {
        project: "my-project".to_string(),
        ..Default::default()
    };

    let config = SelectionConfig {
        strategy: SelectionStrategy::CacheAffinity,
        ..Default::default()
    };

    let result = selector.select(&pool, &request, &config).await;

    // Should prefer warm-cache despite lower SpeedScore
    assert_eq!(result.selected_worker.unwrap().id, "warm-cache");
}

#[tokio::test]
async fn test_fair_fastest_distributes_load() {
    let pool = WorkerPool::new();
    pool.add_worker(make_worker("worker1", 8, 80.0).config.read().await.clone())
        .await;
    pool.add_worker(make_worker("worker2", 8, 80.0).config.read().await.clone())
        .await;

    let selector = WorkerSelector::new();
    let config = SelectionConfig {
        strategy: SelectionStrategy::FairFastest,
        ..Default::default()
    };

    let mut selections: HashMap<String, u32> = HashMap::new();
    for _ in 0..100 {
        let request = SelectionRequest::default();
        let result = selector.select(&pool, &request, &config).await;
        if let Some(worker) = result.selected_worker {
            *selections.entry(worker.id).or_insert(0) += 1;
        }
    }

    // Both workers should get selections (weighted random distribution)
    assert!(selections.get("worker1").unwrap_or(&0) > &20);
    assert!(selections.get("worker2").unwrap_or(&0) > &20);
}
```

### Integration Tests

```bash
# Test with mock workers
RCH_MOCK_SSH=1 cargo test -p rchd selection
```

### A/B Testing Strategies

Compare strategies in production:

```toml
[selection]
strategy = "experiment"

[selection.experiment]
control = "balanced"
treatment = "cache_affinity"
treatment_percent = 20
metrics_endpoint = "http://metrics.example.com/api"
```

> **Note:** Experimental strategy switching is a planned feature. Currently, switch strategies by changing the `strategy` field in your config.
