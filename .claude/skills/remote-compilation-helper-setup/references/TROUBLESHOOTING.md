# RCH Troubleshooting Guide

## Diagnostic Command

Always start here:

```bash
rch doctor --verbose
```

This checks:
- Prerequisites (rsync, zstd, ssh-agent)
- Configuration files
- SSH key availability
- Daemon status
- Hook installation
- Worker connectivity

## Issue Categories

### 1. Installation Issues

**Rust nightly not installed:**
```bash
# Install rustup if missing
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install nightly
rustup install nightly
rustup default nightly
```

**Build fails with edition 2024 error:**
```bash
# Ensure nightly is recent enough
rustup update nightly

# Check version (need 1.82+)
rustc +nightly --version
```

**Missing rsync/zstd:**
```bash
# Ubuntu/Debian
sudo apt install rsync zstd

# macOS
brew install rsync zstd

# Fedora
sudo dnf install rsync zstd
```

### 2. SSH Issues

**"Permission denied (publickey)":**
```bash
# Check key exists
ls -la ~/.ssh/

# Check key permissions (must be 600)
chmod 600 ~/.ssh/id_*

# Test SSH directly
ssh -vvv -i ~/.ssh/key user@host

# Add key to agent
eval $(ssh-agent)
ssh-add ~/.ssh/your_key
```

**"Connection refused":**
```bash
# Check worker is reachable
ping worker-host

# Check SSH port is open
nc -zv worker-host 22

# Check firewall on worker
ssh worker "sudo ufw status"
```

**"Host key verification failed":**
```bash
# Add host to known_hosts
ssh-keyscan worker-host >> ~/.ssh/known_hosts

# Or connect once interactively
ssh user@worker-host
```

### 3. Configuration Issues

**Config file not found:**
```bash
# Create config directory
mkdir -p ~/.config/rch

# Initialize with wizard
rch init

# Or create manually
cat > ~/.config/rch/workers.toml << 'EOF'
[[workers]]
id = "worker1"
host = "192.168.1.100"
user = "ubuntu"
identity_file = "~/.ssh/id_ed25519"
total_slots = 8
priority = 100
EOF
```

**Invalid TOML syntax:**
```bash
# Validate config
rch config check

# Common issues:
# - Missing quotes around strings
# - Wrong array syntax (use [[workers]] not [workers])
# - Typos in field names
```

### 4. Daemon Issues

**Daemon not running:**
```bash
# Start daemon in foreground (for debugging)
rchd --foreground --verbose

# Start as background process
rchd &

# Or use systemd
systemctl --user start rchd
```

**Socket permission denied:**
```bash
# Check socket
ls -la /tmp/rch.sock

# Remove stale socket
rm /tmp/rch.sock

# Restart daemon
rchd
```

**Daemon crashes:**
```bash
# Check logs
journalctl --user -u rchd -f

# Or with foreground mode
rchd --foreground --log-level=debug
```

### 5. Hook Issues

**Hook not triggering:**
```bash
# Check hook is registered
cat ~/.claude/settings.json | jq '.hooks.PreToolUse'

# Reinstall hook
rch hook install --force

# Test with dry run
RCH_DRY_RUN=1 cargo check
```

**Hook returns error JSON:**
```bash
# Check hook binary
which rch

# Test hook directly
echo '{"tool":"Bash","input":{"command":"cargo check"}}' | rch hook

# Enable debug logging
RCH_LOG=debug cargo check
```

**Commands not being intercepted:**
```bash
# Verify command is supported
rch classify "cargo build"

# Check if in NEVER_INTERCEPT list
# (bun install, npm, etc. always run locally)

# Check for piped/backgrounded (not intercepted)
# cargo build | tee log  # NOT intercepted
# cargo build &          # NOT intercepted
```

### 6. Worker Issues

**No workers available:**
```bash
# Check worker status
rch workers status

# Probe connectivity
rch workers probe --all

# Check slots
rch workers list --verbose
```

**Worker missing toolchain:**
```bash
# SSH to worker and check
ssh worker1 "which rustc cargo bun gcc"

# Install on worker
ssh worker1 "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
```

**Transfer failures:**
```bash
# Test rsync directly
rsync -avz --dry-run ./src/ worker1:/tmp/test/

# Check disk space on worker
ssh worker1 "df -h /tmp"

# Check rsync version compatibility
rsync --version
ssh worker1 "rsync --version"
```

### 7. Performance Issues

**Compilation slower than local:**
```bash
# Check worker load
ssh worker1 "uptime; top -bn1 | head -20"

# Check network latency
ping -c 5 worker1

# Profile transfer time
time rsync -avz ./src/ worker1:/tmp/test/

# Consider: is project too small to benefit?
# RCH overhead ~100-500ms, only helps if compile >2s
```

**High transfer overhead:**
```bash
# Check what's being transferred
RCH_LOG=debug cargo build 2>&1 | grep transfer

# Ensure target/ is excluded
cat .rchignore  # Should have target/, node_modules/, .git/

# Use incremental sync
# (default, but verify it's not doing full copies)
```

## Debug Mode

Enable verbose logging:

```bash
# Environment variable
export RCH_LOG=debug

# Or per-command
RCH_LOG=trace cargo build
```

Log levels: `error`, `warn`, `info`, `debug`, `trace`

## Getting Help

```bash
# Show version
rch --version

# Built-in help
rch --help
rch doctor --help
rch workers --help

# Generate diagnostic report
rch doctor --json > rch-diagnostic.json
```
