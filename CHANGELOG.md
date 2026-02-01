# Changelog

All notable changes to the Remote Compilation Helper (RCH) project.

---

## [1.1.0] - 2026-02-01

### Cross-Platform Support

**Windows & macOS Compatibility** - RCH now builds and runs on all major platforms:

- **Windows Compilation**: Cross-platform build with proper feature-gating for Unix-only components (`openssh`, daemon socket)
- **macOS Compatibility**: Platform-specific process detection, handling read-only root filesystem for rich_rust dependencies
- **Platform-Agnostic Tests**: E2E fixtures use platform-agnostic usernames, binary hash computation works across platforms
- **Feature-Gated Imports**: Conditional compilation for Unix-specific functionality

---

### Fleet Module Overhaul (Stub Elimination)

**Critical: All fleet operations now perform REAL work** - Previously, fleet commands were stubs that reported success without doing anything. This was a major reliability issue discovered during production testing.

**`run_preflight()` - Real SSH Checks**:
- Parallel SSH connectivity testing to all workers
- Real disk space queries via `df -BM`
- Tool availability verification (rsync, zstd, rustup)
- rch-wkr version detection on workers
- Timeout handling (30s per check)
- Issue collection with actionable remediation

**`get_fleet_status()` - Live Worker Queries**:
- Concurrent worker status checks (semaphore-limited to 10)
- Real SSH connectivity testing
- Actual version detection via `rch-wkr --version`
- Health check execution via `rch-wkr health`
- Proper timeout handling for unresponsive workers

**`rollback_workers()` - Binary Restoration**:
- Backup creation during deployment (stored in `~/.config/rch/backups.json`)
- Real binary restoration via SSH/SCP
- Version verification after rollback
- Parallel rollback with graceful worker lifecycle
- Backup retention (last 3 per worker)
- CommandRunner trait for testable backup operations

---

### New Commands

**`rch config doctor`** - Diagnose configuration issues:
- Validates SSH identity file existence and permissions
- Tests worker connectivity
- Checks daemon socket accessibility
- Provides remediation suggestions for issues

**`rch config edit`** - Edit configuration in default editor:
- Opens config.toml or workers.toml in `$EDITOR`
- Validates syntax after save
- Supports `--workers` flag for workers.toml

**`rch config get`** - Query individual config values:
- Dot-notation access (e.g., `rch config get general.log_level`)
- JSON output support
- Lists available keys with `--list`

---

### Code Architecture Improvements

**Command Modularization** - The 3800+ line commands.rs was split into focused modules:

| Module | Purpose |
|--------|---------|
| `commands/daemon.rs` | Daemon lifecycle (start, stop, restart, reload) |
| `commands/config.rs` | Config show/set/init/get/edit/doctor |
| `commands/agent.rs` | Agent detection and hook management |
| `commands/queue.rs` | Build queue inspection |
| `commands/speedscore.rs` | Worker benchmarking |
| `commands/workers_deploy.rs` | Binary deployment to workers |
| `commands/workers_init.rs` | Worker discovery and setup |

**Structured Error Handling** - Converted all bare `bail!()` calls to structured `RchError` with:
- Error codes (RCH-Exxx)
- Contextual information
- Remediation suggestions
- Machine-parseable format

**Deep Audit Fixes** - Comprehensive security, reliability, and performance review:
- Removed dead code replaced by single-pass state machine
- Minor improvements to config, completions, and CLI
- Fixed formatting and type mismatches

---

### Security Hardening

**Sigstore Verification** - Strict certificate verification for self-update:
- Enforces `certificate-identity` and `certificate-oidc-issuer` flags
- Validates GitHub Actions OIDC tokens
- Rejects unsigned or incorrectly signed releases

**Dependency Updates**:
- LRU cache updated to 0.16 (security fix)

---

### Bug Fixes

**Bun Test Timeout Wrapper** - Prevents indefinite CPU hangs:
- External `timeout --signal=KILL` wrapper for bun test/typecheck
- Configurable via `bun_test_timeout_secs` (default: 600s / 10 minutes)
- Addresses known Bun defects causing 100% CPU loops

**Cache Affinity Uniqueness**:
- Path hash added to project names for cache key uniqueness
- Prevents collisions between projects with same directory name

**Overflow Prevention**:
- `saturating_mul` for age calculations in cache warmth decay

---

### Testing Improvements

**CI/CD Stability**:
- Exponential backoff in E2E daemon tests
- CPU variance tolerance increased for CI environments
- Daemon test timeouts extended for slower CI runners
- Tests respect `CARGO_TARGET_DIR` consistently

**Test Infrastructure**:
- Thread-safe environment variable helpers
- Proptest regression test cases
- Feature-gating for true-e2e tests
- Global test logging initialization

**Test Coverage**:
- Preflight check unit tests for all worker status types
- Rollback registry unit tests (CRUD, concurrent writes, shutdown order)
- Mock SSH executor for isolated unit testing

---

## [1.0.0] - 2026-01-30

The 1.0.0 release marks RCH as feature-complete for production use. This release represents approximately 3 weeks of intensive development with 827 commits, building the entire system from initial scaffold to a fully functional transparent compilation offloading system.

---

### Core Hook System

**Transparent Compilation Interception** - RCH integrates with Claude Code's PreToolUse hook system to intercept compilation commands transparently. The agent is unaware of remote execution; artifacts simply appear locally.

- **5-Tier Classification System** - High-precision command classification with sub-millisecond performance:
  - Tier 0: Instant reject (empty commands, non-Bash tools)
  - Tier 1: Structure analysis (pipes, redirects, backgrounding)
  - Tier 2: SIMD keyword filter (memchr-accelerated)
  - Tier 3: Negative pattern check (never-intercept commands)
  - Tier 4: Full classification with confidence scoring

- **Supported Compilation Commands**:
  - Rust: `cargo build`, `cargo test`, `cargo check`, `cargo clippy`, `cargo doc`, `cargo bench`, `cargo nextest run`
  - Bun/TypeScript: `bun test`, `bun typecheck`
  - C/C++: `gcc`, `g++`, `clang`, `clang++`
  - Build systems: `make`, `cmake --build`, `ninja`, `meson compile`

- **Smart Multi-Command Classification** - Commands chained with `&&`, `||`, or `;` are split and classified independently. If any sub-command is compilation, it triggers remote execution.

- **Zero-Allocation Hot Paths** - Classification strings use `Cow<'static, str>` to avoid heap allocations on the critical path.

---

### Worker Fleet Management

**Intelligent Worker Selection** - Multiple selection strategies optimize for different workloads:

- **Priority Strategy** - Respects configured worker priorities for preferred routing
- **Fastest Strategy** - Routes to workers with highest SpeedScore
- **Balanced Strategy** - Distributes load evenly across workers
- **CacheAffinity Strategy** - Routes projects to workers with warm caches
- **FairFastest Strategy** - Balances speed and fair distribution

**Health Monitoring & Circuit Breakers**:
- Automatic health probing with configurable intervals
- Circuit breaker pattern (Closed → Open → HalfOpen) for fault tolerance
- Consecutive failure tracking with configurable thresholds
- Automatic recovery with exponential backoff

**Worker Lifecycle Management**:
- `Healthy`, `Degraded`, `Unhealthy`, `Unknown`, `Draining`, `Drained`, `Disabled` statuses
- Graceful drain: stop new jobs, let existing jobs complete
- Manual enable/disable with optional reasons

**Slot-Based Concurrency** - Each worker declares total slots (typically CPU cores). The daemon tracks slot usage and never overcommits.

---

### Transfer Pipeline

**High-Performance File Synchronization**:
- rsync with zstd compression for efficient transfers
- Incremental sync (only changed files)
- Configurable exclusion patterns
- `.rchignore` support (similar to `.gitignore`)

**Artifact Retrieval**:
- Automatic retrieval of `target/` build artifacts
- Checksum verification for integrity
- Retry logic with exponential backoff for transient failures

**SSH Transport Hardening**:
- Configurable ServerAliveInterval and ControlPersist
- Connection pooling with ControlMaster
- Timeout handling for hung connections
- Identity file validation

---

### Daemon (rchd)

**Unix Socket API** - HTTP-like protocol over Unix socket for hook-daemon communication:

| Endpoint | Purpose |
|----------|---------|
| `GET /select-worker` | Request a worker for compilation |
| `POST /release-worker` | Release a reserved worker |
| `GET /status` | Full system status |
| `GET /health` | Kubernetes-style health check |
| `GET /ready` | Kubernetes-style readiness check |
| `GET /metrics` | Prometheus-format metrics |
| `GET /events` | Server-Sent Events stream |
| `POST /reload` | Hot-reload configuration |
| `POST /shutdown` | Graceful shutdown |

**Build Queue Management**:
- Wait-for-worker queueing when all workers busy
- FIFO queue with priority support
- Queue position tracking and ETA estimation

**Configuration Hot-Reload**:
- `rch daemon reload` or `SIGHUP` signal
- Add/remove/update workers without restart
- Validation before applying changes

**Event Bus**:
- Real-time event streaming for monitoring
- Events: `worker:selected`, `worker:released`, `build:started`, `build:completed`, `health:changed`

---

### CLI Commands

**Comprehensive Command Suite**:

```
rch status              # System overview with worker health
rch workers list        # List workers with slot usage
rch workers probe       # Test worker connectivity
rch workers deploy      # Deploy rch-wkr binary to workers
rch workers enable/disable/drain  # Lifecycle management
rch fleet deploy        # Parallel deployment to all workers
rch daemon start/stop/restart/reload  # Daemon control
rch config show/set/init  # Configuration management
rch doctor              # Diagnose and fix issues
rch check               # Quick health verification
rch update check/install/rollback  # Self-update system
```

**Rich Output Modes**:
- Interactive mode with colors and panels (TTY)
- JSON mode for programmatic access (`--json` or `RCH_JSON=1`)
- Plain text for non-TTY environments

**Short Flags** - Common options have short forms:
- `-a` for `--all`
- `-v` for `--verbose`
- `-f` for `--force`

**Confirmation Prompts** - Destructive actions require confirmation (can skip with `--yes`).

**Shell Completions** - Tab completion for bash, zsh, and fish:
- `rch completions bash > ~/.local/share/bash-completion/completions/rch`
- `rch completions zsh > ~/.zfunc/_rch`
- `rch completions fish > ~/.config/fish/completions/rch.fish`

---

### Runtime Detection & Toolchain Management

**Automatic Toolchain Detection**:
- Parses `rust-toolchain.toml` for Rust projects
- Detects channel (stable/beta/nightly), version, components, targets
- Forwards toolchain info to workers for environment setup

**Worker Capability Probing**:
- Workers declare available runtimes (Rust, Bun, Node, GCC/Clang)
- Runtime filtering prevents routing to incompatible workers
- Version detection for all supported toolchains

**Toolchain Synchronization**:
- Auto-installs missing Rust toolchains on workers via rustup
- Verifies component availability before compilation
- Graceful local fallback on toolchain failures

---

### Terminal UI (TUI)

**Interactive Dashboard** (`rch tui`):

- **Workers Panel** - Real-time worker status with health indicators
- **Build History Panel** - Recent builds with timing and outcomes
- **Detail Bar** - Full content preview of selected items
- **Help Overlay** - Keyboard shortcut reference (`?` to toggle)

**Keyboard Controls**:
- `Tab` / `Shift+Tab` - Navigate panels
- `j/k` or arrows - Navigate items
- `d` - Drain selected worker
- `e` - Enable selected worker
- `/` - Filter mode
- `q` - Quit

**Visual Indicators**:
- Unicode symbols for cross-terminal compatibility (no emoji)
- Color-coded status (green=healthy, yellow=degraded, red=unhealthy)
- Slot usage bars with percentage

**Sort Controls** - Build history sortable by time, duration, or worker.

---

### Rich Terminal Output

**Styled Box Rendering**:
- Configurable borders (rounded, sharp, double, ascii)
- Padding and margin control
- Nested box support

**Terminal Hyperlinks**:
- Clickable links in terminal output (OSC 8 escape sequences)
- Links to documentation, GitHub issues, worker hosts
- Graceful fallback for unsupported terminals

**Markdown Rendering**:
- Basic markdown support in terminal output
- Code blocks with syntax hints
- Headers, lists, and emphasis

**Agent Detection**:
- Automatic detection of Claude Code, Codex CLI, Cursor
- Context-aware output formatting per agent
- Machine-readable output for automated agents

**Progress Indicators**:
- Spinner for long-running operations
- Progress bars for file transfers
- Phase indicators for multi-step operations

---

### Self-Test System

**Remote Compilation Verification** - Proves the pipeline works end-to-end:

1. Applies unique test marker to source code
2. Builds locally for reference hash
3. Syncs source to worker
4. Builds on worker
5. Retrieves artifacts
6. Compares binary hashes

**Scheduled Self-Tests**:
- Configurable cron schedule
- History retention and reporting
- Alert on repeated failures

---

### Update System

**Self-Updating Capability**:
- `rch update check` - Check for new releases
- `rch update install` - Download and install update
- `rch update rollback` - Revert to previous version

**Security Features**:
- SHA256 checksum verification
- Sigstore signature verification (when bundle available)
- Backup of current binary before update

**Fleet Deployment** (`rch fleet deploy`):
- Parallel deployment to all workers
- Progress tracking per worker
- Version verification after install

---

### Telemetry & SpeedScore

**Worker Telemetry Collection**:
- CPU usage (overall, per-core, load average)
- Memory usage (used, available, swap)
- Disk I/O metrics
- Network throughput

**SpeedScore System** - Composite performance score (0-100) based on:
- CPU benchmark results
- Memory bandwidth
- Disk I/O speed
- Network latency

**Benchmark Scheduler**:
- Automatic re-benchmark on score staleness
- Drift detection triggers re-benchmark
- Manual trigger via API

---

### Configuration & Validation

**Configuration Files**:
- `~/.config/rch/config.toml` - User settings
- `~/.config/rch/workers.toml` - Worker definitions

**Setup Wizard** (`rch config init`):
- Interactive worker configuration
- SSH key validation
- Connectivity testing

**Comprehensive Validation**:
- SSH identity file existence and permissions
- rsync exclude pattern syntax
- Network address resolution
- Remote directory accessibility

---

### Error Handling & Diagnostics

**Structured Error Codes** - All errors have unique codes (RCH-Exxx):

| Range | Category |
|-------|----------|
| E001-E099 | Configuration errors |
| E100-E199 | Network/SSH errors |
| E200-E299 | Worker errors |
| E300-E399 | Build errors |
| E400-E499 | Daemon errors |
| E500-E599 | Internal errors |

**Rich Error Display**:
- Color-coded severity
- Contextual information (worker, host, command)
- Remediation suggestions
- Related documentation links

**Fail-Open Philosophy** - If anything fails, allow local execution rather than blocking the agent.

---

### Installation System

**One-Line Install**:
```bash
curl -fsSL https://raw.githubusercontent.com/Dicklesworthstone/remote_compilation_helper/main/install.sh | bash
```

**Installation Features**:
- Automatic binary download for platform
- Fallback to source build if binaries unavailable
- Claude Code hook integration
- Systemd/launchd service setup (optional)
- Shell completions (bash, zsh, fish)

**Agent Integration**:
- Automatic Claude Code `.claude.json` configuration
- Hook registration in `~/.claude/settings.json`

---

### Testing Infrastructure

**Comprehensive Test Coverage**:
- Unit tests with TestLogger (JSONL output)
- Integration tests for subsystems
- True end-to-end tests with real workers
- Property-based tests (proptest) for edge cases

**Test Categories**:
- Command classification tests
- Worker selection tests
- Transfer pipeline tests
- Fail-open behavior tests
- Exit code propagation tests
- Artifact integrity tests

**CI/CD Integration**:
- GitHub Actions workflows
- Dependabot automerge for patches
- Performance regression detection

---

### Performance Characteristics

| Operation | Target | Panic Threshold |
|-----------|--------|-----------------|
| Hook decision (non-compilation) | <1ms | 5ms |
| Hook decision (compilation) | <5ms | 10ms |
| Worker selection | <10ms | 50ms |
| Full pipeline overhead | <15% | 50% |

**Optimizations**:
- SIMD-accelerated keyword search (memchr)
- Zero-allocation classification paths
- In-memory timing history cache
- Connection pooling

---

### Security

- **No unsafe code** - `#![forbid(unsafe_code)]` enforced
- **Shell escaping** - All user input properly escaped with shell-words
- **SSH key validation** - Permissions checked (0600/0400) before use
- **Checksum verification** - All downloads verified with SHA256
- **No secrets in config** - Identity files referenced by path
- **Command classification hardening** - Prevents shell injection via crafted commands
- **Path sanitization** - All file paths validated before transfer

---

### Known Limitations (1.0.0)

- Benchmark execution stub (placeholder for rch-benchmark integration)
- Changelog diff computation in update check

**Note:** Rollback was a stub in 1.0.0 but is now fully implemented in 1.1.0.

---

## Contributors

Built with assistance from Claude Code AI agents using the Beads issue tracking system.

---

## License

MIT License - See LICENSE file for details.
