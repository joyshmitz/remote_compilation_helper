# ADR-005: Shell Hook Architecture

## Status

Accepted

## Context

RCH needs to intercept compilation commands from AI coding agents transparently. The hook must:
- See every Bash command before execution
- Decide quickly whether to intercept
- Execute remotely if appropriate
- Return results as if local execution occurred
- Never block or slow down non-compilation commands

## Decision

Integrate with Claude Code's PreToolUse hook mechanism.

**Hook registration:**

```json
// In Claude Code hooks configuration
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "command": ["rch"]
      }
    ]
  }
}
```

**Hook protocol:**

1. Claude Code calls `rch` with JSON on stdin
2. `rch` reads JSON, extracts command
3. `rch` classifies command (5-tier system)
4. If compilation: execute remotely, return artifacts
5. `rch` writes JSON response to stdout

**Input format:**

```json
{
  "tool": "Bash",
  "input": {
    "command": "cargo build --release"
  }
}
```

**Output format (allow):**

```json
{
  "decision": "allow",
  "reason": null
}
```

**Output format (handled remotely):**

```json
{
  "decision": "allow",
  "reason": null
}
```

Note: Even when handled remotely, we return "allow" because the command has effectively "executed" (remotely). The agent doesn't need to know.

## Consequences

### Positive

- **Transparent**: Agent never knows command ran remotely
- **Non-blocking**: Non-compilation commands pass through instantly (<1ms)
- **Fail-safe**: On any error, allows local execution
- **Debuggable**: Hook can be tested standalone: `echo '{"tool":"Bash",...}' | rch`
- **Extensible**: Same hook can support other agents

### Negative

- **Agent-specific**: Currently only supports Claude Code
- **JSON overhead**: Parsing adds latency (mitigated by fast serde)
- **Process spawn**: Each command spawns hook process

### Mitigations

- Hook binary is small (<1MB) and starts quickly
- JSON parsing is optimized with serde
- Stateless hook delegates to daemon for heavy lifting

## Hook Flow Diagram

```
Agent Types Command
      │
      ▼
┌──────────────┐
│ Claude Code  │
│   Bash Tool  │
└──────┬───────┘
       │ JSON stdin
       ▼
┌──────────────┐     ┌─────────────────┐
│     rch      │────▶│ Classify (Tier  │
│    (hook)    │     │   0-4, <5ms)    │
└──────┬───────┘     └────────┬────────┘
       │                      │
       │            ┌─────────┴─────────┐
       │            ▼                   ▼
       │    ┌──────────────┐    ┌──────────────┐
       │    │ Not Compile  │    │  Compilation │
       │    └──────┬───────┘    └──────┬───────┘
       │           │                   │
       │    Return │           Query   │
       │    allow  │           daemon  │
       │           │                   │
       │           │            ┌──────▼───────┐
       │           │            │ Select Worker│
       │           │            │ Transfer Code│
       │           │            │ Execute      │
       │           │            │ Return Arts  │
       │           │            └──────┬───────┘
       │           │                   │
       │           │            Return │
       │           │            allow  │
       │           │                   │
       ▼           ▼                   ▼
┌──────────────────────────────────────────────┐
│              JSON stdout (allow)              │
└──────────────────────────────────────────────┘
       │
       ▼
Agent Sees Command "Executed"
```

## Error Handling

All errors result in "allow" to enable local execution:

| Error | Response | Agent Experience |
|-------|----------|------------------|
| Daemon unreachable | allow | Local execution |
| Worker selection failed | allow | Local execution |
| Transfer failed | allow | Local execution |
| Remote execution failed | allow (with error output) | Sees compile errors |
| Classification error | allow | Local execution |

## Alternatives Considered

### Shell Function Override

- **Pro**: Works with any shell
- **Con**: Complex setup per shell (bash, zsh, fish)
- **Con**: Harder to maintain
- **Con**: User might bypass

### LD_PRELOAD Interception

- **Pro**: Catches all process spawns
- **Con**: Complex, fragile
- **Con**: Security concerns
- **Con**: Performance overhead

### Proxy Binary (cargo → rch-cargo)

- **Pro**: Targeted to specific tools
- **Con**: Requires per-tool proxies
- **Con**: PATH manipulation needed
- **Con**: Doesn't work with full paths

### Claude Code Plugin

- **Pro**: Deep integration
- **Con**: Proprietary, limited access
- **Con**: Not portable to other agents

## Installation

```bash
# Auto-detect and install
rch hook install

# Manual installation
rch hook install --agent=claude-code

# Verify
rch hook status
```

## References

- `rch/src/hook.rs` - Hook implementation
- `rch/src/main.rs` - Hook mode entry point
- Claude Code hooks documentation
