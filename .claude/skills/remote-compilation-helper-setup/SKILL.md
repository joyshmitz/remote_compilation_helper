---
name: remote-compilation-helper-setup
description: >-
  Install RCH, configure workers, fix SSH/daemon/hook issues. Use when setting up
  remote compilation, adding build machines, or troubleshooting rch doctor failures.
---

# RCH Setup

## Quick Start

```bash
rch doctor              # Diagnose issues
rch doctor --fix        # Auto-fix common problems
rch init                # Interactive setup wizard
```

## Setup Checklist

- [ ] Prerequisites: `rustup`, `rsync`, `zstd`, `ssh-agent`
- [ ] Install: `cargo install --path rch`
- [ ] Configure: `~/.config/rch/workers.toml`
- [ ] Hook: `rch hook install`
- [ ] Daemon: `rchd &` or `systemctl --user start rchd`
- [ ] Verify: `rch doctor` (all green)

## Worker Config

```toml
# ~/.config/rch/workers.toml
[[workers]]
id = "worker1"
host = "192.168.1.100"
user = "ubuntu"
identity_file = "~/.ssh/id_ed25519"
total_slots = 8    # CPU cores
priority = 100     # Higher = preferred
tags = ["rust"]    # Optional
```

### Discover from SSH Config

```bash
rch workers discover --from-ssh-config --dry-run  # Preview
rch workers discover --from-ssh-config            # Add workers
```

Requires: `HostName`, `User`, `IdentityFile` in SSH config entry.

### Test Workers

```bash
rch workers probe worker1 --verbose    # Single worker
rch workers probe --all                # All workers
rch workers list --capabilities        # Show toolchains
```

## Troubleshooting Quick Reference

| Symptom | Check | Fix |
|---------|-------|-----|
| Missing tools | `which rsync zstd` | `apt install rsync zstd` |
| SSH fails | `ssh -i key host "echo ok"` | `chmod 600 ~/.ssh/*`; `ssh-add` |
| No config | `ls ~/.config/rch/` | `rch init` |
| Daemon down | `ls /tmp/rch.sock` | `rchd &` |
| Hook missing | `grep rch ~/.claude/settings.json` | `rch hook install` |
| No workers | `rch workers status` | Check SSH, firewall, config |

### Common Fixes

```bash
# SSH agent not running
eval $(ssh-agent) && ssh-add ~/.ssh/your_key

# Daemon socket stale
rm /tmp/rch.sock && rchd &

# Hook not triggering
rch hook install --force

# Test hook directly
echo '{"tool":"Bash","input":{"command":"cargo check"}}' | rch hook
```

## Hook Setup

```bash
rch hook install       # Add to ~/.claude/settings.json
rch hook status        # Verify
RCH_DRY_RUN=1 cargo check  # Test without remote execution
```

## Validation

```bash
rch doctor --verbose   # Full diagnostics
rch doctor --json      # For scripting
```

## References

| Topic | Reference |
|-------|-----------|
| Worker schema, selection, tags | [WORKERS.md](references/WORKERS.md) |
| All error symptoms & fixes | [TROUBLESHOOTING.md](references/TROUBLESHOOTING.md) |
| Hook protocol, classification | [HOOKS.md](references/HOOKS.md) |
