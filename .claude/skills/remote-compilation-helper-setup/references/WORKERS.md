# Worker Configuration Reference

## Worker Definition Schema

```toml
[[workers]]
id = "unique-name"           # Required: identifier for this worker
host = "192.168.1.100"       # Required: IP or hostname
user = "ubuntu"              # Required: SSH username
identity_file = "~/.ssh/key" # Required: path to SSH private key
total_slots = 16             # Required: max concurrent jobs (usually CPU cores)
priority = 100               # Optional: selection weight (default: 50)
tags = ["rust", "fast"]      # Optional: capability/group tags
enabled = true               # Optional: disable without removing (default: true)
```

## Worker Selection Algorithm

RCH selects workers based on:

1. **Available slots**: `total_slots - active_jobs`
2. **Priority**: Higher priority preferred when slots equal
3. **Project locality**: Workers with cached project data preferred
4. **Tag matching**: Project can require specific tags

Formula: `score = available_slots * priority * locality_bonus`

## Multi-Worker Example

```toml
# ~/.config/rch/workers.toml

# Primary fast worker - prefer this
[[workers]]
id = "fast-builder"
host = "build-server.local"
user = "build"
identity_file = "~/.ssh/build_key"
total_slots = 48
priority = 100
tags = ["fast", "rust", "bun"]

# Secondary worker - use when primary busy
[[workers]]
id = "backup-builder"
host = "192.168.1.50"
user = "ubuntu"
identity_file = "~/.ssh/id_ed25519"
total_slots = 8
priority = 50
tags = ["rust"]

# Specialized TypeScript worker
[[workers]]
id = "ts-builder"
host = "ts.internal"
user = "node"
identity_file = "~/.ssh/ts_key"
total_slots = 16
priority = 75
tags = ["bun", "typescript"]
```

## Discovery from SSH Config

RCH can auto-discover workers from `~/.ssh/config`:

```bash
# Preview discovered workers
rch workers discover --from-ssh-config --dry-run

# Add discovered workers to config
rch workers discover --from-ssh-config

# Filter by pattern
rch workers discover --from-ssh-config --filter "build*"
```

### SSH Config Requirements

Only entries with ALL of these are discovered:
- `Host` (the alias)
- `HostName` (actual IP/domain)
- `User`
- `IdentityFile`

Example SSH config entry:
```
Host build-server
    HostName 192.168.1.100
    User ubuntu
    IdentityFile ~/.ssh/build_key
    # Optional RCH hints (parsed as comments)
    # rch-slots: 16
    # rch-priority: 90
    # rch-tags: rust,bun
```

## Worker Probing

Test worker connectivity and detect capabilities:

```bash
# Probe single worker
rch workers probe fast-builder --verbose

# Probe all workers
rch workers probe --all

# Output shows:
# - SSH connectivity
# - Detected toolchains (rustc, cargo, bun, gcc, etc.)
# - Available disk space
# - Current load
```

## Worker Tags

Tags enable project-specific worker selection:

```toml
# In workers.toml
[[workers]]
id = "rust-only"
tags = ["rust"]
# ...

[[workers]]
id = "polyglot"
tags = ["rust", "bun", "cpp"]
# ...
```

Projects can require tags in `.rch.toml`:
```toml
# .rch.toml in project root
required_tags = ["rust", "fast"]
```

## Slot Management

`total_slots` should match available CPU cores:

```bash
# Check cores on worker
ssh worker1 "nproc"

# Recommended: leave 1-2 cores for system
# For 16-core machine: total_slots = 14
```

## Disabling Workers

Temporarily disable without removing:

```toml
[[workers]]
id = "maintenance"
enabled = false  # Won't be selected
# ...
```

## Health Monitoring

```bash
# Check all worker status
rch workers status

# JSON output for monitoring
rch workers status --json

# Continuous monitoring
watch -n 5 'rch workers status'
```
