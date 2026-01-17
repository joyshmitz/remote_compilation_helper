# Claude Code Hook Integration

## How RCH Hooks Work

RCH uses Claude Code's PreToolUse hook to intercept compilation commands:

```
Claude Code Agent
       │
       ▼
 ┌─────────────────┐
 │  PreToolUse     │◄── rch binary receives JSON
 │  Hook           │
 └────────┬────────┘
          │
          ▼
   ┌──────────────┐
   │ Is it Bash?  │──No──► Pass through
   └──────┬───────┘
          │Yes
          ▼
   ┌──────────────┐
   │ Compilation? │──No──► Pass through
   └──────┬───────┘
          │Yes
          ▼
   ┌──────────────┐
   │ Execute      │
   │ Remotely     │
   └──────┬───────┘
          │
          ▼
   Return artifacts locally
   (agent sees local results)
```

## Hook Installation

```bash
# Install hook (modifies ~/.claude/settings.json)
rch hook install

# What this adds:
# {
#   "hooks": {
#     "PreToolUse": [
#       {
#         "matcher": "Bash",
#         "command": "/path/to/rch hook"
#       }
#     ]
#   }
# }
```

## Hook Protocol

RCH receives JSON on stdin:

```json
{
  "tool": "Bash",
  "input": {
    "command": "cargo build --release"
  }
}
```

RCH responds with one of:

**Allow (pass through):**
```json
{"allow": true}
```

**Intercept and handle:**
```json
{
  "allow": true,
  "output": "   Compiling myproject v0.1.0\n    Finished release [optimized] target(s) in 45.2s"
}
```

**Block (rarely used):**
```json
{
  "allow": false,
  "reason": "Worker unavailable"
}
```

## Command Classification

RCH uses a 5-tier classification system:

| Tier | Time | Purpose |
|------|------|---------|
| 1 | <100μs | Keyword bloom filter |
| 2 | <200μs | Quick regex scan |
| 3 | <500μs | Full command parse |
| 4 | <1ms | Context extraction |
| 5 | <5ms | Worker selection |

### Intercepted Commands

```rust
// Rust
"cargo build", "cargo test", "cargo check", "cargo run", "rustc"

// TypeScript/Bun
"bun test", "bun typecheck"

// C/C++
"gcc", "g++", "clang", "clang++", "cc"

// Build systems
"make", "cmake --build", "ninja", "meson compile"
```

### Never Intercepted

```rust
// Package management (modifies local node_modules)
"bun install", "bun add", "bun remove"

// Local execution (needs local ports/env)
"bun run", "bun dev", "bun build"

// Piped/redirected commands
"cargo build | tee log"
"cargo build > output.txt"

// Background commands
"cargo build &"
```

## Manual Hook Testing

Test the hook directly:

```bash
# Test with compilation command
echo '{"tool":"Bash","input":{"command":"cargo build"}}' | rch hook
# Should return {"allow":true,"output":"..."}

# Test with non-compilation
echo '{"tool":"Bash","input":{"command":"ls -la"}}' | rch hook
# Should return {"allow":true}

# Dry run mode (logs but doesn't execute remotely)
RCH_DRY_RUN=1 rch hook < test-input.json
```

## Hook Configuration

In `~/.config/rch/config.toml`:

```toml
[hook]
# Timeout for classification (ms)
classify_timeout_ms = 5

# Timeout for full pipeline (seconds)
pipeline_timeout_s = 300

# Fail open on errors (allow local execution)
fail_open = true

# Commands to always run locally
local_patterns = [
    "cargo fmt",
    "cargo doc"
]
```

## Debugging Hooks

```bash
# Enable hook debug logging
export RCH_LOG=debug

# Run a command - logs will show hook decisions
cargo build

# Or trace every step
RCH_LOG=trace cargo check

# Check hook is being invoked
ps aux | grep rch
```

## Uninstalling Hook

```bash
# Remove hook from settings
rch hook uninstall

# Manual removal
# Edit ~/.claude/settings.json and remove the PreToolUse entry
```

## Security Considerations

- Hook runs with user permissions
- SSH keys should use ssh-agent (not plaintext paths)
- Workers should be trusted machines you control
- RCH never modifies source code, only reads and transfers
- Artifacts are transferred back via secure rsync
