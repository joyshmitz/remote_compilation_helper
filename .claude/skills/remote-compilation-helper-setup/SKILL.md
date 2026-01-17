---
name: remote-compilation-helper-setup
description: >-
  Set up and troubleshoot RCH (Remote Compilation Helper). Use when installing RCH,
  configuring workers, diagnosing connection issues, or setting up Claude Code hooks.
---

# Remote Compilation Helper Setup

Automated setup and troubleshooting for RCH - transparent compilation offloading.

## Table of Contents

- [Quick Start](#quick-start) - Run diagnostics and initialize
- [Setup Workflow](#setup-workflow) - Step-by-step checklist
- [Worker Configuration](#worker-configuration) - Add remote machines
- [Troubleshooting](#troubleshooting) - Common issues and fixes
- [Claude Code Hook](#claude-code-hook-setup) - Hook installation
- [References](#references) - Deep dives

## Quick Start

```bash
# 1. Run diagnostics (shows what's missing)
rch doctor

# 2. Auto-fix common issues
rch doctor --fix

# 3. Initialize interactively (if not yet configured)
rch init
```

## Setup Workflow

- [ ] **Prerequisites**: Rust nightly, rsync, zstd, ssh-agent
- [ ] **Install RCH**: `cargo install --path rch`
- [ ] **Configure workers**: `~/.config/rch/workers.toml`
- [ ] **Install Claude Code hook**: `rch hook install`
- [ ] **Start daemon**: `rchd` (or systemd service)
- [ ] **Verify**: `rch doctor` shows all green

## Worker Configuration

Workers are remote machines with SSH access. Minimal config:

```toml
# ~/.config/rch/workers.toml
[[workers]]
id = "worker1"
host = "192.168.1.100"
user = "ubuntu"
identity_file = "~/.ssh/id_ed25519"
total_slots = 8      # CPU cores available
priority = 100       # Higher = preferred
tags = ["rust"]      # Optional capability tags
```

### Auto-Discover Workers from SSH Config

```bash
# RCH can discover workers from ~/.ssh/config
rch workers discover --from-ssh-config

# Preview what would be added
rch workers discover --from-ssh-config --dry-run
```

SSH config entries become workers if they have:
- `HostName` (IP or domain)
- `User`
- `IdentityFile`

### Test Worker Connectivity

```bash
# Test single worker
rch workers probe worker1 --verbose

# Test all workers
rch workers probe --all

# Show worker capabilities (toolchains detected)
rch workers list --capabilities
```

## Troubleshooting

### `rch doctor` Checks

| Check | Issue | Fix |
|-------|-------|-----|
| `prerequisites` | Missing tools | Install rsync, zstd, ssh-agent |
| `config` | No config file | `rch init` or create manually |
| `ssh_keys` | Agent not running | `eval $(ssh-agent) && ssh-add` |
| `daemon` | rchd not running | `rchd` or `systemctl start rchd` |
| `hooks` | Hook not installed | `rch hook install` |
| `workers` | No reachable workers | Check SSH, firewall, worker config |

### Common Issues

**"Connection refused" to worker:**
```bash
# Check SSH directly
ssh -i ~/.ssh/key worker1 "echo ok"

# Check if worker has required tools
ssh worker1 "which rustc rsync zstd"
```

**"Hook not triggering":**
```bash
# Verify hook is installed
cat ~/.claude/settings.json | grep -A5 PreToolUse

# Reinstall hook
rch hook install --force
```

**"Daemon not responding":**
```bash
# Check socket
ls -la /tmp/rch.sock

# Restart daemon
pkill rchd && rchd
```

## Claude Code Hook Setup

RCH integrates via PreToolUse hook:

```bash
# Install hook (updates ~/.claude/settings.json)
rch hook install

# Verify installation
rch hook status

# Test interception (dry run)
RCH_DRY_RUN=1 cargo check
```

## Validation

```bash
# Full diagnostic
rch doctor --verbose

# JSON output for scripting
rch doctor --json

# Expected: all checks pass
rch doctor && echo "RCH ready!"
```

## References

- [WORKERS.md](references/WORKERS.md) - Advanced worker configuration
- [TROUBLESHOOTING.md](references/TROUBLESHOOTING.md) - Detailed diagnostics
- [HOOKS.md](references/HOOKS.md) - Claude Code hook internals
