# Command Reference

## Core Commands

| Command | Purpose | Common Flags |
|---------|---------|--------------|
| `rch doctor` | Diagnose issues | `--fix`, `--verbose`, `--json` |
| `rch status` | Daemon status | `--json` |
| `rch workers probe` | Test workers | `--all`, `-v`, `worker_id` |
| `rch workers list` | List workers | `--capabilities`, `--json` |
| `rch workers discover` | Auto-find workers | `--from-ssh-config`, `--dry-run` |
| `rch hook install` | Setup Claude hook | `--force` |
| `rch hook status` | Check hook | — |
| `rch config show` | Show config | — |
| `rch config check` | Validate config | — |

## Daemon Commands

```bash
# Start (choose one)
rchd &                           # Background
rchd --foreground                # Foreground (for debugging)
systemctl --user start rchd      # Systemd (Linux)
launchctl load ~/Library/LaunchAgents/com.rch.daemon.plist  # macOS

# Stop
systemctl --user stop rchd
launchctl unload ~/Library/LaunchAgents/com.rch.daemon.plist

# Logs
journalctl --user -u rchd -f     # Linux
tail -f ~/.config/rch/logs/daemon.log  # macOS
```

## Debug Commands

```bash
# Verbose diagnostics
rch doctor --verbose

# Export diagnostics
rch doctor --json > diagnostic.json

# Test hook directly
echo '{"tool":"Bash","input":{"command":"cargo check"}}' | rch hook

# Dry run (logs without execution)
RCH_DRY_RUN=1 cargo check

# Debug logging
RCH_LOG=debug cargo build
RCH_LOG=trace cargo build  # Maximum detail
```

## Worker Probe Output

```bash
rch workers probe worker1 --verbose
```

Shows:
- SSH connectivity (port 22)
- Detected toolchains (rustc, cargo, bun, gcc, clang)
- Disk space (/tmp)
- System load

## Environment Variables

| Variable | Purpose | Example |
|----------|---------|---------|
| `RCH_LOG` | Log level | `debug`, `trace`, `info` |
| `RCH_DRY_RUN` | No remote execution | `1` |
| `RCH_CONFIG_DIR` | Config location | `~/.config/rch` |
| `RCH_NO_COLOR` | Disable colors | `1` |

## Config Files

| File | Purpose |
|------|---------|
| `~/.config/rch/workers.toml` | Worker definitions |
| `~/.config/rch/daemon.toml` | Daemon settings |
| `~/.config/rch/config.toml` | Hook settings |
| `.rch.toml` (project) | Per-project overrides |
