# ADR-003: Circuit Breaker Pattern for Workers

## Status

Accepted

## Context

Workers can fail for various reasons:
- Network issues (temporary or permanent)
- Worker overload
- SSH configuration problems
- Worker software crashes

Without protection, a failing worker causes:
- Repeated connection attempts
- Timeouts slowing all builds
- Cascade failures affecting other workers

## Decision

Implement the circuit breaker pattern for each worker.

**States:**

```
     ┌─────────────────────────────────────────┐
     │                                         │
     ▼                                         │
┌─────────┐   failure threshold   ┌───────┐   │
│ CLOSED  │ ────────────────────▶ │ OPEN  │   │
│         │                       │       │   │
└────┬────┘                       └───┬───┘   │
     │                                │       │
     │ success                        │ timeout
     │                                │       │
     │         ┌───────────┐          │       │
     └─────────│ HALF-OPEN │◀─────────┘       │
               │           │                   │
               └─────┬─────┘                   │
                     │                         │
                     │ success                 │
                     └─────────────────────────┘
```

**Configuration:**

```toml
[circuit_breaker]
failure_threshold = 3      # Failures before opening
success_threshold = 2      # Successes to close from half-open
timeout_secs = 30          # Time before half-open probe
half_open_max_calls = 1    # Concurrent calls in half-open
```

## Consequences

### Positive

- **Fast failure**: Immediately reject requests to broken workers
- **Self-healing**: Automatic recovery when worker comes back
- **Load protection**: Prevents dogpiling on failing workers
- **Graceful degradation**: Other workers continue serving

### Negative

- **Added complexity**: State machine to maintain
- **Potential false positives**: Brief network hiccups can open circuit
- **Recovery delay**: Workers not used immediately after recovery

### Mitigations

- Tune thresholds based on observed failure patterns
- Health checks in half-open state use lightweight probes
- Manual override: `rch workers reset <id>` to force close

## State Transitions

### Closed → Open

Triggered when:
- `failure_count >= failure_threshold`

Actions:
- Stop routing to worker
- Log warning
- Start timeout timer

### Open → Half-Open

Triggered when:
- `timeout_secs` elapsed since opening

Actions:
- Allow limited probe requests
- Reset success/failure counters

### Half-Open → Closed

Triggered when:
- `success_count >= success_threshold` in half-open

Actions:
- Resume normal routing
- Log recovery

### Half-Open → Open

Triggered when:
- Any failure in half-open state

Actions:
- Return to open state
- Reset timeout timer

## Implementation Notes

- Circuit state is per-worker, stored in `WorkerPool`
- Thread-safe updates using atomic operations
- Circuit state survives daemon restart (persisted in state file)

## Alternatives Considered

### Simple Retry with Backoff

- **Pro**: Simpler implementation
- **Con**: Doesn't prevent cascade failures
- **Con**: Slower recovery detection

### Health Check Only (No Circuit)

- **Pro**: Simpler, health checks already exist
- **Con**: Doesn't prevent fast failures during outage
- **Con**: Health checks add load to failing workers

### Per-Request Timeout Only

- **Pro**: Simplest approach
- **Con**: Every request pays timeout cost
- **Con**: No memory of past failures

## References

- `rchd/src/selection.rs` - Circuit breaker integration
- `rchd/src/workers.rs` - Worker state management
- [Circuit Breaker Pattern (Martin Fowler)](https://martinfowler.com/bliki/CircuitBreaker.html)
