# Circuit Breaker Guide

RCH uses a circuit breaker per worker to stop routing builds to a flaky machine
and prevent cascading failures. When a worker repeatedly fails health checks or
remote commands, its circuit opens, and the daemon temporarily skips it.

Think of this like an electrical breaker: it trips to protect the system, then
resets after a cooldown to test whether the worker recovered.

## How It Works in RCH

- **Per worker**: Each worker has its own circuit state.
- **Fail-open philosophy**: If something is wrong, RCH avoids that worker and
  falls back to other workers or local execution.
- **Automatic recovery**: After a cooldown, RCH probes the worker before using
  it normally again.

## Circuit States

**Closed**
- Normal operation.
- Worker accepts jobs.

**Open**
- Worker is excluded after repeated failures.
- Cooldown timer runs before probing.

**Half-open**
- Limited probe requests test recovery.
- Success closes the circuit; any failure reopens it.

## State Transitions (Defaults)

```
CLOSED --(3 failures OR >=50% error rate in 60s)--> OPEN
OPEN   --(cooldown 30s elapsed)------------------> HALF-OPEN
HALF   --(2 consecutive successes)--------------> CLOSED
HALF   --(any failure)--------------------------> OPEN
```

## Configuration

Circuit breaker settings live in your config:

- User-level: `~/.config/rch/config.toml`
- Project-level: `.rch/config.toml`

```toml
[circuit]
failure_threshold = 3        # consecutive failures to open
success_threshold = 2        # consecutive successes to close
error_rate_threshold = 0.5   # error rate to open within window
window_secs = 60             # rolling window size
open_cooldown_secs = 30      # cooldown before half-open
half_open_max_probes = 1     # concurrent probes allowed
```

Notes:
- If you lower thresholds too far, brief network blips can open circuits.
- If you raise thresholds too high, failing workers will be retried too often.

## What To Do When a Circuit Opens

1. **Wait for auto-recovery**
   - The default cooldown is 30 seconds.

2. **Check the reason**

```bash
rch status --circuits
rch daemon logs | grep -i circuit
rch workers history my-worker
```

3. **Fix the underlying issue**
   - SSH connectivity: `ssh -v -i ~/.ssh/key user@host "echo OK"`
   - Worker load: check CPU/memory/disk
   - Disk space: `df -h` on the worker

4. **Manually reset after a fix**

```bash
rch workers reset my-worker
rch workers probe my-worker
```

## Related Docs

- Troubleshooting: `docs/TROUBLESHOOTING.md` (Circuit Breaker Open)
- Worker recovery runbook: `docs/runbooks/worker-recovery.md`
- Monitoring guide: `docs/guides/monitoring.md`
