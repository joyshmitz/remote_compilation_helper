# Remote Compilation Helper (RCH) - Comprehensive Implementation Plan

> **Mission**: Transform an overloaded agent workstation into a serene orchestration hub by transparently offloading all compilation work to a fleet of remote workers, with zero agent awareness and sub-second decision latency.

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Problem Analysis](#2-problem-analysis)
3. [System Architecture](#3-system-architecture)
4. [Core Components](#4-core-components)
5. [Command Interception Layer](#5-command-interception-layer)
6. [Worker Fleet Management](#6-worker-fleet-management)
7. [Code Transfer Pipeline](#7-code-transfer-pipeline)
8. [Remote Execution Engine](#8-remote-execution-engine)
9. [Artifact Return Pipeline](#9-artifact-return-pipeline)
10. [Toolchain Synchronization](#10-toolchain-synchronization)
11. [Configuration System](#11-configuration-system)
12. [Installation & Deployment](#12-installation--deployment)
13. [Performance Optimization](#13-performance-optimization)
14. [Failure Modes & Resilience](#14-failure-modes--resilience)
15. [Security Considerations](#15-security-considerations)
16. [Observability & Metrics](#16-observability--metrics)
17. [CLI Interface](#17-cli-interface)
18. [Implementation Roadmap](#18-implementation-roadmap)
19. [File Structure](#19-file-structure)
20. [Appendices](#appendices)

---

## 1. Executive Summary

### The Problem
Running 15+ AI coding agents simultaneously creates a "compilation storm" where multiple agents invoke `cargo build`, `rustc`, `gcc`, or `clang` concurrently. On a single machine, this:
- Saturates all CPU cores (100% utilization across 16-32 threads)
- Causes kernel scheduler contention, making terminals unresponsive
- Creates thermal throttling, reducing sustained throughput
- Destroys the joy of watching agents work in parallel

### The Solution: Remote Compilation Helper (RCH)

RCH is a **transparent compilation offloading system** that:

1. **Intercepts** compilation commands via Claude Code's PreToolUse hook (like DCG)
2. **Rewrites** the command to execute remotely instead of locally
3. **Selects** the optimal worker based on available slots and performance scores
4. **Transfers** the codebase efficiently using zstd compression + rsync delta encoding
5. **Executes** the exact same compilation command on the remote worker
6. **Returns** artifacts compressed with zstd to the local target directory

**Key Differentiator from DCG**: DCG *blocks* dangerous commands; RCH *redirects* compilation commands. The hook never denies—it transforms and executes.

### Key Metrics Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| Hook decision latency | <5ms | Agents shouldn't notice the interception |
| Worker selection | <10ms | Quick routing decision |
| Initial code transfer | <30s for 100MB | First-time sync with full codebase |
| Incremental transfer | <3s for typical change | Only changed files via rsync |
| Artifact return | <10s for debug build | target/ dir can be large |
| End-to-end overhead | <15% | Networking shouldn't kill benefits |

---

## 2. Problem Analysis

### 2.1 Current Pain Points

```
┌─────────────────────────────────────────────────────────────────┐
│                     AGENT WORKSTATION                            │
│                                                                  │
│  Agent 1 ──► cargo build ──┐                                    │
│  Agent 2 ──► cargo build ──┼──► CPU: 100% ──► KERNEL CONTENTION │
│  Agent 3 ──► cargo test  ──┤              ──► THERMAL THROTTLE  │
│  Agent 4 ──► gcc -O3     ──┤              ──► UNRESPONSIVE SHELL│
│  Agent 5 ──► rustc       ──┘              ──► ELEVATED BLOOD    │
│  ...                                           PRESSURE          │
│  Agent 15 ──► cargo build (waiting in queue...)                 │
└─────────────────────────────────────────────────────────────────┘
```

**Observed Symptoms:**
- `top` shows 16 `rustc` processes at 100% each
- Terminal input latency: 2-5 seconds
- System load average: 30+ on 16-core machine
- Memory pressure causing swap thrashing
- SSH sessions timing out during heavy compilation

### 2.2 Available Resources

Based on the user's infrastructure:

| Alias | Hostname | Provider | Purpose | Est. Cores |
|-------|----------|----------|---------|------------|
| `css` | 203.0.113.20 | Contabo | Superserver | 16-32 |
| `csd` | 198.51.100.20 | Contabo | Demo box | 8-16 |
| `yto` | 203.0.113.30 | OVH | Worker | 8-16 |
| `fmd` | 192.0.2.40 | OVH | Worker | 8-16 |

**Total Remote Capacity**: ~48-80 cores across 4 machines

### 2.3 Why Not Just Use Distributed Build Tools?

| Tool | Why Not Sufficient |
|------|-------------------|
| `sccache` | Only caches; doesn't distribute compilation |
| `distcc` | C/C++ only; requires same toolchain setup |
| `icecream` | Complex scheduler; not agent-aware |
| `cargo remote` | Requires manual invocation; not transparent |
| `cross` | For cross-compilation, not remote |

**RCH fills the gap**: A transparent, agent-aware, polyglot (Rust + C/C++) remote compilation layer with automatic worker selection.

---

## 3. System Architecture

### 3.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            AGENT WORKSTATION                                 │
│                                                                              │
│  ┌─────────┐    ┌─────────────────────────────────────────────────────────┐ │
│  │ Claude  │    │                     RCH SYSTEM                          │ │
│  │ Code    │    │                                                         │ │
│  │         │    │  ┌──────────────┐   ┌─────────────┐   ┌──────────────┐ │ │
│  │ Agent 1 │───►│  │ PreToolUse  │──►│  Command    │──►│   Worker     │ │ │
│  │ Agent 2 │    │  │    Hook     │   │  Analyzer   │   │  Selector    │ │ │
│  │ Agent 3 │    │  │   (rch)     │   │             │   │              │ │ │
│  └─────────┘    │  └──────────────┘   └─────────────┘   └──────┬───────┘ │ │
│                 │                                               │         │ │
│                 │  ┌──────────────────────────────────────────┐ │         │ │
│                 │  │              LOCAL DAEMON                 │ │         │ │
│                 │  │   ┌────────────────────────────────┐     │ │         │ │
│                 │  │   │  Worker Pool State Manager    │◄────┘ │         │ │
│                 │  │   │  • Slot tracking per worker   │       │         │ │
│                 │  │   │  • Speed scores from bench    │       │         │ │
│                 │  │   │  • Health check heartbeats    │       │         │ │
│                 │  │   └────────────────────────────────┘       │         │ │
│                 │  │                     │                      │         │ │
│                 │  │   ┌────────────────▼────────────────┐     │         │ │
│                 │  │   │     Job Queue & Dispatcher      │     │         │ │
│                 │  │   └─────────────────────────────────┘     │         │ │
│                 │  └────────────────────┬───────────────────────┘         │ │
│                 └───────────────────────┼─────────────────────────────────┘ │
└─────────────────────────────────────────┼───────────────────────────────────┘
                                          │
                     ┌────────────────────┴────────────────────┐
                     │              SSH TRANSPORT              │
                     │    (Multiplexed, Persistent Connections) │
                     └────────┬─────────────┬─────────────┬────┘
                              │             │             │
              ┌───────────────▼──┐  ┌───────▼───────┐  ┌──▼──────────────┐
              │   WORKER: css    │  │  WORKER: csd  │  │  WORKER: yto   │
              │   ┌───────────┐  │  │  ┌─────────┐  │  │  ┌───────────┐ │
              │   │  rch-wkr  │  │  │  │ rch-wkr │  │  │  │  rch-wkr  │ │
              │   │           │  │  │  │         │  │  │  │           │ │
              │   │ •Executor │  │  │  │ •Exec   │  │  │  │ •Executor │ │
              │   │ •Toolchain│  │  │  │ •Tool   │  │  │  │ •Toolchain│ │
              │   │ •Sandbox  │  │  │  │ •Sand   │  │  │  │ •Sandbox  │ │
              │   └───────────┘  │  │  └─────────┘  │  │  └───────────┘ │
              │   Slots: 8/16    │  │  Slots: 4/8   │  │  Slots: 6/8    │
              │   Speed: 87.3    │  │  Speed: 72.1  │  │  Speed: 68.9   │
              └──────────────────┘  └───────────────┘  └────────────────┘
```

### 3.2 Component Interaction Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         COMPILATION REQUEST FLOW                             │
└─────────────────────────────────────────────────────────────────────────────┘

 ① Agent invokes: cargo build --release

 ② Claude Code triggers PreToolUse hook with JSON:
    {
      "tool_name": "Bash",
      "tool_input": {
        "command": "cargo build --release"
      }
    }

 ③ RCH Hook receives JSON, parses command:
    • Detected: cargo build --release
    • Classification: RUST_CARGO_BUILD
    • Project root: /data/projects/my_project
    • Target dir: /data/projects/my_project/target

 ④ RCH Hook queries Local Daemon via Unix socket:
    → GET /select-worker?project=my_project&est_cores=4

 ⑤ Daemon responds with best worker:
    ← { "worker": "css", "slots_available": 12, "speed_score": 87.3 }

 ⑥ Daemon decrements css slots: 12 → 8 (cargo may use 4 cores)

 ⑦ RCH Hook initiates transfer pipeline:
    a. Compute project hash for cache key
    b. rsync with --zstd to css:/tmp/rch/my_project_abc123/
    c. Transfer completes in 2.3s (incremental)

 ⑧ RCH Hook executes remote command via SSH:
    ssh css "cd /tmp/rch/my_project_abc123 && cargo build --release"
    • stdout/stderr streamed back in real-time
    • Exit code captured

 ⑨ On completion, artifact return pipeline:
    a. Remote: tar --zstd -cf /tmp/artifacts.tar.zst target/release/
    b. rsync back to local target/release/
    c. 4.1s transfer time

 ⑩ RCH Hook outputs to Claude Code:
    • Empty stdout (success, no denial)
    • Exit code 0
    • Local target/ now has remote-built artifacts

 ⑪ Daemon increments css slots: 8 → 12

 ⑫ Agent sees successful "cargo build" - completely unaware of remote execution
```

### 3.3 Data Flow Diagram

```
┌────────────┐     ┌────────────┐     ┌────────────┐     ┌────────────┐
│   Source   │     │  Staging   │     │   Remote   │     │  Artifacts │
│   Files    │────►│   Area     │────►│   Worker   │────►│   Return   │
│            │     │            │     │            │     │            │
│ /project/* │     │ .rch/stage │     │ /tmp/rch/* │     │  target/*  │
└────────────┘     └────────────┘     └────────────┘     └────────────┘
       │                 │                  │                  │
       ▼                 ▼                  ▼                  ▼
   ┌────────┐       ┌────────┐        ┌────────┐         ┌────────┐
   │ .git/  │       │ zstd   │        │ rustc  │         │ zstd   │
   │ ignore │       │ level  │        │ gcc    │         │ level  │
   │ filter │       │ 3-5    │        │ clang  │         │ 1-3    │
   └────────┘       └────────┘        └────────┘         └────────┘
```

---

## 4. Core Components

### 4.1 Component Overview

| Component | Binary | Purpose | Location |
|-----------|--------|---------|----------|
| **rch** | `rch` | PreToolUse hook CLI | Agent Workstation |
| **rchd** | `rchd` | Local daemon for orchestration | Agent Workstation |
| **rch-wkr** | `rch-wkr` | Worker agent | Remote Workers |
| **rch-install** | `rch-install` | Installer/deployer | Both |
| **rch-bench** | `rch-bench` | Benchmark runner | Remote Workers |

### 4.2 Component Details

#### 4.2.1 RCH Hook (`rch`)

The PreToolUse hook binary that Claude Code invokes for every Bash command.

**Responsibilities:**
- Parse Claude Code hook JSON input
- Identify compilation commands via pattern matching
- Communicate with `rchd` daemon for worker selection
- Orchestrate transfer → execute → return pipeline
- Output empty (allow) or transformed result

**Key Design Decisions:**
- **Stateless**: All state lives in the daemon; hook is pure function
- **Fast-path rejection**: Non-compilation commands exit in <1ms
- **Async I/O**: Use tokio for concurrent transfer/execute operations
- **Streaming output**: Forward compilation output to terminal in real-time

```rust
// Pseudo-code structure
pub struct RchHook {
    daemon_socket: PathBuf,
    config: Config,
}

impl RchHook {
    pub async fn process(&self, input: HookInput) -> HookOutput {
        // 1. Quick reject non-Bash tools
        if input.tool_name != "Bash" {
            return HookOutput::allow();
        }

        // 2. Parse and classify command
        let cmd = &input.tool_input.command;
        let classification = classify_command(cmd);

        // 3. Fast-path for non-compilation commands
        if !classification.is_compilation() {
            return HookOutput::allow();
        }

        // 4. Execute remote compilation pipeline
        match self.execute_remote(cmd, &classification).await {
            Ok(_) => HookOutput::allow(),  // Success - artifacts are in place
            Err(e) => {
                // Fallback to local execution with warning
                eprintln!("[RCH] Remote failed, falling back to local: {}", e);
                HookOutput::allow()
            }
        }
    }
}
```

#### 4.2.2 RCH Daemon (`rchd`)

Long-running daemon managing worker pool state and job coordination.

**Responsibilities:**
- Maintain worker health via periodic heartbeats
- Track slot availability per worker
- Store/update speed scores from benchmarks
- Provide worker selection API
- Manage SSH connection pool (multiplexing)
- Coordinate concurrent compilations

**State Management:**

```rust
pub struct DaemonState {
    workers: HashMap<WorkerId, WorkerState>,
    ssh_pool: SshConnectionPool,
    metrics: MetricsCollector,
}

pub struct WorkerState {
    id: WorkerId,
    host: String,
    ssh_config: SshConfig,

    // Capacity
    total_slots: u32,
    used_slots: AtomicU32,

    // Performance
    speed_score: f64,          // 0-100 from cloud_benchmarker
    last_benchmark: DateTime,

    // Health
    status: WorkerStatus,
    last_heartbeat: DateTime,
    consecutive_failures: u32,
}

#[derive(Clone, Copy)]
pub enum WorkerStatus {
    Healthy,
    Degraded,      // Slow responses
    Unreachable,   // Failed heartbeat
    Draining,      // No new jobs
    Disabled,      // Manual override
}
```

**Worker Selection Algorithm:**

```rust
/// Selects optimal worker using weighted scoring
pub fn select_worker(&self, request: &SelectionRequest) -> Option<&Worker> {
    self.workers
        .values()
        .filter(|w| w.status == WorkerStatus::Healthy)
        .filter(|w| w.available_slots() >= request.estimated_cores)
        .max_by(|a, b| {
            let score_a = self.compute_selection_score(a, request);
            let score_b = self.compute_selection_score(b, request);
            score_a.partial_cmp(&score_b).unwrap_or(Ordering::Equal)
        })
}

fn compute_selection_score(&self, worker: &Worker, request: &SelectionRequest) -> f64 {
    // Weights configurable via config file
    let slot_weight = 0.4;
    let speed_weight = 0.5;
    let locality_weight = 0.1;

    let slot_score = worker.available_slots() as f64 / worker.total_slots as f64;
    let speed_score = worker.speed_score / 100.0;
    let locality_score = if worker.has_cached_project(&request.project) { 1.0 } else { 0.5 };

    slot_weight * slot_score +
    speed_weight * speed_score +
    locality_weight * locality_score
}
```

#### 4.2.3 Worker Agent (`rch-wkr`)

Lightweight agent running on each remote worker.

**Responsibilities:**
- Respond to health checks
- Report slot availability
- Execute compilation commands in sandboxed environment
- Manage local project caches
- Run benchmarks on demand
- Clean up old project directories

**Design:**
- Minimal resource usage when idle
- No always-on daemon required (can use SSH directly)
- Optional: systemd service for persistent state

```rust
// Worker can operate in two modes:
// 1. Direct SSH invocation (stateless)
// 2. Daemon mode with Unix socket (stateful, faster)

pub enum WorkerMode {
    /// No persistent process; each SSH command is independent
    DirectSsh,

    /// Worker daemon running, accepts commands via socket
    Daemon { socket: PathBuf },
}
```

---

## 5. Command Interception Layer

The command interception layer is the heart of RCH's intelligence. It must achieve two seemingly contradictory goals: **intercept all legitimate compilation commands** while **never disrupting commands that should run locally**. False positives are catastrophic—they break agent workflows and destroy velocity. This section describes a sophisticated multi-tier approach designed for precision.

### 5.1 Command Structure Analysis

Before examining *what* command is being run, we first analyze *how* it's structured. Commands with pipes, redirections, or background operators often depend on local execution semantics that remote execution would break.

**Structural Categories:**

```rust
/// Represents the structural form of a shell command
#[derive(Debug, Clone)]
pub enum CommandStructure {
    /// Simple command: `cargo build`
    Simple(String),

    /// Piped command: `cargo build 2>&1 | grep error`
    /// Output format matters - DO NOT intercept
    Piped { commands: Vec<String> },

    /// Chained commands: `cargo build && ./run.sh`
    /// May intercept first command only
    Chained { commands: Vec<(String, ChainOperator)> },

    /// Backgrounded: `cargo build &`
    /// Long-running, agent expects immediate return - DO NOT intercept
    Backgrounded(String),

    /// Output redirected: `cargo build > log.txt`
    /// Agent is capturing output - DO NOT intercept
    Redirected {
        command: String,
        stdout: Option<PathBuf>,
        stderr: Option<PathBuf>,
        append: bool,
    },

    /// Subshell capture: `result=$(cargo build)`
    /// Output is being captured programmatically - DO NOT intercept
    Subshell(String),
}

#[derive(Debug, Clone, Copy)]
pub enum ChainOperator {
    And,        // &&  - second runs only if first succeeds
    Or,         // ||  - second runs only if first fails
    Sequence,   // ;   - unconditional sequence
}
```

**Structure Parser:**

```rust
pub fn analyze_structure(cmd: &str) -> CommandStructure {
    let cmd = cmd.trim();

    // Check for backgrounding (must be at very end, not in quotes)
    if ends_with_unquoted(cmd, '&') && !ends_with_unquoted(cmd, "&&") {
        return CommandStructure::Backgrounded(cmd.trim_end_matches('&').trim().into());
    }

    // Check for subshell capture
    if cmd.contains("$(") || cmd.contains("`") {
        return CommandStructure::Subshell(cmd.into());
    }

    // Check for pipes (outside quotes)
    if contains_unquoted(cmd, '|') && !contains_unquoted(cmd, "||") {
        let parts = split_on_unquoted(cmd, '|');
        return CommandStructure::Piped { commands: parts };
    }

    // Check for output redirection
    if let Some(redir) = parse_redirections(cmd) {
        return CommandStructure::Redirected {
            command: redir.command,
            stdout: redir.stdout,
            stderr: redir.stderr,
            append: redir.append,
        };
    }

    // Check for chaining operators
    if contains_unquoted(cmd, "&&") || contains_unquoted(cmd, "||") || contains_unquoted(cmd, ';') {
        let parts = parse_chain(cmd);
        return CommandStructure::Chained { commands: parts };
    }

    CommandStructure::Simple(cmd.into())
}

impl CommandStructure {
    /// Returns true if this structure allows remote interception
    pub fn allows_interception(&self) -> bool {
        match self {
            // Simple commands are always candidates
            CommandStructure::Simple(_) => true,

            // Piped commands: output format/timing matters, don't intercept
            CommandStructure::Piped { .. } => false,

            // Backgrounded: agent expects immediate return
            CommandStructure::Backgrounded(_) => false,

            // Redirected: output is being captured
            CommandStructure::Redirected { .. } => false,

            // Subshell: output is being programmatically captured
            CommandStructure::Subshell(_) => false,

            // Chained: only intercept if first in chain with &&
            CommandStructure::Chained { commands } => {
                commands.len() >= 1 &&
                commands.first().map(|(_, op)| *op == ChainOperator::And).unwrap_or(false)
            }
        }
    }

    /// Extract the interceptable portion of a command (if any)
    pub fn interceptable_command(&self) -> Option<&str> {
        match self {
            CommandStructure::Simple(cmd) => Some(cmd),
            CommandStructure::Chained { commands } if self.allows_interception() => {
                commands.first().map(|(cmd, _)| cmd.as_str())
            }
            _ => None,
        }
    }
}
```

**Why Structure Matters:**

| Structure | Example | Intercept? | Reason |
|-----------|---------|------------|--------|
| Simple | `cargo build` | ✅ Yes | Safe to redirect |
| Piped | `cargo build \| grep warning` | ❌ No | Output format matters to grep |
| Backgrounded | `cargo build &` | ❌ No | Agent expects immediate return |
| Redirected | `cargo build > log.txt` | ❌ No | Agent is capturing to file |
| Subshell | `lines=$(cargo build)` | ❌ No | Output captured as variable |
| Chained (first) | `cargo build && ./run` | ✅ Yes | Can intercept first part |

### 5.2 Command Classification Taxonomy

**Commands to Intercept (Positive Patterns):**

```
INTERCEPTABLE_COMMANDS
├── RUST
│   ├── cargo build [--release] [--target TARGET]
│   ├── cargo test [--release] [--no-run]
│   ├── cargo check
│   ├── cargo clippy
│   ├── cargo doc
│   ├── cargo bench
│   └── rustc [flags] source.rs
│
├── C_CPP
│   ├── gcc [flags] source.c [-o output]
│   ├── g++ [flags] source.cpp [-o output]
│   ├── clang [flags] source.c [-o output]
│   ├── clang++ [flags] source.cpp [-o output]
│   └── cc / c++ (symlinks)
│
└── BUILD_SYSTEMS
    ├── make [target]
    ├── cmake --build <dir>
    ├── ninja [-C dir]
    └── meson compile [-C dir]
```

**Commands to NEVER Intercept (Negative Patterns):**

These commands match compilation keywords but must run locally:

```rust
/// Commands that should NEVER be intercepted, regardless of pattern match.
/// These either modify local state, run long-term, or have side effects
/// that require local execution.
static NEVER_INTERCEPT: &[&str] = &[
    // === Long-running watchers ===
    "cargo watch",
    "cargo-watch",

    // === Local system modifications ===
    "cargo install",      // Installs to local ~/.cargo/bin
    "cargo uninstall",    // Removes from local system

    // === Authentication & publishing ===
    "cargo publish",      // Publishes to crates.io
    "cargo login",        // Stores credentials locally
    "cargo logout",       // Removes local credentials
    "cargo owner",        // Manages crate ownership
    "cargo yank",         // Yanks published version

    // === Package/dependency management ===
    "cargo update",       // Updates Cargo.lock (must be local)
    "cargo add",          // Modifies Cargo.toml (may prompt)
    "cargo remove",       // Modifies Cargo.toml (may prompt)
    "cargo generate-lockfile",
    "cargo vendor",       // Creates local vendor directory
    "cargo fetch",        // Downloads to local cache

    // === Project scaffolding ===
    "cargo init",         // Creates files in CWD
    "cargo new",          // Creates new directory locally

    // === Local source modifications ===
    "cargo fmt",          // Modifies source files in place
    "cargo fix",          // Modifies source files in place

    // === Cache operations ===
    "cargo clean",        // Cleans LOCAL target directory
    "cargo cache",        // Manages local cache

    // === Metadata only (no compilation) ===
    "cargo metadata",     // Just outputs JSON
    "cargo tree",         // Just outputs dependency tree
    "cargo search",       // Searches crates.io
    "cargo locate-project",

    // === Server patterns (cargo run variants) ===
    "cargo run -- --serve",
    "cargo run -- --server",
    "cargo run -- --listen",
    "cargo run -- --daemon",
    "cargo run -- -s",

    // === C/C++ non-compilation ===
    "gcc --version",
    "gcc -v",
    "clang --version",
    "clang -v",
    "make --version",
    "make -v",
    "make clean",         // Just deletes files
    "make distclean",

    // === Help flags ===
    "--help",
    "-h",
    "--version",
    "-V",
];

/// Check if command matches any negative pattern
pub fn matches_never_intercept(cmd: &str) -> bool {
    let normalized = normalize_command(cmd);
    let lower = normalized.to_lowercase();

    NEVER_INTERCEPT.iter().any(|pattern| {
        // Check if pattern appears at word boundary
        if pattern.contains(' ') {
            lower.contains(pattern)
        } else {
            // For single words, check word boundaries
            lower.split_whitespace().any(|word| word == *pattern)
        }
    })
}
```

### 5.3 Five-Tier Classification Engine

RCH uses a sophisticated five-tier classification system. Each tier acts as a filter, with cheap checks first and expensive checks last. This ensures sub-millisecond decisions for the 99% of commands that aren't compilation.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    5-TIER CLASSIFICATION ARCHITECTURE                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  TIER 0: Instant Reject (<0.01ms)                                           │
│  ├── Tool name != "Bash" ──────────────────────────────► PASS THROUGH       │
│  ├── Command is empty ─────────────────────────────────► PASS THROUGH       │
│  ├── Starts with non-compilation prefix ───────────────► PASS THROUGH       │
│  └── Contains --help, --version, -h, -V ───────────────► PASS THROUGH       │
│                     │                                                        │
│                     ▼                                                        │
│  TIER 1: Structure Analysis (<0.1ms)                                        │
│  ├── Has pipes (|) outside quotes ─────────────────────► PASS THROUGH       │
│  ├── Has background operator (&) ──────────────────────► PASS THROUGH       │
│  ├── Has output redirection (>, >>) ───────────────────► PASS THROUGH       │
│  └── Has subshell capture ($(), ``) ───────────────────► PASS THROUGH       │
│                     │                                                        │
│                     ▼                                                        │
│  TIER 2: Quick Keyword Filter (<0.2ms) [SIMD]                               │
│  ├── No compilation keywords found ────────────────────► PASS THROUGH       │
│  └── Has keywords ─────────────────────────────────────► Continue           │
│                     │                                                        │
│                     ▼                                                        │
│  TIER 3: Negative Pattern Check (<0.5ms)                                    │
│  ├── Matches NEVER_INTERCEPT list ─────────────────────► PASS THROUGH       │
│  ├── Matches project-level exclusions ─────────────────► PASS THROUGH       │
│  └── Clean ────────────────────────────────────────────► Continue           │
│                     │                                                        │
│                     ▼                                                        │
│  TIER 4: Full Classification & Confidence (<5ms)                            │
│  ├── Regex pattern matching                                                  │
│  ├── Context extraction (project root, args)                                │
│  ├── Confidence scoring (0.0 - 1.0)                                         │
│  └── Final decision with threshold check                                    │
│                     │                                                        │
│                     ▼                                                        │
│               INTERCEPT or PASS THROUGH                                      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Tier 0: Instant Reject**

```rust
/// Commands that NEVER involve compilation - reject instantly
static INSTANT_REJECT_PREFIXES: &[&str] = &[
    // Navigation & inspection
    "cd ", "cd\t", "ls", "pwd", "tree",

    // File viewing
    "cat ", "head ", "tail ", "less ", "more ",
    "bat ", "view ",

    // File operations
    "mkdir ", "touch ", "rm ", "cp ", "mv ", "ln ",
    "chmod ", "chown ", "chgrp ",

    // Text processing
    "grep ", "rg ", "ag ", "awk ", "sed ",
    "sort ", "uniq ", "wc ", "cut ", "tr ",
    "diff ", "comm ", "join ",

    // Version control
    "git ", "hg ", "svn ",

    // Container & orchestration
    "docker ", "podman ", "kubectl ", "helm ",

    // Remote operations
    "ssh ", "scp ", "rsync ", "sftp ",

    // Network utilities
    "curl ", "wget ", "http ",

    // Package managers (non-build)
    "pip ", "npm ", "yarn ", "pnpm ", "bun ",
    "apt ", "apt-get ", "brew ", "dnf ", "yum ",

    // Shell built-ins
    "echo ", "printf ", "export ", "source ", ".",
    "alias ", "unalias ", "which ", "type ", "whereis ",
    "env ", "set ", "unset ",

    // Process management
    "ps ", "top ", "htop ", "kill ", "pkill ",
    "jobs ", "fg ", "bg ", "nohup ",

    // System info
    "uname ", "hostname ", "whoami ", "id ", "groups ",
    "df ", "du ", "free ", "uptime ", "date ",

    // Editors (interactive, don't intercept)
    "vim ", "vi ", "nvim ", "nano ", "emacs ",
    "code ", "subl ",
];

#[inline]
pub fn instant_reject(cmd: &str) -> bool {
    let cmd = cmd.trim_start();

    // Empty command
    if cmd.is_empty() {
        return true;
    }

    // Check prefixes
    if INSTANT_REJECT_PREFIXES.iter().any(|p| cmd.starts_with(p)) {
        return true;
    }

    // Help/version flags anywhere in command
    if cmd.contains("--help") || cmd.contains("--version")
        || cmd.contains(" -h") || cmd.contains(" -V") {
        return true;
    }

    false
}
```

**Tier 2: SIMD Keyword Filter**

```rust
use memchr::memmem;

static COMPILATION_KEYWORDS: &[&str] = &[
    "cargo", "rustc",           // Rust
    "gcc", "g++", "clang",      // C/C++
    "make", "cmake", "ninja",   // Build systems
    "meson", "bazel",           // Modern build systems
];

/// Fast SIMD-accelerated keyword check
pub fn might_be_compilation(cmd: &str) -> bool {
    let bytes = cmd.as_bytes();
    COMPILATION_KEYWORDS.iter().any(|kw| {
        memmem::find(bytes, kw.as_bytes()).is_some()
    })
}
```

**Tier 4: Full Classification with Confidence**

```rust
use regex::RegexSet;

/// Classification result with confidence scoring
#[derive(Debug)]
pub struct ClassificationResult {
    /// What type of compilation this is
    pub category: CommandCategory,

    /// Confidence score (0.0 - 1.0)
    /// Higher = more certain this is a compilation command
    pub confidence: f64,

    /// Reasons for the classification (for debugging/logging)
    pub reasons: Vec<String>,

    /// Whether interception is recommended
    pub recommend_intercept: bool,
}

#[derive(Debug, Clone)]
pub enum CommandCategory {
    NotCompilation,
    Cargo(CargoCommand),
    Rustc(RustcCommand),
    Gcc(GccCommand),
    Clang(ClangCommand),
    Make(MakeCommand),
    CMake(CMakeCommand),
    Ninja,
    Meson,
    Unknown,
}

lazy_static! {
    static ref PATTERNS: RegexSet = RegexSet::new(&[
        // === Cargo (indices 0-1) ===
        r"^cargo\s+(build|test|check|clippy|doc|bench)\b",
        r"^cargo\s+\+\S+\s+(build|test|check|clippy|doc|bench)\b",

        // === cargo run - special handling (index 2) ===
        // Only intercept cargo run if NOT a server/daemon
        r"^cargo\s+run\b(?!.*(--(serve|server|listen|daemon|watch)|-[sd]))",

        // === rustc (index 3) ===
        r"^rustc\s+",

        // === GCC (indices 4-5) ===
        r"^gcc\s+.*\.(c|h)\b",
        r"^g\+\+\s+.*\.(cpp|cc|cxx|hpp|C)\b",

        // === Clang (indices 6-7) ===
        r"^clang\s+.*\.(c|h)\b",
        r"^clang\+\+\s+.*\.(cpp|cc|cxx|hpp|C)\b",

        // === Build systems (indices 8-11) ===
        r"^make\s+(?!clean|distclean|install|uninstall)",
        r"^cmake\s+--build\b",
        r"^ninja\b(?!\s+(-t|--help|-v|--version))",
        r"^meson\s+compile\b",
    ]).unwrap();
}

pub fn classify_full(cmd: &str) -> ClassificationResult {
    let normalized = normalize_command(cmd);
    let matches: Vec<usize> = PATTERNS.matches(&normalized).into_iter().collect();

    if matches.is_empty() {
        return ClassificationResult {
            category: CommandCategory::NotCompilation,
            confidence: 0.0,
            reasons: vec!["No pattern match".into()],
            recommend_intercept: false,
        };
    }

    // Calculate base confidence from pattern match
    let (category, base_confidence) = match matches[0] {
        0 | 1 => (CommandCategory::Cargo(parse_cargo(cmd)), 0.95),
        2 => (CommandCategory::Cargo(parse_cargo(cmd)), 0.80),  // cargo run - lower confidence
        3 => (CommandCategory::Rustc(parse_rustc(cmd)), 0.90),
        4 | 5 => (CommandCategory::Gcc(parse_gcc(cmd)), 0.90),
        6 | 7 => (CommandCategory::Clang(parse_clang(cmd)), 0.90),
        8 => (CommandCategory::Make(parse_make(cmd)), 0.85),
        9 => (CommandCategory::CMake(parse_cmake(cmd)), 0.90),
        10 => (CommandCategory::Ninja, 0.90),
        11 => (CommandCategory::Meson, 0.90),
        _ => (CommandCategory::Unknown, 0.50),
    };

    // Adjust confidence based on context clues
    let mut confidence = base_confidence;
    let mut reasons = vec![format!("Pattern match: {:?}", category)];

    // Boost confidence for common compilation flags
    if cmd.contains("--release") || cmd.contains("-O") {
        confidence = (confidence + 0.05).min(1.0);
        reasons.push("Has optimization flags".into());
    }

    // Lower confidence for ambiguous cases
    if cmd.contains("--") && cmd.contains("--help") {
        confidence = 0.0;
        reasons.push("Help flag detected".into());
    }

    ClassificationResult {
        category,
        confidence,
        reasons,
        recommend_intercept: confidence >= 0.85,
    }
}
```

**The Complete Classification Pipeline:**

```rust
impl RchClassifier {
    /// Main entry point for command classification
    pub async fn classify(&self, tool_name: &str, cmd: &str) -> ClassificationDecision {
        // Tier 0: Instant reject
        if tool_name != "Bash" {
            return ClassificationDecision::pass_through("Not a Bash command");
        }

        if instant_reject(cmd) {
            return ClassificationDecision::pass_through("Instant reject");
        }

        // Tier 1: Structure analysis
        let structure = analyze_structure(cmd);
        if !structure.allows_interception() {
            return ClassificationDecision::pass_through(format!(
                "Structure not interceptable: {:?}", structure
            ));
        }

        // Get the actual command to classify (may be subset of full command)
        let target_cmd = match structure.interceptable_command() {
            Some(cmd) => cmd,
            None => return ClassificationDecision::pass_through("No interceptable portion"),
        };

        // Tier 2: Quick keyword filter
        if !might_be_compilation(target_cmd) {
            return ClassificationDecision::pass_through("No compilation keywords");
        }

        // Tier 3: Negative pattern check
        if matches_never_intercept(target_cmd) {
            return ClassificationDecision::pass_through("Matches never-intercept list");
        }

        // Check project-level exclusions
        if let Some(exclusions) = self.project_config.exclusions() {
            if exclusions.matches(target_cmd) {
                return ClassificationDecision::pass_through("Project-level exclusion");
            }
        }

        // Tier 4: Full classification
        let result = classify_full(target_cmd);

        // Apply confidence threshold (configurable, default 0.85)
        let threshold = self.config.confidence_threshold.unwrap_or(0.85);

        if result.confidence >= threshold && result.recommend_intercept {
            ClassificationDecision::intercept(result)
        } else {
            ClassificationDecision::pass_through(format!(
                "Below confidence threshold: {:.2} < {:.2}",
                result.confidence, threshold
            ))
        }
    }
}

#[derive(Debug)]
pub enum ClassificationDecision {
    Intercept {
        result: ClassificationResult,
    },
    PassThrough {
        reason: String,
    },
}
```

### 5.4 Command Normalization

Normalize commands to handle wrappers, paths, and shell constructs:

```rust
pub fn normalize_command(cmd: &str) -> Cow<'_, str> {
    let mut result = cmd.trim();

    // Strip common command prefixes/wrappers
    let wrappers = [
        "sudo ",
        "env ",
        "time ",
        "nice ",
        "ionice ",
        "strace ",
        "ltrace ",
        "perf ",
        "taskset ",
        "numactl ",
    ];

    for wrapper in wrappers {
        while result.starts_with(wrapper) {
            result = result.strip_prefix(wrapper).unwrap().trim_start();
        }
    }

    // Handle env VAR=value syntax
    while let Some(eq_pos) = result.find('=') {
        let space_pos = result.find(' ').unwrap_or(result.len());
        if eq_pos < space_pos && !result[..eq_pos].contains('"') {
            // This is VAR=value at start, skip it
            result = result[space_pos..].trim_start();
        } else {
            break;
        }
    }

    // Strip absolute paths: /usr/bin/cargo → cargo
    if result.starts_with('/') {
        if let Some(idx) = result.find(|c: char| c.is_whitespace()) {
            let path_part = &result[..idx];
            if let Some(slash_idx) = path_part.rfind('/') {
                result = &result[slash_idx + 1..];
            }
        } else if let Some(slash_idx) = result.rfind('/') {
            result = &result[slash_idx + 1..];
        }
    }

    if result == cmd {
        Cow::Borrowed(cmd)
    } else {
        Cow::Owned(result.to_string())
    }
}
```

### 5.5 Smart Command Decomposition

For chained commands, RCH can intercept only the compilation portion while letting the rest execute locally:

```rust
/// Decompose a chained command and determine which parts to intercept
pub fn decompose_for_interception(cmd: &str) -> InterceptionPlan {
    let structure = analyze_structure(cmd);

    match structure {
        CommandStructure::Simple(cmd) => {
            InterceptionPlan::single(cmd, should_intercept(&cmd))
        }

        CommandStructure::Chained { commands } => {
            let mut plan = InterceptionPlan::new();

            for (i, (part, op)) in commands.iter().enumerate() {
                let classification = classify_full(part);

                // Only intercept if:
                // 1. It's a compilation command with high confidence
                // 2. The chain operator allows it (e.g., && but not ||)
                // 3. It's the first command (subsequent depend on artifacts)
                let can_intercept = i == 0
                    && classification.recommend_intercept
                    && op.allows_interception();

                plan.add(part.clone(), can_intercept, *op);
            }

            plan
        }

        // Other structures: don't intercept
        _ => InterceptionPlan::pass_through(cmd),
    }
}

impl ChainOperator {
    pub fn allows_interception(&self) -> bool {
        match self {
            // && - can intercept first command, exit code passed through
            ChainOperator::And => true,

            // || - second command depends on first FAILING, tricky semantics
            ChainOperator::Or => false,

            // ; - commands are independent, safe to intercept first
            ChainOperator::Sequence => true,
        }
    }
}

/// Execute a decomposed command plan
pub async fn execute_plan(
    plan: &InterceptionPlan,
    ctx: &ExecutionContext,
    daemon: &Daemon,
) -> Result<PlanResult> {
    let mut results = Vec::new();
    let mut last_exit_code = 0;

    for step in &plan.steps {
        // Check if we should continue based on previous exit code and operator
        let should_run = match step.chain_op {
            Some(ChainOperator::And) => last_exit_code == 0,
            Some(ChainOperator::Or) => last_exit_code != 0,
            Some(ChainOperator::Sequence) | None => true,
        };

        if !should_run {
            break;
        }

        let exit_code = if step.intercept {
            // Execute remotely
            execute_remote(&step.command, ctx, daemon).await?
        } else {
            // Execute locally
            execute_local(&step.command, ctx).await?
        };

        results.push(StepResult {
            command: step.command.clone(),
            intercepted: step.intercept,
            exit_code,
        });

        last_exit_code = exit_code;
    }

    Ok(PlanResult { steps: results })
}
```

### 5.6 Context Extraction

After classification, extract full context needed for remote execution:

```rust
pub struct CompilationContext {
    /// Working directory where command was invoked
    pub cwd: PathBuf,

    /// Project root (contains Cargo.toml, Makefile, etc.)
    pub project_root: PathBuf,

    /// Target directory for artifacts
    pub target_dir: PathBuf,

    /// The exact command to execute
    pub command: String,

    /// Parsed command arguments
    pub args: CommandArgs,

    /// Environment variables needed for compilation
    pub env: HashMap<String, String>,

    /// Estimated core usage for slot reservation
    pub estimated_cores: u32,

    /// Classification result (for logging/metrics)
    pub classification: ClassificationResult,

    /// Project-level RCH configuration
    pub project_config: Option<ProjectConfig>,
}

impl CompilationContext {
    pub fn from_cargo(cmd: &str, cwd: &Path, classification: ClassificationResult) -> Result<Self> {
        let project_root = find_cargo_toml_root(cwd)?;
        let target_dir = project_root.join("target");

        // Load project-level config if present
        let project_config = ProjectConfig::load(&project_root).ok();

        // Parse -j flag or default to num_cpus
        let estimated_cores = parse_jobs_flag(cmd).unwrap_or_else(num_cpus::get);

        Ok(Self {
            cwd: cwd.to_path_buf(),
            project_root,
            target_dir,
            command: cmd.to_string(),
            args: parse_cargo_args(cmd),
            env: collect_rust_env(),
            estimated_cores: estimated_cores as u32,
            classification,
            project_config,
        })
    }

    /// Compute a cache key for this specific compilation
    pub fn cache_key(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(self.project_root.to_string_lossy().as_bytes());
        hasher.update(self.command.as_bytes());

        // Include git state if available
        if let Ok(git_hash) = get_git_hash(&self.project_root) {
            hasher.update(git_hash.as_bytes());
        }

        format!("{}_{}",
            self.project_root.file_name().unwrap().to_string_lossy(),
            &hasher.finalize().to_hex()[..12]
        )
    }
}
```

### 5.7 Dry Run Mode

For debugging and building trust, RCH supports a dry-run mode:

```rust
/// When RCH_DRY_RUN=1, show what would happen without doing it
pub async fn maybe_dry_run(
    cmd: &str,
    decision: &ClassificationDecision,
    ctx: Option<&CompilationContext>,
    daemon: &Daemon,
) -> Option<HookOutput> {
    if std::env::var("RCH_DRY_RUN").is_err() {
        return None;
    }

    eprintln!("\x1b[36m╭─ RCH DRY RUN ─────────────────────────────────────────╮\x1b[0m");
    eprintln!("\x1b[36m│\x1b[0m Command: {}", truncate(cmd, 50));

    match decision {
        ClassificationDecision::Intercept { result } => {
            eprintln!("\x1b[36m│\x1b[0m Decision: \x1b[32mWOULD INTERCEPT\x1b[0m");
            eprintln!("\x1b[36m│\x1b[0m Category: {:?}", result.category);
            eprintln!("\x1b[36m│\x1b[0m Confidence: {:.1}%", result.confidence * 100.0);

            if let Some(ctx) = ctx {
                eprintln!("\x1b[36m│\x1b[0m Project: {}", ctx.project_root.display());

                if let Some(worker) = daemon.select_worker(ctx).await {
                    eprintln!("\x1b[36m│\x1b[0m Worker: {} ({} slots available)",
                        worker.id, worker.available_slots());
                }
            }
        }
        ClassificationDecision::PassThrough { reason } => {
            eprintln!("\x1b[36m│\x1b[0m Decision: \x1b[33mPASS THROUGH\x1b[0m");
            eprintln!("\x1b[36m│\x1b[0m Reason: {}", reason);
        }
    }

    eprintln!("\x1b[36m│\x1b[0m \x1b[90m(Running locally - dry run mode)\x1b[0m");
    eprintln!("\x1b[36m╰───────────────────────────────────────────────────────╯\x1b[0m");

    // Always pass through in dry-run mode
    Some(HookOutput::allow())
}

---

## 6. Worker Fleet Management

### 6.1 Worker Configuration

Define workers in `~/.config/rch/workers.toml`:

```toml
# ~/.config/rch/workers.toml

[defaults]
slots_per_core = 1.0
ssh_timeout_ms = 5000
heartbeat_interval_sec = 30

[[workers]]
id = "css"
host = "203.0.113.20"
user = "ubuntu"
identity_file = "~/.ssh/contabo_new_baremetal_superserver_box.pem"
total_slots = 32  # Override auto-detection
priority = 100    # Higher = preferred

[[workers]]
id = "csd"
host = "198.51.100.20"
user = "ubuntu"
identity_file = "~/.ssh/contabo_new_baremetal_sense_demo_box.pem"
# total_slots auto-detected from nproc

[[workers]]
id = "yto"
host = "203.0.113.30"
user = "ubuntu"
identity_file = "~/.ssh/je_ovh_ssh_key.pem"

[[workers]]
id = "fmd"
host = "192.0.2.40"
user = "ubuntu"
identity_file = "~/.ssh/je_ovh_ssh_key.pem"
```

**Alternative: Parse from SSH config and zshrc aliases**

```rust
pub fn discover_workers() -> Vec<WorkerConfig> {
    let mut workers = Vec::new();

    // 1. Parse ~/.ssh/config
    if let Ok(ssh_config) = parse_ssh_config(home_dir().join(".ssh/config")) {
        for host in ssh_config.hosts() {
            if is_compilation_worker(&host) {
                workers.push(WorkerConfig::from_ssh_host(&host));
            }
        }
    }

    // 2. Parse zshrc aliases
    if let Ok(aliases) = parse_zshrc_aliases() {
        for (name, command) in aliases {
            if command.starts_with("ssh ") {
                if let Some(worker) = parse_ssh_alias(&name, &command) {
                    workers.push(worker);
                }
            }
        }
    }

    workers
}
```

### 6.2 Slot Management

**Slot Semantics:**
- 1 slot ≈ 1 CPU core worth of compilation work
- `cargo build` typically uses `num_cpus` parallelism
- Prevent oversubscription that causes kernel contention

```rust
impl WorkerState {
    pub fn try_reserve_slots(&self, count: u32) -> Result<SlotReservation, SlotError> {
        loop {
            let current = self.used_slots.load(Ordering::Acquire);
            let new = current + count;

            if new > self.total_slots {
                return Err(SlotError::InsufficientCapacity {
                    requested: count,
                    available: self.total_slots - current,
                });
            }

            if self.used_slots.compare_exchange_weak(
                current, new,
                Ordering::AcqRel, Ordering::Relaxed
            ).is_ok() {
                return Ok(SlotReservation {
                    worker_id: self.id.clone(),
                    slots: count,
                    reserved_at: Instant::now(),
                });
            }
        }
    }

    pub fn release_slots(&self, reservation: SlotReservation) {
        self.used_slots.fetch_sub(reservation.slots, Ordering::AcqRel);
    }
}
```

### 6.3 Health Monitoring

Periodic health checks to detect worker failures:

```rust
async fn health_check_loop(state: Arc<DaemonState>) {
    let interval = Duration::from_secs(30);

    loop {
        for worker in state.workers.values() {
            let health = check_worker_health(worker).await;

            match health {
                Health::Healthy { latency_ms } => {
                    worker.update_status(WorkerStatus::Healthy);
                    worker.consecutive_failures.store(0, Ordering::Relaxed);
                    state.metrics.record_heartbeat_latency(worker.id, latency_ms);
                }

                Health::Degraded { reason } => {
                    worker.update_status(WorkerStatus::Degraded);
                    warn!("Worker {} degraded: {}", worker.id, reason);
                }

                Health::Unreachable => {
                    let failures = worker.consecutive_failures.fetch_add(1, Ordering::AcqRel);
                    if failures >= 3 {
                        worker.update_status(WorkerStatus::Unreachable);
                        warn!("Worker {} marked unreachable after {} failures",
                              worker.id, failures);
                    }
                }
            }
        }

        tokio::time::sleep(interval).await;
    }
}

async fn check_worker_health(worker: &Worker) -> Health {
    let start = Instant::now();

    match ssh_command(worker, "echo ok").timeout(Duration::from_secs(5)).await {
        Ok(output) if output.trim() == "ok" => {
            Health::Healthy { latency_ms: start.elapsed().as_millis() as u32 }
        }
        Ok(output) => {
            Health::Degraded { reason: format!("Unexpected output: {}", output) }
        }
        Err(_) => Health::Unreachable,
    }
}
```

### 6.4 Speed Scoring Integration

Integrate with [cloud_benchmarker](https://github.com/Dicklesworthstone/cloud_benchmarker):

```rust
/// Fetches performance scores from cloud_benchmarker API
pub async fn fetch_speed_scores(benchmarker_url: &str) -> Result<HashMap<String, f64>> {
    let client = reqwest::Client::new();

    let response: Vec<BenchmarkResult> = client
        .get(format!("{}/data/overall/", benchmarker_url))
        .send()
        .await?
        .json()
        .await?;

    let scores = response
        .into_iter()
        .map(|r| (r.hostname, r.overall_score))
        .collect();

    Ok(scores)
}

#[derive(Deserialize)]
struct BenchmarkResult {
    hostname: String,
    overall_score: f64,
    cpu_score: f64,
    memory_score: f64,
    disk_io_score: f64,
}
```

**Or run lightweight inline benchmark:**

```rust
/// Quick benchmark using single-threaded CPU + memory test
async fn quick_benchmark(worker: &Worker) -> f64 {
    let script = r#"
        # CPU: Single-threaded sysbench
        cpu=$(sysbench cpu --time=1 run 2>/dev/null | grep 'events per second' | awk '{print $4}')

        # Memory: Bandwidth test
        mem=$(sysbench memory --time=1 run 2>/dev/null | grep 'MiB transferred' | awk '{print $1}')

        echo "$cpu $mem"
    "#;

    let output = ssh_command(worker, script).await.unwrap_or_default();
    let parts: Vec<&str> = output.split_whitespace().collect();

    let cpu_score = parts.get(0).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
    let mem_score = parts.get(1).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);

    // Normalize to 0-100 scale (adjust based on expected hardware)
    let normalized_cpu = (cpu_score / 10000.0 * 100.0).min(100.0);
    let normalized_mem = (mem_score / 50000.0 * 100.0).min(100.0);

    // Weighted average (CPU more important for compilation)
    0.7 * normalized_cpu + 0.3 * normalized_mem
}
```

### 6.5 Worker Affinity & Project Locality

One of the most impactful optimizations is routing compilations to workers that already have a cached copy of the project. This avoids the transfer phase entirely for incremental builds.

**Affinity Tracking:**

```rust
/// Tracks which workers have cached copies of which projects
pub struct WorkerAffinity {
    /// Maps project_key -> Vec<(worker_id, last_used, cache_freshness)>
    project_locations: DashMap<String, Vec<AffinityEntry>>,

    /// Maps worker_id -> set of cached project keys
    worker_projects: DashMap<WorkerId, HashSet<String>>,
}

#[derive(Debug, Clone)]
pub struct AffinityEntry {
    pub worker_id: WorkerId,
    pub last_used: Instant,
    pub cache_freshness: CacheFreshness,
    pub hit_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CacheFreshness {
    /// Exact match: project hash matches, no transfer needed
    Exact,
    /// Stale: same project but different version, incremental rsync needed
    Stale,
    /// Unknown: need to verify with remote
    Unknown,
}

impl WorkerAffinity {
    /// Record that a worker now has a cached copy of a project
    pub fn record_cache(&self, project_key: &str, worker_id: &WorkerId, freshness: CacheFreshness) {
        self.project_locations
            .entry(project_key.to_string())
            .or_default()
            .push(AffinityEntry {
                worker_id: worker_id.clone(),
                last_used: Instant::now(),
                cache_freshness: freshness,
                hit_count: 1,
            });

        self.worker_projects
            .entry(worker_id.clone())
            .or_default()
            .insert(project_key.to_string());
    }

    /// Find workers with cached copies of a project, ordered by preference
    pub fn find_cached_workers(&self, project_key: &str) -> Vec<WorkerId> {
        self.project_locations
            .get(project_key)
            .map(|entries| {
                let mut sorted: Vec<_> = entries.iter().collect();
                // Prefer: Exact > Stale > Unknown, then by hit_count, then by recency
                sorted.sort_by(|a, b| {
                    match (&a.cache_freshness, &b.cache_freshness) {
                        (CacheFreshness::Exact, CacheFreshness::Exact) => {
                            b.hit_count.cmp(&a.hit_count)
                                .then_with(|| b.last_used.cmp(&a.last_used))
                        }
                        (CacheFreshness::Exact, _) => Ordering::Less,
                        (_, CacheFreshness::Exact) => Ordering::Greater,
                        (CacheFreshness::Stale, CacheFreshness::Stale) => {
                            b.hit_count.cmp(&a.hit_count)
                        }
                        (CacheFreshness::Stale, _) => Ordering::Less,
                        (_, CacheFreshness::Stale) => Ordering::Greater,
                        _ => b.hit_count.cmp(&a.hit_count),
                    }
                });
                sorted.into_iter().map(|e| e.worker_id.clone()).collect()
            })
            .unwrap_or_default()
    }
}
```

**Affinity-Aware Selection:**

```rust
impl DaemonState {
    /// Select optimal worker considering affinity, slots, and speed
    pub async fn select_worker_with_affinity(
        &self,
        ctx: &CompilationContext,
    ) -> Option<(&Worker, SelectionReason)> {
        let project_key = ctx.cache_key();

        // Priority 1: Worker with exact cache match and available slots
        let cached_workers = self.affinity.find_cached_workers(&project_key);

        for worker_id in &cached_workers {
            if let Some(worker) = self.workers.get(worker_id) {
                if worker.status == WorkerStatus::Healthy
                    && worker.available_slots() >= ctx.estimated_cores
                {
                    return Some((worker, SelectionReason::AffinityExact));
                }
            }
        }

        // Priority 2: Worker with stale cache (same project, old version)
        // Incremental rsync will be fast
        let project_prefix = ctx.project_root.file_name().unwrap().to_string_lossy();
        for (key, entries) in self.affinity.project_locations.iter() {
            if key.starts_with(&*project_prefix) {
                for entry in entries.iter() {
                    if let Some(worker) = self.workers.get(&entry.worker_id) {
                        if worker.status == WorkerStatus::Healthy
                            && worker.available_slots() >= ctx.estimated_cores
                        {
                            return Some((worker, SelectionReason::AffinityStale));
                        }
                    }
                }
            }
        }

        // Priority 3: Standard selection (no affinity)
        self.select_optimal_worker(ctx)
            .map(|w| (w, SelectionReason::Standard))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SelectionReason {
    /// Worker had exact cached copy
    AffinityExact,
    /// Worker had same project, different version
    AffinityStale,
    /// Standard selection (slots + speed)
    Standard,
    /// Fallback after preferred workers unavailable
    Fallback,
}
```

### 6.6 Compilation Deduplication

When multiple agents compile the same project simultaneously, RCH can share the result rather than running redundant compilations. This is especially valuable in multi-agent scenarios.

```rust
/// Tracks in-flight compilations to enable result sharing
pub struct InFlightCompilations {
    /// Maps project_hash -> (job_id, result_channel)
    active: DashMap<String, InFlightJob>,
}

struct InFlightJob {
    job_id: JobId,
    command: String,
    started_at: Instant,
    /// Broadcast channel for subscribers waiting on this compilation
    result_tx: broadcast::Sender<CompilationEvent>,
}

#[derive(Clone)]
pub enum CompilationEvent {
    /// Compilation in progress, here's some output
    Output { line: String, stream: OutputStream },
    /// Compilation completed
    Completed { exit_code: i32, artifacts_ready: bool },
    /// Compilation failed
    Failed { error: String },
}

impl InFlightCompilations {
    /// Try to join an existing compilation for the same project+command
    pub fn try_join(
        &self,
        project_hash: &str,
        command: &str,
    ) -> Option<broadcast::Receiver<CompilationEvent>> {
        self.active.get(project_hash).and_then(|job| {
            // Only join if same command
            if job.command == command {
                Some(job.result_tx.subscribe())
            } else {
                None
            }
        })
    }

    /// Register a new in-flight compilation
    pub fn register(&self, project_hash: &str, command: &str) -> broadcast::Sender<CompilationEvent> {
        let (tx, _) = broadcast::channel(256);

        self.active.insert(
            project_hash.to_string(),
            InFlightJob {
                job_id: JobId::new(),
                command: command.to_string(),
                started_at: Instant::now(),
                result_tx: tx.clone(),
            },
        );

        tx
    }

    /// Mark compilation as complete and remove from tracking
    pub fn complete(&self, project_hash: &str) {
        self.active.remove(project_hash);
    }
}
```

**Deduplication in Practice:**

```rust
impl Daemon {
    pub async fn execute_compilation(
        &self,
        ctx: &CompilationContext,
    ) -> Result<CompilationResult> {
        let project_hash = ctx.cache_key();

        // Check if identical compilation already in flight
        if let Some(mut rx) = self.in_flight.try_join(&project_hash, &ctx.command) {
            info!(
                "Deduplicating compilation for {} - joining existing job",
                project_hash
            );

            // Wait for existing compilation and replay output
            while let Ok(event) = rx.recv().await {
                match event {
                    CompilationEvent::Output { line, stream } => {
                        match stream {
                            OutputStream::Stdout => println!("{}", line),
                            OutputStream::Stderr => eprintln!("{}", line),
                        }
                    }
                    CompilationEvent::Completed { exit_code, .. } => {
                        return if exit_code == 0 {
                            Ok(CompilationResult::Success)
                        } else {
                            Ok(CompilationResult::Failed { exit_code })
                        };
                    }
                    CompilationEvent::Failed { error } => {
                        return Err(CompilationError::RemoteFailed(error));
                    }
                }
            }

            return Err(CompilationError::ChannelClosed);
        }

        // No existing compilation - start new one
        let result_tx = self.in_flight.register(&project_hash, &ctx.command);

        let result = self.execute_new_compilation(ctx, &result_tx).await;

        self.in_flight.complete(&project_hash);

        result
    }
}
```

### 6.7 Cost-Benefit Analysis

Not every compilation benefits from remote execution. For very quick builds, the transfer overhead may exceed local execution time. RCH performs a cost-benefit analysis before deciding to intercept.

**Cost Model:**

```rust
/// Estimates whether remote execution is beneficial
pub struct CostEstimator {
    /// Historical compilation times for this project
    history: CompilationHistory,
    /// Network characteristics to each worker
    network_stats: NetworkStats,
}

#[derive(Debug)]
pub struct CostEstimate {
    /// Estimated time to transfer code to worker
    pub transfer_time_ms: u64,
    /// Estimated remote compilation time
    pub remote_compile_time_ms: u64,
    /// Estimated time to return artifacts
    pub artifact_return_time_ms: u64,
    /// Total estimated remote time
    pub total_remote_ms: u64,
    /// Estimated local compilation time
    pub estimated_local_ms: u64,
    /// Whether remote is recommended
    pub recommend_remote: bool,
    /// Explanation for the decision
    pub reasoning: String,
}

impl CostEstimator {
    pub fn estimate(&self, ctx: &CompilationContext, worker: &Worker) -> CostEstimate {
        // === Transfer Time Estimate ===
        let project_size_bytes = estimate_transfer_size(&ctx.project_root);
        let network_speed_bps = self.network_stats.bandwidth_to(worker);
        let transfer_time_ms = if self.has_affinity(ctx, worker) {
            // Incremental transfer - assume 5% of full size
            (project_size_bytes as f64 * 0.05 / network_speed_bps * 1000.0) as u64
        } else {
            (project_size_bytes as f64 / network_speed_bps * 1000.0) as u64
        };

        // === Compilation Time Estimate ===
        let base_compile_time = self.history
            .get_average_time(&ctx.project_root, &ctx.command)
            .unwrap_or_else(|| heuristic_compile_time(ctx));

        // Adjust for worker speed (relative to local)
        let local_speed = 100.0;  // Baseline
        let speed_ratio = worker.speed_score / local_speed;
        let remote_compile_time_ms = (base_compile_time as f64 / speed_ratio) as u64;

        // === Artifact Return Estimate ===
        let artifact_size = estimate_artifact_size(ctx);
        let artifact_return_time_ms = (artifact_size as f64 / network_speed_bps * 1000.0) as u64;

        // === Total & Decision ===
        let total_remote_ms = transfer_time_ms + remote_compile_time_ms + artifact_return_time_ms;
        let estimated_local_ms = base_compile_time;

        // Remote should be at least 20% faster to be worth the complexity
        let threshold = 0.80;
        let recommend_remote = (total_remote_ms as f64) < (estimated_local_ms as f64 * threshold);

        let reasoning = if recommend_remote {
            format!(
                "Remote ({:.1}s) is {:.0}% faster than local ({:.1}s)",
                total_remote_ms as f64 / 1000.0,
                (1.0 - total_remote_ms as f64 / estimated_local_ms as f64) * 100.0,
                estimated_local_ms as f64 / 1000.0
            )
        } else {
            format!(
                "Local ({:.1}s) preferred over remote ({:.1}s) due to overhead",
                estimated_local_ms as f64 / 1000.0,
                total_remote_ms as f64 / 1000.0
            )
        };

        CostEstimate {
            transfer_time_ms,
            remote_compile_time_ms,
            artifact_return_time_ms,
            total_remote_ms,
            estimated_local_ms,
            recommend_remote,
            reasoning,
        }
    }

    fn has_affinity(&self, ctx: &CompilationContext, worker: &Worker) -> bool {
        // Check if worker has cached copy
        self.affinity
            .find_cached_workers(&ctx.cache_key())
            .contains(&worker.id)
    }
}

fn heuristic_compile_time(ctx: &CompilationContext) -> u64 {
    // Rough heuristics based on command type
    match &ctx.command {
        cmd if cmd.contains("cargo check") => 5_000,      // 5s
        cmd if cmd.contains("cargo clippy") => 10_000,    // 10s
        cmd if cmd.contains("cargo test") => 30_000,      // 30s
        cmd if cmd.contains("--release") => 120_000,      // 2min
        cmd if cmd.contains("cargo build") => 30_000,     // 30s
        cmd if cmd.contains("cargo doc") => 60_000,       // 1min
        cmd if cmd.contains("make") => 60_000,            // 1min
        _ => 30_000,                                       // 30s default
    }
}
```

**Minimum Threshold Configuration:**

```toml
# ~/.config/rch/config.toml

[cost_benefit]
# Minimum estimated compilation time (ms) before considering remote
# Quick commands (<2s) usually aren't worth the overhead
min_compile_time_ms = 2000

# Remote must be this much faster than local to be chosen
# 0.8 = remote must be 20% faster
benefit_threshold = 0.80

# Always use remote for these patterns (skip cost analysis)
always_remote = [
    "cargo build --release",
    "cargo test --release",
]

# Never use remote for these patterns
never_remote = [
    "cargo check",  # Usually fast enough locally
]
```

### 6.8 Unified Worker Selection

Bringing it all together—the final worker selection algorithm considers affinity, cost-benefit, slots, health, and speed:

```rust
impl DaemonState {
    pub async fn select_worker(&self, ctx: &CompilationContext) -> Option<WorkerSelection> {
        // Step 1: Cost-benefit check - should we even go remote?
        let local_estimate = self.cost_estimator.estimate_local(ctx);
        if local_estimate < self.config.min_compile_time_ms {
            return None;  // Too quick for remote
        }

        // Step 2: Find candidate workers
        let mut candidates: Vec<_> = self.workers
            .values()
            .filter(|w| w.status == WorkerStatus::Healthy)
            .filter(|w| w.available_slots() >= ctx.estimated_cores)
            .collect();

        if candidates.is_empty() {
            return None;  // No available workers
        }

        // Step 3: Score each candidate
        let mut scored: Vec<_> = candidates
            .into_iter()
            .map(|w| {
                let cost = self.cost_estimator.estimate(ctx, w);
                let affinity = self.affinity.score_for(ctx, w);
                let score = compute_selection_score(w, &cost, affinity);
                (w, score, cost)
            })
            .filter(|(_, _, cost)| cost.recommend_remote)
            .collect();

        if scored.is_empty() {
            return None;  // Remote not beneficial for any worker
        }

        // Step 4: Select best
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

        let (worker, score, cost) = scored.into_iter().next()?;

        Some(WorkerSelection {
            worker_id: worker.id.clone(),
            reason: SelectionReason::Optimal,
            score,
            cost_estimate: cost,
        })
    }
}

fn compute_selection_score(worker: &Worker, cost: &CostEstimate, affinity: f64) -> f64 {
    // Weights (configurable)
    let w_speed = 0.35;
    let w_slots = 0.25;
    let w_affinity = 0.25;
    let w_cost = 0.15;

    let speed_score = worker.speed_score / 100.0;
    let slot_score = worker.available_slots() as f64 / worker.total_slots as f64;
    let cost_score = 1.0 - (cost.total_remote_ms as f64 / cost.estimated_local_ms as f64);

    w_speed * speed_score +
    w_slots * slot_score +
    w_affinity * affinity +
    w_cost * cost_score.max(0.0)
}
```

---

## 7. Code Transfer Pipeline

### 7.1 Transfer Strategy

**Optimization Goals:**
1. Minimize bytes transferred (delta encoding)
2. Maximize compression ratio (zstd)
3. Avoid transferring build artifacts (target/, node_modules/)
4. Support resumable transfers for large codebases

**Pipeline:**

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      CODE TRANSFER PIPELINE                              │
└─────────────────────────────────────────────────────────────────────────┘

 LOCAL                                                           REMOTE
┌──────────────────────────────────────────────────────────────┐
│ 1. Identify project root (Cargo.toml, .git, etc.)            │
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────┐
│ 2. Compute content hash for cache key                        │
│    • Hash of .git/HEAD + staged changes                      │
│    • Or hash of all source files if no git                   │
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────┐
│ 3. Check remote cache                                        │
│    • SSH: test -d /tmp/rch/${project}_${hash}               │
│    • If exists and fresh: skip transfer                      │
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────┐
│ 4. Generate exclude list                                     │
│    • target/                                                 │
│    • .git/objects/ (large, not needed)                       │
│    • node_modules/                                           │
│    • *.rlib, *.rmeta, *.o, *.a (build artifacts)            │
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────┐
│ 5. rsync with zstd compression                               │
│                                                              │
│    rsync -az --zc=zstd --zl=3 \                             │
│          --exclude-from=.rch-exclude \                       │
│          --delete \                                          │
│          -e "ssh -o ControlMaster=auto" \                   │
│          ./  worker:/tmp/rch/project_hash/                  │
└──────────────────────────────────────────────────────────────┘
```

### 7.2 rsync Integration

```rust
use std::process::Command;

pub struct TransferConfig {
    pub compression_level: u8,  // 1-19, default 3
    pub exclude_patterns: Vec<String>,
    pub delete_extraneous: bool,
    pub checksum: bool,  // Use checksums vs. mod-time (slower but more accurate)
}

impl Default for TransferConfig {
    fn default() -> Self {
        Self {
            compression_level: 3,
            exclude_patterns: vec![
                "target/".into(),
                ".git/objects/".into(),
                "node_modules/".into(),
                "*.rlib".into(),
                "*.rmeta".into(),
                "*.o".into(),
                "*.a".into(),
                "*.so".into(),
                "*.dylib".into(),
                ".rch/".into(),
            ],
            delete_extraneous: true,
            checksum: false,
        }
    }
}

pub async fn transfer_project(
    project: &Path,
    worker: &Worker,
    remote_path: &str,
    config: &TransferConfig,
) -> Result<TransferStats> {
    let mut cmd = Command::new("rsync");

    cmd.arg("-az")  // archive + compress
       .arg(format!("--zc=zstd"))
       .arg(format!("--zl={}", config.compression_level));

    if config.delete_extraneous {
        cmd.arg("--delete");
    }

    if config.checksum {
        cmd.arg("-c");
    }

    // Add excludes
    for pattern in &config.exclude_patterns {
        cmd.arg(format!("--exclude={}", pattern));
    }

    // SSH with multiplexing
    cmd.arg("-e")
       .arg(format!(
           "ssh -o ControlMaster=auto -o ControlPath=/tmp/rch-ssh-%r@%h:%p -o ControlPersist=600 -i {}",
           worker.identity_file.display()
       ));

    // Source and destination
    cmd.arg(format!("{}/", project.display()))
       .arg(format!("{}@{}:{}", worker.user, worker.host, remote_path));

    // Progress output
    cmd.arg("--info=progress2");

    let start = Instant::now();
    let output = cmd.output().await?;

    if !output.status.success() {
        return Err(TransferError::RsyncFailed {
            stderr: String::from_utf8_lossy(&output.stderr).into(),
        });
    }

    Ok(TransferStats {
        duration: start.elapsed(),
        bytes_transferred: parse_rsync_stats(&output.stdout),
    })
}
```

### 7.3 Intelligent Caching

Avoid redundant transfers using content-addressable caching:

```rust
/// Computes a cache key based on project content
pub fn compute_cache_key(project: &Path) -> Result<String> {
    // Strategy 1: Use git commit hash + index state
    if let Ok(git_key) = compute_git_cache_key(project) {
        return Ok(git_key);
    }

    // Strategy 2: Hash all source files
    let hasher = blake3::Hasher::new();
    walk_source_files(project, |path, content| {
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update(content);
    })?;

    Ok(format!("src-{}", hasher.finalize().to_hex()[..16]))
}

fn compute_git_cache_key(project: &Path) -> Result<String> {
    let head = std::fs::read_to_string(project.join(".git/HEAD"))?;
    let head_ref = head.trim().strip_prefix("ref: ").unwrap_or(head.trim());

    let commit = if head_ref.starts_with("refs/") {
        std::fs::read_to_string(project.join(".git").join(head_ref))?
    } else {
        head.clone()
    };

    // Include index hash for uncommitted changes
    let index_hash = compute_git_index_hash(project)?;

    Ok(format!("git-{}-{}", &commit.trim()[..8], &index_hash[..8]))
}
```

### 7.4 Large File Handling

For very large projects, use parallel transfer:

```rust
/// Splits project into chunks and transfers in parallel
pub async fn parallel_transfer(
    project: &Path,
    worker: &Worker,
    remote_path: &str,
    parallelism: usize,
) -> Result<TransferStats> {
    // 1. Create manifest of all files with sizes
    let manifest = create_file_manifest(project)?;

    // 2. Partition into roughly equal chunks
    let chunks = partition_by_size(manifest, parallelism);

    // 3. Transfer chunks in parallel
    let handles: Vec<_> = chunks
        .into_iter()
        .enumerate()
        .map(|(i, chunk)| {
            let worker = worker.clone();
            let remote = format!("{}.chunk{}", remote_path, i);
            tokio::spawn(async move {
                transfer_chunk(&chunk, &worker, &remote).await
            })
        })
        .collect();

    // 4. Wait for all transfers
    let results = futures::future::try_join_all(handles).await?;

    // 5. Merge chunks on remote
    merge_remote_chunks(worker, remote_path, parallelism).await?;

    Ok(TransferStats::aggregate(&results))
}
```

---

## 8. Remote Execution Engine

### 8.1 Execution Environment Setup

Ensure remote environment matches local expectations:

```rust
pub struct ExecutionEnvironment {
    pub working_dir: PathBuf,
    pub env_vars: HashMap<String, String>,
    pub toolchain: Toolchain,
    pub resource_limits: ResourceLimits,
}

impl ExecutionEnvironment {
    pub fn for_cargo(ctx: &CompilationContext) -> Self {
        let mut env = HashMap::new();

        // Rust-specific
        env.insert("CARGO_HOME".into(), "/home/ubuntu/.cargo".into());
        env.insert("RUSTUP_HOME".into(), "/home/ubuntu/.rustup".into());
        env.insert("CARGO_TARGET_DIR".into(), "./target".into());

        // Incremental compilation (faster for remote)
        env.insert("CARGO_INCREMENTAL".into(), "1".into());

        // Parallelism matching slot reservation
        env.insert("CARGO_BUILD_JOBS".into(), ctx.estimated_cores.to_string());

        // sccache if available
        if std::env::var("RUSTC_WRAPPER").is_ok() {
            env.insert("RUSTC_WRAPPER".into(), "sccache".into());
        }

        Self {
            working_dir: ctx.project_root.clone(),
            env_vars: env,
            toolchain: Toolchain::Nightly,
            resource_limits: ResourceLimits::default(),
        }
    }
}
```

### 8.2 Command Execution

Execute compilation with proper environment:

```rust
pub async fn execute_remote_command(
    worker: &Worker,
    command: &str,
    env: &ExecutionEnvironment,
    output_handler: impl FnMut(OutputLine),
) -> Result<ExitStatus> {
    let remote_cmd = format!(
        r#"
        cd {work_dir} && \
        source ~/.cargo/env && \
        rustup default nightly && \
        {env_exports} \
        {command}
        "#,
        work_dir = env.working_dir.display(),
        env_exports = env.env_vars
            .iter()
            .map(|(k, v)| format!("export {}='{}';", k, v))
            .collect::<String>(),
        command = command,
    );

    let mut ssh = AsyncSshSession::connect(worker).await?;
    let mut channel = ssh.channel_session().await?;

    channel.exec(&remote_cmd).await?;

    // Stream output in real-time
    let stdout = channel.stream(0);
    let stderr = channel.stderr();

    let stdout_task = stream_output(stdout, OutputType::Stdout, &output_handler);
    let stderr_task = stream_output(stderr, OutputType::Stderr, &output_handler);

    tokio::try_join!(stdout_task, stderr_task)?;

    channel.wait_close().await?;

    Ok(ExitStatus::from_raw(channel.exit_status()? as i32))
}

async fn stream_output(
    reader: impl AsyncRead + Unpin,
    output_type: OutputType,
    handler: &impl Fn(OutputLine),
) -> Result<()> {
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        handler(OutputLine {
            output_type,
            content: line,
            timestamp: Instant::now(),
        });
    }

    Ok(())
}
```

### 8.3 Output Streaming

The output streaming subsystem is critical for maintaining the illusion of local compilation.
Agents must see exactly what they would see if compilation ran locally—same colors, same
timing, same progress indicators. This is one of the most delicate parts of the system.

**Design Principles:**
1. **Transparency First**: Output must be indistinguishable from local execution
2. **Color Preservation**: ANSI escape sequences must pass through unchanged
3. **Minimal Latency**: Buffer intelligently but favor responsiveness
4. **PTY Emulation**: Some tools detect TTY and behave differently—we handle both modes

```rust
use std::io::{self, Write, BufWriter};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncBufReadExt, BufReader};

/// Output streaming configuration
#[derive(Clone, Debug)]
pub struct OutputStreamConfig {
    /// Whether to preserve ANSI color codes (default: true)
    pub preserve_colors: bool,

    /// Whether the local terminal supports colors
    pub local_tty_colors: bool,

    /// Buffer size for batching output (bytes)
    pub buffer_size: usize,

    /// Maximum latency before flushing incomplete buffer (ms)
    pub flush_interval_ms: u64,

    /// Whether to prefix output with worker ID (for debugging)
    pub show_worker_prefix: bool,

    /// Filter mode for output
    pub filter: OutputFilter,
}

impl Default for OutputStreamConfig {
    fn default() -> Self {
        Self {
            preserve_colors: true,
            local_tty_colors: atty::is(atty::Stream::Stdout),
            buffer_size: 8192,
            flush_interval_ms: 50,  // 50ms max latency—imperceptible
            show_worker_prefix: false,
            filter: OutputFilter::All,
        }
    }
}

/// Intelligent output buffer with latency-aware flushing
pub struct SmartOutputBuffer {
    inner: BufWriter<io::Stdout>,
    last_flush: Instant,
    config: OutputStreamConfig,
    partial_line: String,
    in_escape_sequence: bool,
}

impl SmartOutputBuffer {
    pub fn new(config: OutputStreamConfig) -> Self {
        Self {
            inner: BufWriter::with_capacity(config.buffer_size, io::stdout()),
            last_flush: Instant::now(),
            config,
            partial_line: String::new(),
            in_escape_sequence: false,
        }
    }

    /// Write data, preserving ANSI sequences and handling partial lines
    pub fn write_preserving_ansi(&mut self, data: &[u8]) -> io::Result<()> {
        // Fast path: if we're not filtering and colors enabled, pass through
        if self.config.preserve_colors && matches!(self.config.filter, OutputFilter::All) {
            self.inner.write_all(data)?;
            self.maybe_flush()?;
            return Ok(());
        }

        // Slow path: need to process character by character
        for &byte in data {
            match byte {
                0x1B => {
                    // Start of ANSI escape sequence
                    self.in_escape_sequence = true;
                    if self.config.preserve_colors {
                        self.partial_line.push(byte as char);
                    }
                }
                b'm' if self.in_escape_sequence => {
                    // End of color sequence
                    self.in_escape_sequence = false;
                    if self.config.preserve_colors {
                        self.partial_line.push(byte as char);
                    }
                }
                b'\n' => {
                    // End of line—apply filter and flush
                    if self.should_emit_line(&self.partial_line) {
                        writeln!(self.inner, "{}", self.partial_line)?;
                    }
                    self.partial_line.clear();
                }
                _ if self.in_escape_sequence => {
                    // Inside escape sequence
                    if self.config.preserve_colors {
                        self.partial_line.push(byte as char);
                    }
                }
                _ => {
                    // Regular character
                    self.partial_line.push(byte as char);
                }
            }
        }

        self.maybe_flush()
    }

    /// Flush if buffer is full or latency threshold exceeded
    fn maybe_flush(&mut self) -> io::Result<()> {
        let elapsed = self.last_flush.elapsed();

        if elapsed >= Duration::from_millis(self.config.flush_interval_ms) {
            self.inner.flush()?;
            self.last_flush = Instant::now();
        }

        Ok(())
    }

    fn should_emit_line(&self, line: &str) -> bool {
        match &self.config.filter {
            OutputFilter::All => true,
            OutputFilter::ErrorsOnly => {
                // Match cargo error/warning patterns (strip ANSI for matching)
                let stripped = strip_ansi(line);
                stripped.contains("error[E") ||
                stripped.contains("error:") ||
                stripped.contains("warning:") ||
                stripped.starts_with("  --> ")  // Source location
            }
            OutputFilter::Summary => {
                let stripped = strip_ansi(line);
                stripped.starts_with("   Compiling") ||
                stripped.starts_with("    Finished") ||
                stripped.starts_with("     Running") ||
                stripped.starts_with("   Doc-tests") ||
                stripped.contains("test result:")
            }
            OutputFilter::Progress => {
                let stripped = strip_ansi(line);
                stripped.starts_with("   Compiling") ||
                stripped.starts_with("   Downloading") ||
                stripped.starts_with("    Building") ||
                stripped.contains("% ")  // Progress percentage
            }
            OutputFilter::Custom(f) => f(&OutputLine {
                output_type: OutputType::Stdout,
                content: line.to_string(),
                timestamp: Instant::now(),
            }),
        }
    }

    /// Force flush any remaining content
    pub fn finish(&mut self) -> io::Result<()> {
        // Emit any partial line
        if !self.partial_line.is_empty() {
            if self.should_emit_line(&self.partial_line) {
                writeln!(self.inner, "{}", self.partial_line)?;
            }
            self.partial_line.clear();
        }
        self.inner.flush()
    }
}

/// Strip ANSI escape sequences for pattern matching
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_escape = false;

    for c in s.chars() {
        match c {
            '\x1b' => in_escape = true,
            'm' if in_escape => in_escape = false,
            _ if !in_escape => result.push(c),
            _ => {}
        }
    }

    result
}

pub struct OutputForwarder {
    prefix: String,
    config: OutputStreamConfig,
    stdout_buffer: SmartOutputBuffer,
    stderr_buffer: SmartOutputBuffer,
}

impl OutputForwarder {
    pub fn new(worker_id: &str, config: OutputStreamConfig) -> Self {
        Self {
            prefix: worker_id.to_string(),
            stdout_buffer: SmartOutputBuffer::new(config.clone()),
            stderr_buffer: SmartOutputBuffer::new(config.clone()),
            config,
        }
    }

    pub fn handle(&mut self, line: OutputLine) {
        // Apply optional prefix for debugging/multi-worker visibility
        let data = if self.config.show_worker_prefix {
            let prefix = if self.config.local_tty_colors {
                format!("\x1b[36m[{}]\x1b[0m ", self.prefix)
            } else {
                format!("[{}] ", self.prefix)
            };
            format!("{}{}\n", prefix, line.content)
        } else {
            format!("{}\n", line.content)
        };

        let _ = match line.output_type {
            OutputType::Stdout => self.stdout_buffer.write_preserving_ansi(data.as_bytes()),
            OutputType::Stderr => self.stderr_buffer.write_preserving_ansi(data.as_bytes()),
        };
    }

    pub fn finish(&mut self) {
        let _ = self.stdout_buffer.finish();
        let _ = self.stderr_buffer.finish();
    }
}

pub enum OutputFilter {
    All,                    // Show everything (default)
    ErrorsOnly,             // Only errors and warnings
    Summary,                // Only compilation summary lines
    Progress,               // Progress indicators only
    Custom(Box<dyn Fn(&OutputLine) -> bool + Send + Sync>),
}

// For cloning configs
impl Clone for OutputFilter {
    fn clone(&self) -> Self {
        match self {
            Self::All => Self::All,
            Self::ErrorsOnly => Self::ErrorsOnly,
            Self::Summary => Self::Summary,
            Self::Progress => Self::Progress,
            Self::Custom(_) => Self::All,  // Can't clone closures
        }
    }
}
```

#### 8.3.1 PTY Emulation for Color-Sensitive Tools

Some tools (like cargo, rustc) detect whether stdout is a TTY and disable colors if not.
When running over SSH, we lose the TTY. We solve this by forcing pseudo-terminal allocation:

```rust
/// Execute with PTY allocation for proper color output
pub async fn execute_with_pty(
    worker: &Worker,
    command: &str,
    env: &ExecutionEnvironment,
) -> Result<(ExitStatus, String, String)> {
    let remote_cmd = format!(
        r#"cd {work_dir} && \
           source ~/.cargo/env && \
           {env_exports} \
           script -q -c '{command}' /dev/null"#,  // script forces PTY
        work_dir = env.working_dir.display(),
        env_exports = env.env_vars.iter()
            .map(|(k, v)| format!("export {}='{}';", k, v))
            .collect::<String>(),
        command = shell_escape(command),
    );

    // SSH with -tt for forced PTY allocation
    let mut cmd = Command::new("ssh");
    cmd.arg("-tt")  // Force PTY
       .args(ssh_args(worker))
       .arg(format!("{}@{}", worker.user, worker.host))
       .arg(&remote_cmd);

    // ... execution with streaming
}

/// Alternative: Set TERM and force colors via env vars
pub fn force_color_env(env: &mut HashMap<String, String>) {
    env.insert("TERM".into(), "xterm-256color".into());
    env.insert("CARGO_TERM_COLOR".into(), "always".into());
    env.insert("CLICOLOR_FORCE".into(), "1".into());
    env.insert("FORCE_COLOR".into(), "1".into());
    // GCC/Clang color forcing
    env.insert("GCC_COLORS".into(), "error=01;31:warning=01;35:note=01;36".into());
}
```

#### 8.3.2 Real-Time Progress Indicators

Handle cargo's progress bar, which uses carriage returns for in-place updates:

```rust
/// Detect and handle progress bar output
pub fn is_progress_line(line: &str) -> bool {
    // Cargo progress: "    Building [=====>        ] 45/123: crate_name"
    line.contains('[') && line.contains(']') && line.contains('/') ||
    // Percentage: "  45% complete"
    line.contains("% ") ||
    // Spinner: "⠋ Waiting"
    line.chars().next().map(|c| "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏".contains(c)).unwrap_or(false)
}

/// Write progress with carriage return handling
pub fn write_progress(buffer: &mut impl Write, line: &str) -> io::Result<()> {
    // Use \r to overwrite previous progress line
    if is_progress_line(line) {
        write!(buffer, "\r{}", line)?;
        buffer.flush()?;
    } else {
        writeln!(buffer, "{}", line)?;
    }
    Ok(())
}
```

### 8.4 Pre-flight Syntax Validation

Before investing time in code transfer and remote execution, validate that the compilation
has a reasonable chance of success. This "fail-fast" approach saves significant time when
commands have obvious errors.

**Validation Strategy:**
1. **Cargo.toml Parsing**: Verify manifest is valid TOML with required fields
2. **Rust Syntax Check**: Quick `cargo check --message-format=json` locally (optional)
3. **Dependency Availability**: Check that required targets/features exist
4. **Path Validation**: Ensure referenced files actually exist

```rust
use std::path::Path;
use serde::Deserialize;

/// Pre-flight validation result
#[derive(Debug)]
pub enum PreflightResult {
    /// Ready for remote execution
    Ready,
    /// Likely to fail—skip remote, run local for error messages
    LikelyToFail(PreflightFailure),
    /// Cannot determine—proceed with remote (fail-open)
    Unknown,
}

#[derive(Debug)]
pub struct PreflightFailure {
    pub reason: PreflightFailureReason,
    pub details: String,
    pub suggestion: Option<String>,
}

#[derive(Debug)]
pub enum PreflightFailureReason {
    /// Cargo.toml missing or invalid
    InvalidManifest,
    /// Required file not found
    MissingFile,
    /// Invalid target/feature specified
    InvalidTarget,
    /// Syntax error detected locally
    SyntaxError,
    /// Missing dependency (e.g., cargo workspace member)
    MissingDependency,
}

/// Fast pre-flight checks before remote execution
pub fn preflight_check(ctx: &CompilationContext) -> PreflightResult {
    // Check 1: Valid Cargo.toml (for Rust projects)
    if ctx.command.contains("cargo") {
        if let Err(e) = validate_cargo_manifest(&ctx.project_root) {
            return PreflightResult::LikelyToFail(PreflightFailure {
                reason: PreflightFailureReason::InvalidManifest,
                details: e.to_string(),
                suggestion: Some("Fix Cargo.toml errors before compiling".into()),
            });
        }
    }

    // Check 2: Source files exist
    if let Some(target) = extract_target_from_command(&ctx.command) {
        if !target_exists(&ctx.project_root, &target) {
            return PreflightResult::LikelyToFail(PreflightFailure {
                reason: PreflightFailureReason::MissingFile,
                details: format!("Target '{}' not found", target),
                suggestion: Some("Check that the target name matches a bin/lib/example".into()),
            });
        }
    }

    // Check 3: Workspace members exist (for workspace commands)
    if ctx.command.contains("-p ") || ctx.command.contains("--package ") {
        if let Some(pkg) = extract_package_name(&ctx.command) {
            if !workspace_member_exists(&ctx.project_root, &pkg) {
                return PreflightResult::LikelyToFail(PreflightFailure {
                    reason: PreflightFailureReason::MissingDependency,
                    details: format!("Package '{}' not in workspace", pkg),
                    suggestion: Some("Verify package name matches Cargo.toml [package].name".into()),
                });
            }
        }
    }

    // Check 4: Quick local syntax check (optional, time-bounded)
    if ctx.config.enable_preflight_syntax_check {
        match quick_syntax_check(&ctx.project_root, Duration::from_secs(5)) {
            Ok(true) => {}  // Passed
            Ok(false) => {
                return PreflightResult::LikelyToFail(PreflightFailure {
                    reason: PreflightFailureReason::SyntaxError,
                    details: "Local syntax check failed".into(),
                    suggestion: Some("Run `cargo check` locally to see errors".into()),
                });
            }
            Err(_) => {}  // Timeout or error—proceed anyway (fail-open)
        }
    }

    PreflightResult::Ready
}

/// Validate Cargo.toml structure
fn validate_cargo_manifest(project_root: &Path) -> Result<(), ManifestError> {
    let manifest_path = project_root.join("Cargo.toml");

    if !manifest_path.exists() {
        return Err(ManifestError::NotFound);
    }

    let content = std::fs::read_to_string(&manifest_path)?;

    // Parse as TOML
    let manifest: toml::Value = toml::from_str(&content)
        .map_err(|e| ManifestError::InvalidToml(e.to_string()))?;

    // Check for required sections
    let has_package = manifest.get("package").is_some();
    let has_workspace = manifest.get("workspace").is_some();

    if !has_package && !has_workspace {
        return Err(ManifestError::MissingSection(
            "Neither [package] nor [workspace] found".into()
        ));
    }

    // For packages, verify required fields
    if let Some(pkg) = manifest.get("package") {
        if pkg.get("name").is_none() {
            return Err(ManifestError::MissingField("package.name".into()));
        }
        if pkg.get("edition").is_none() {
            // Warning but not error—defaults to 2015
        }
    }

    Ok(())
}

/// Quick local syntax check with timeout
fn quick_syntax_check(project_root: &Path, timeout: Duration) -> Result<bool, ()> {
    use std::process::{Command, Stdio};

    let child = Command::new("cargo")
        .args(["check", "--message-format=json", "--quiet"])
        .current_dir(project_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| ())?;

    // Wait with timeout
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status.success()),
            Ok(None) => {
                if start.elapsed() > timeout {
                    // Kill and give up
                    let _ = child.kill();
                    return Err(());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => return Err(()),
        }
    }
}

/// Check if workspace member exists
fn workspace_member_exists(project_root: &Path, package_name: &str) -> bool {
    let manifest_path = project_root.join("Cargo.toml");

    if let Ok(content) = std::fs::read_to_string(&manifest_path) {
        if let Ok(manifest) = toml::from_str::<toml::Value>(&content) {
            // Check workspace members
            if let Some(workspace) = manifest.get("workspace") {
                if let Some(members) = workspace.get("members") {
                    if let Some(arr) = members.as_array() {
                        for member in arr {
                            if let Some(m) = member.as_str() {
                                // Check if package name matches member directory
                                let member_cargo = project_root.join(m).join("Cargo.toml");
                                if let Ok(member_content) = std::fs::read_to_string(&member_cargo) {
                                    if member_content.contains(&format!("name = \"{}\"", package_name)) {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    false
}

#[derive(Debug)]
pub enum ManifestError {
    NotFound,
    IoError(std::io::Error),
    InvalidToml(String),
    MissingSection(String),
    MissingField(String),
}

impl From<std::io::Error> for ManifestError {
    fn from(e: std::io::Error) -> Self {
        ManifestError::IoError(e)
    }
}
```

#### 8.4.1 Integrating Pre-flight with Execution Pipeline

```rust
pub async fn execute_compilation(
    ctx: &CompilationContext,
    daemon: &Daemon,
) -> Result<CompilationResult> {
    // Pre-flight validation
    match preflight_check(ctx) {
        PreflightResult::Ready => {
            // Proceed with remote execution
            execute_with_fallback(ctx, daemon).await
        }
        PreflightResult::LikelyToFail(failure) => {
            // Skip remote—let local execution show the error
            tracing::info!(
                "Pre-flight check failed: {:?}. Running locally for error output.",
                failure.reason
            );
            execute_local(ctx).await
        }
        PreflightResult::Unknown => {
            // Fail-open: proceed with remote
            execute_with_fallback(ctx, daemon).await
        }
    }
}
```

### 8.5 Error Handling & Fallback

Graceful degradation when remote execution fails:

```rust
pub async fn execute_with_fallback(
    ctx: &CompilationContext,
    daemon: &Daemon,
) -> Result<CompilationResult> {
    // 1. Try remote execution
    match execute_remote(ctx, daemon).await {
        Ok(result) => return Ok(result),
        Err(e) => {
            warn!("Remote compilation failed: {}. Falling back to local.", e);
        }
    }

    // 2. Fallback to local execution
    eprintln!("\x1b[33m[RCH] Remote unavailable, running locally\x1b[0m");

    execute_local(ctx).await
}

async fn execute_remote(
    ctx: &CompilationContext,
    daemon: &Daemon,
) -> Result<CompilationResult> {
    // Select worker
    let worker = daemon.select_worker(&SelectionRequest {
        project: ctx.project_root.file_name().unwrap().to_string_lossy().into(),
        estimated_cores: ctx.estimated_cores,
    }).ok_or(RemoteError::NoAvailableWorkers)?;

    // Reserve slots
    let reservation = worker.try_reserve_slots(ctx.estimated_cores)?;
    let _guard = scopeguard::guard(reservation, |r| worker.release_slots(r));

    // Transfer project
    let remote_path = format!("/tmp/rch/{}_{}",
        ctx.project_root.file_name().unwrap().to_string_lossy(),
        compute_cache_key(&ctx.project_root)?);

    transfer_project(&ctx.project_root, worker, &remote_path, &Default::default()).await?;

    // Execute command
    let env = ExecutionEnvironment::for_cargo(ctx);
    let status = execute_remote_command(
        worker,
        &ctx.command,
        &env,
        |line| print_output_line(&line),
    ).await?;

    if !status.success() {
        return Err(RemoteError::CompilationFailed {
            exit_code: status.code().unwrap_or(-1)
        });
    }

    // Return artifacts
    return_artifacts(worker, &remote_path, &ctx.target_dir).await?;

    Ok(CompilationResult::Success)
}
```

---

## 9. Artifact Return Pipeline

### 9.1 Artifact Identification

Determine which artifacts need to be returned:

```rust
pub struct ArtifactSpec {
    pub patterns: Vec<String>,
    pub compress: bool,
    pub compression_level: u8,
}

impl ArtifactSpec {
    pub fn for_cargo(command: &str) -> Self {
        let mut patterns = Vec::new();

        // Always return compilation artifacts
        if command.contains("--release") {
            patterns.push("target/release/**".into());
        } else {
            patterns.push("target/debug/**".into());
        }

        // Specific artifacts based on command
        if command.contains("cargo build") || command.contains("cargo run") {
            // Executables and libraries
            patterns.extend(vec![
                "target/*/deps/*.rlib".into(),
                "target/*/*.d".into(),
                "target/*/build/**".into(),
            ]);
        }

        if command.contains("cargo test") {
            // Test binaries and results
            patterns.push("target/*/deps/*-*".into());
        }

        if command.contains("cargo doc") {
            patterns.push("target/doc/**".into());
        }

        Self {
            patterns,
            compress: true,
            compression_level: 1,  // Fast compression for return
        }
    }
}
```

### 9.2 Efficient Artifact Transfer

Return only changed artifacts:

```rust
pub async fn return_artifacts(
    worker: &Worker,
    remote_project: &str,
    local_target: &Path,
) -> Result<TransferStats> {
    // 1. Create compressed archive on remote
    let archive_path = format!("{}/artifacts.tar.zst", remote_project);

    let tar_cmd = format!(
        r#"
        cd {project} && \
        tar --zstd -cf artifacts.tar.zst \
            --exclude='*.rmeta' \
            --exclude='*.d' \
            --exclude='incremental/*' \
            --exclude='.fingerprint/*' \
            target/
        "#,
        project = remote_project,
    );

    ssh_command(worker, &tar_cmd).await?;

    // 2. Transfer archive
    let local_archive = local_target.join("artifacts.tar.zst");

    rsync_file(
        worker,
        &archive_path,
        &local_archive,
    ).await?;

    // 3. Extract locally
    let tar = std::process::Command::new("tar")
        .args(["--zstd", "-xf", "artifacts.tar.zst"])
        .current_dir(local_target.parent().unwrap())
        .output()?;

    if !tar.status.success() {
        return Err(ArtifactError::ExtractionFailed);
    }

    // 4. Cleanup
    std::fs::remove_file(&local_archive)?;

    Ok(TransferStats { /* ... */ })
}
```

### 9.3 Incremental Artifact Sync

For faster iterations, use rsync for artifact return too:

```rust
pub async fn incremental_artifact_sync(
    worker: &Worker,
    remote_project: &str,
    local_target: &Path,
) -> Result<TransferStats> {
    let mut cmd = Command::new("rsync");

    cmd.arg("-az")
       .arg("--zc=zstd")
       .arg("--zl=1")  // Fast compression
       .arg("--delete")
       // Exclude intermediate artifacts
       .arg("--exclude=*.rmeta")
       .arg("--exclude=*.d")
       .arg("--exclude=incremental/")
       .arg("--exclude=.fingerprint/")
       .arg("-e")
       .arg(ssh_command_string(worker));

    cmd.arg(format!(
        "{}@{}:{}/target/",
        worker.user, worker.host, remote_project
    ));

    cmd.arg(format!("{}/", local_target.display()));

    let output = cmd.output().await?;

    if !output.status.success() {
        return Err(ArtifactError::SyncFailed {
            stderr: String::from_utf8_lossy(&output.stderr).into(),
        });
    }

    Ok(TransferStats::parse_rsync(&output.stdout))
}
```

---

## 10. Toolchain Synchronization

### 10.1 Rust Toolchain Management

Ensure all workers have matching Rust toolchain:

```rust
/// Synchronizes Rust toolchain to nightly on all workers
pub async fn sync_rust_toolchain(workers: &[Worker]) -> Result<()> {
    let install_script = r#"
        # Install rustup if not present
        if ! command -v rustup &> /dev/null; then
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain nightly
        fi

        source ~/.cargo/env

        # Switch to nightly
        rustup default nightly

        # Update to latest nightly
        rustup update nightly

        # Install common components
        rustup component add clippy rustfmt

        # Install sccache for caching
        if ! command -v sccache &> /dev/null; then
            cargo install sccache
        fi

        # Report version
        rustc --version
    "#;

    let handles: Vec<_> = workers
        .iter()
        .map(|w| {
            let worker = w.clone();
            let script = install_script.to_string();
            tokio::spawn(async move {
                ssh_command(&worker, &script).await
            })
        })
        .collect();

    for (worker, result) in workers.iter().zip(futures::future::join_all(handles).await) {
        match result? {
            Ok(output) => {
                info!("Worker {} rustc: {}", worker.id, output.lines().last().unwrap_or(""));
            }
            Err(e) => {
                error!("Failed to sync toolchain on {}: {}", worker.id, e);
            }
        }
    }

    Ok(())
}
```

### 10.2 C/C++ Toolchain Management

```rust
pub async fn sync_cpp_toolchain(workers: &[Worker]) -> Result<()> {
    let install_script = r#"
        # Ubuntu 25.10 should have latest GCC/Clang

        # Ensure build essentials
        sudo apt-get update
        sudo apt-get install -y build-essential clang llvm

        # Install latest GCC if not present
        gcc --version || sudo apt-get install -y gcc g++

        # Install latest Clang
        clang --version || sudo apt-get install -y clang

        # Install ccache for caching
        ccache --version || sudo apt-get install -y ccache

        # Report versions
        echo "GCC: $(gcc --version | head -1)"
        echo "Clang: $(clang --version | head -1)"
    "#;

    // Execute on all workers in parallel
    parallel_ssh_command(workers, install_script).await
}
```

### 10.3 Toolchain Version Locking

For reproducible builds, lock specific versions:

```toml
# ~/.config/rch/toolchain.toml

[rust]
channel = "nightly"
date = "2026-01-15"  # Specific nightly date
components = ["clippy", "rustfmt", "rust-src"]

[cpp]
gcc_version = "14"
clang_version = "19"

[caching]
rustc_wrapper = "sccache"
cc_wrapper = "ccache"
```

---

## 11. Configuration System

### 11.1 Configuration Hierarchy

Following DCG's layered approach:

```
Priority (highest to lowest):
1. Environment variables (RCH_*)
2. CLI arguments
3. Project config (.rch/config.toml)
4. User config (~/.config/rch/config.toml)
5. System config (/etc/rch/config.toml)
6. Compiled defaults
```

### 11.2 Configuration Schema

```toml
# ~/.config/rch/config.toml

[general]
enabled = true
verbose = false
log_level = "info"
log_file = "~/.local/share/rch/rch.log"

[daemon]
socket_path = "/tmp/rch.sock"
pid_file = "/tmp/rch.pid"
heartbeat_interval_sec = 30
benchmark_interval_hours = 6

[transfer]
compression_level = 3          # zstd level (1-19, 3 is fast)
parallel_transfers = false
max_transfer_size_mb = 500
timeout_sec = 300

[transfer.exclude]
patterns = [
    "target/",
    ".git/objects/",
    "node_modules/",
    "*.rlib",
    "*.rmeta",
]

[execution]
streaming_output = true
fallback_to_local = true
timeout_sec = 3600             # 1 hour max
resource_limits = true

[artifacts]
compression_level = 1          # Fast for return
incremental_sync = true
cleanup_remote = false         # Keep for caching

[workers]
# See workers.toml for detailed worker config
config_file = "~/.config/rch/workers.toml"
discovery_enabled = true       # Auto-discover from SSH config

[hooks]
pre_transfer = ""              # Shell command before transfer
post_compile = ""              # Shell command after compile
on_fallback = ""               # Shell command on local fallback

[metrics]
enabled = true
prometheus_port = 9090
history_retention_days = 30
```

### 11.3 Environment Variable Overrides

```bash
# Enable/disable
RCH_ENABLED=1|0

# Worker selection
RCH_WORKER=css                 # Force specific worker
RCH_WORKERS="css,csd,yto"      # Limit worker pool

# Transfer
RCH_COMPRESSION=5              # Override compression level
RCH_NO_CACHE=1                 # Disable caching

# Debugging
RCH_VERBOSE=1
RCH_DRY_RUN=1                  # Show what would happen
RCH_LOCAL_ONLY=1               # Force local execution

# Emergency bypass
RCH_BYPASS=1                   # Disable entirely
```

### 11.4 Project-Level Configuration

Projects can override global settings with a `.rch/config.toml` file in the project root.
This enables fine-grained control per-project without affecting other codebases.

**Use Cases:**
- Disable remote compilation for a specific project
- Pin a project to specific worker(s) with required toolchain
- Override transfer exclusions for unusual project structures
- Set project-specific build timeouts

```toml
# .rch/config.toml - Project-level overrides

[project]
# Unique project identifier (auto-generated if not specified)
id = "my_rust_project"

# Human-readable name for logs/metrics
name = "My Rust Project"

# Disable RCH entirely for this project
enabled = true

# Force local execution (useful for debugging)
local_only = false

[workers]
# Restrict this project to specific workers
allowed = ["css", "yto"]

# Preferred worker for this project (has affinity cache)
preferred = "css"

# Workers to never use for this project
blocked = []

# Minimum speed score required
min_speed_score = 50.0

[transfer]
# Additional exclude patterns beyond global
exclude_extra = [
    "testdata/",
    "*.pdb",
    "benches/large_fixtures/",
]

# Files that MUST be transferred (override global excludes)
force_include = [
    ".cargo/config.toml",
]

# Maximum project size to transfer (MB)
max_size_mb = 200

[execution]
# Override execution timeout for slow builds
timeout_sec = 7200  # 2 hours for large workspace

# Number of cores to request (override auto-detection)
cores = 8

# Enable pre-flight syntax check
preflight_check = true

# Custom environment variables for remote execution
[execution.env]
CARGO_BUILD_JOBS = "16"
RUSTFLAGS = "-C target-cpu=native"

[artifacts]
# Additional patterns to return beyond standard artifacts
extra_patterns = [
    "target/criterion/**",  # Benchmark results
]

# Keep remote artifacts for longer (hours)
cache_ttl_hours = 48

[hooks]
# Project-specific hooks (run locally)
pre_transfer = "./scripts/pre_compile.sh"
post_compile = "./scripts/post_compile.sh"
```

#### 11.4.1 Configuration Loading & Merging

```rust
use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ProjectConfig {
    #[serde(default)]
    pub project: ProjectMeta,
    #[serde(default)]
    pub workers: ProjectWorkerConfig,
    #[serde(default)]
    pub transfer: ProjectTransferConfig,
    #[serde(default)]
    pub execution: ProjectExecutionConfig,
    #[serde(default)]
    pub artifacts: ProjectArtifactConfig,
    #[serde(default)]
    pub hooks: ProjectHooks,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ProjectMeta {
    pub id: Option<String>,
    pub name: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub local_only: bool,
}

fn default_true() -> bool { true }

impl ProjectConfig {
    /// Load project config from .rch/config.toml if present
    pub fn load(project_root: &Path) -> Option<Self> {
        let config_path = project_root.join(".rch/config.toml");

        if !config_path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&config_path).ok()?;
        toml::from_str(&content).ok()
    }

    /// Discover project root by walking up from working directory
    pub fn find_project_root(start: &Path) -> Option<PathBuf> {
        let mut current = start.to_path_buf();

        loop {
            // Check for project markers
            if current.join("Cargo.toml").exists() ||
               current.join(".rch").exists() ||
               current.join(".git").exists() {
                return Some(current);
            }

            if !current.pop() {
                return None;
            }
        }
    }
}

/// Merge project config with global config
pub struct MergedConfig {
    pub global: GlobalConfig,
    pub project: Option<ProjectConfig>,
}

impl MergedConfig {
    pub fn load(working_dir: &Path) -> Self {
        let global = GlobalConfig::load();
        let project_root = ProjectConfig::find_project_root(working_dir);
        let project = project_root.and_then(|p| ProjectConfig::load(&p));

        Self { global, project }
    }

    /// Check if RCH is enabled (project can disable)
    pub fn is_enabled(&self) -> bool {
        if !self.global.general.enabled {
            return false;
        }
        self.project.as_ref().map(|p| p.project.enabled).unwrap_or(true)
    }

    /// Check if local-only mode is forced
    pub fn is_local_only(&self) -> bool {
        if self.global.general.local_only {
            return true;
        }
        self.project.as_ref().map(|p| p.project.local_only).unwrap_or(false)
    }

    /// Get allowed workers (intersection of global and project)
    pub fn allowed_workers(&self) -> Option<Vec<String>> {
        let project_allowed = self.project.as_ref()
            .and_then(|p| {
                if p.workers.allowed.is_empty() {
                    None
                } else {
                    Some(p.workers.allowed.clone())
                }
            });

        // Project restrictions take priority
        project_allowed.or_else(|| {
            if self.global.workers.allowed.is_empty() {
                None
            } else {
                Some(self.global.workers.allowed.clone())
            }
        })
    }

    /// Get combined exclusion patterns
    pub fn exclude_patterns(&self) -> Vec<String> {
        let mut patterns = self.global.transfer.exclude.patterns.clone();

        // Add project-specific excludes
        if let Some(project) = &self.project {
            patterns.extend(project.transfer.exclude_extra.clone());
        }

        // Remove any force-includes
        if let Some(project) = &self.project {
            patterns.retain(|p| !project.transfer.force_include.iter().any(|f| p == f));
        }

        patterns
    }

    /// Get execution timeout (project can override)
    pub fn execution_timeout(&self) -> Duration {
        let default = Duration::from_secs(self.global.execution.timeout_sec);

        self.project.as_ref()
            .and_then(|p| p.execution.timeout_sec)
            .map(Duration::from_secs)
            .unwrap_or(default)
    }

    /// Get custom environment variables (merged)
    pub fn execution_env(&self) -> HashMap<String, String> {
        let mut env = self.global.execution.env.clone();

        if let Some(project) = &self.project {
            env.extend(project.execution.env.clone());
        }

        env
    }
}
```

#### 11.4.2 Project Detection & Config Caching

```rust
use std::collections::HashMap;
use std::sync::RwLock;

/// Cache loaded project configs to avoid repeated disk access
pub struct ProjectConfigCache {
    cache: RwLock<HashMap<PathBuf, (ProjectConfig, Instant)>>,
    ttl: Duration,
}

impl ProjectConfigCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            ttl,
        }
    }

    pub fn get(&self, project_root: &Path) -> Option<ProjectConfig> {
        // Check cache first
        {
            let cache = self.cache.read().unwrap();
            if let Some((config, loaded_at)) = cache.get(project_root) {
                if loaded_at.elapsed() < self.ttl {
                    return Some(config.clone());
                }
            }
        }

        // Cache miss or stale—reload
        let config = ProjectConfig::load(project_root)?;

        // Update cache
        {
            let mut cache = self.cache.write().unwrap();
            cache.insert(project_root.to_path_buf(), (config.clone(), Instant::now()));
        }

        Some(config)
    }

    /// Invalidate cache entry (e.g., when config file changes)
    pub fn invalidate(&self, project_root: &Path) {
        let mut cache = self.cache.write().unwrap();
        cache.remove(project_root);
    }
}

/// Watch for config file changes
pub async fn watch_project_configs(
    cache: Arc<ProjectConfigCache>,
    project_root: PathBuf,
) -> Result<()> {
    use notify::{Watcher, RecursiveMode, watcher};

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = watcher(tx, Duration::from_secs(1))?;

    let config_path = project_root.join(".rch");
    if config_path.exists() {
        watcher.watch(&config_path, RecursiveMode::NonRecursive)?;
    }

    loop {
        match rx.recv_timeout(Duration::from_secs(60)) {
            Ok(event) => {
                tracing::debug!("Project config changed: {:?}", event);
                cache.invalidate(&project_root);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}
```

#### 11.4.3 CLI for Project Config Management

```bash
# Initialize .rch/config.toml with defaults
rch config init

# Show effective config (merged global + project)
rch config show

# Set project-specific value
rch config set workers.preferred css
rch config set execution.timeout_sec 7200

# Validate config file
rch config validate

# Reset project config to defaults
rch config reset
```

```rust
/// CLI subcommand for config management
pub fn handle_config_command(args: &ConfigArgs) -> Result<()> {
    match &args.action {
        ConfigAction::Init => {
            let project_root = find_project_root(&std::env::current_dir()?)?;
            let config_dir = project_root.join(".rch");

            std::fs::create_dir_all(&config_dir)?;

            let config_path = config_dir.join("config.toml");
            if config_path.exists() {
                bail!(".rch/config.toml already exists");
            }

            let default_config = ProjectConfig::default();
            let content = toml::to_string_pretty(&default_config)?;
            std::fs::write(&config_path, content)?;

            println!("Created {}", config_path.display());
            Ok(())
        }

        ConfigAction::Show => {
            let working_dir = std::env::current_dir()?;
            let merged = MergedConfig::load(&working_dir);

            println!("# Effective configuration (merged global + project)\n");
            println!("enabled: {}", merged.is_enabled());
            println!("local_only: {}", merged.is_local_only());

            if let Some(workers) = merged.allowed_workers() {
                println!("allowed_workers: {:?}", workers);
            }

            println!("timeout: {:?}", merged.execution_timeout());
            println!("exclude_patterns: {:?}", merged.exclude_patterns());

            Ok(())
        }

        ConfigAction::Set { key, value } => {
            let project_root = find_project_root(&std::env::current_dir()?)?;
            let config_path = project_root.join(".rch/config.toml");

            let mut config = if config_path.exists() {
                let content = std::fs::read_to_string(&config_path)?;
                toml::from_str(&content)?
            } else {
                ProjectConfig::default()
            };

            // Apply the setting (simplified—real impl would parse dotted keys)
            apply_setting(&mut config, key, value)?;

            let content = toml::to_string_pretty(&config)?;
            std::fs::write(&config_path, content)?;

            println!("Set {} = {}", key, value);
            Ok(())
        }

        ConfigAction::Validate => {
            let project_root = find_project_root(&std::env::current_dir()?)?;
            let config_path = project_root.join(".rch/config.toml");

            if !config_path.exists() {
                println!("No .rch/config.toml found (using defaults)");
                return Ok(());
            }

            let content = std::fs::read_to_string(&config_path)?;
            match toml::from_str::<ProjectConfig>(&content) {
                Ok(_) => println!("✓ Configuration is valid"),
                Err(e) => {
                    eprintln!("✗ Configuration error: {}", e);
                    std::process::exit(1);
                }
            }

            Ok(())
        }
    }
}
```

---

## 12. Installation & Deployment

### 12.1 Installation Script Architecture

Single-command installation that sets up everything:

```bash
#!/usr/bin/env bash
# install.sh - Remote Compilation Helper Installer

set -euo pipefail

# ============================================================================
# Configuration
# ============================================================================
REPO="Dicklesworthstone/remote_compilation_helper"
BINARY_NAME="rch"
DAEMON_NAME="rchd"
WORKER_NAME="rch-wkr"

# ============================================================================
# Installation Modes
# ============================================================================
#
# 1. Local-only:   Install rch + rchd on current machine
# 2. Fleet:        Install on current + all workers
# 3. Worker-only:  Install just rch-wkr on current machine
#
# Usage:
#   bash install.sh                      # Interactive
#   bash install.sh --fleet              # Full fleet setup
#   bash install.sh --worker-only        # Just worker
#   bash install.sh --easy-mode          # Non-interactive full setup
# ============================================================================

main() {
    parse_args "$@"

    print_banner
    detect_platform

    case "$MODE" in
        local)
            install_local
            configure_hook
            ;;
        fleet)
            install_local
            configure_hook
            discover_workers
            deploy_to_workers
            sync_toolchains
            run_benchmarks
            ;;
        worker)
            install_worker
            ;;
    esac

    print_summary
}

install_local() {
    info "Installing RCH locally..."

    # 1. Download/build binary
    if [[ "$FROM_SOURCE" == "true" ]]; then
        build_from_source
    else
        download_binary "$BINARY_NAME" "$DEST"
        download_binary "$DAEMON_NAME" "$DEST"
    fi

    # 2. Create config directories
    mkdir -p ~/.config/rch
    mkdir -p ~/.local/share/rch

    # 3. Generate default configs if not exist
    if [[ ! -f ~/.config/rch/config.toml ]]; then
        generate_default_config
    fi

    # 4. Setup daemon service
    setup_daemon_service
}

deploy_to_workers() {
    info "Deploying to remote workers..."

    for worker in "${WORKERS[@]}"; do
        info "  → $worker"

        # 1. Copy binary
        scp "$DEST/$WORKER_NAME" "$worker:/tmp/"

        # 2. Install via SSH
        ssh "$worker" "
            sudo mv /tmp/$WORKER_NAME /usr/local/bin/
            sudo chmod +x /usr/local/bin/$WORKER_NAME

            # Create cache directories
            mkdir -p /tmp/rch

            # Setup systemd service (optional)
            if [[ '$DAEMON_MODE' == 'true' ]]; then
                sudo tee /etc/systemd/system/rch-worker.service > /dev/null <<EOF
[Unit]
Description=RCH Worker Agent
After=network.target

[Service]
ExecStart=/usr/local/bin/$WORKER_NAME daemon
Restart=always
User=$USER

[Install]
WantedBy=multi-user.target
EOF
                sudo systemctl daemon-reload
                sudo systemctl enable rch-worker
                sudo systemctl start rch-worker
            fi
        "
    done
}

configure_hook() {
    info "Configuring Claude Code hook..."

    local settings_file="$HOME/.claude/settings.json"

    # Backup existing
    if [[ -f "$settings_file" ]]; then
        cp "$settings_file" "$settings_file.bak.$(date +%s)"
    fi

    # Merge RCH hook (using Python helper like DCG)
    python3 << 'PYTHON'
import json
import os

settings_path = os.path.expanduser("~/.claude/settings.json")
rch_path = os.path.expanduser("~/.local/bin/rch")

# Load or create settings
if os.path.exists(settings_path):
    with open(settings_path) as f:
        settings = json.load(f)
else:
    settings = {}

# Ensure hooks structure
if "hooks" not in settings:
    settings["hooks"] = {}
if "PreToolUse" not in settings["hooks"]:
    settings["hooks"]["PreToolUse"] = []

# Find or create Bash matcher
bash_matcher = None
for matcher in settings["hooks"]["PreToolUse"]:
    if matcher.get("matcher") == "Bash":
        bash_matcher = matcher
        break

if bash_matcher is None:
    bash_matcher = {"matcher": "Bash", "hooks": []}
    settings["hooks"]["PreToolUse"].append(bash_matcher)

# Check if RCH already configured
rch_configured = any(
    h.get("command", "").endswith("rch")
    for h in bash_matcher.get("hooks", [])
)

if not rch_configured:
    # Add RCH hook BEFORE dcg (if present) so it intercepts first
    bash_matcher["hooks"].insert(0, {
        "type": "command",
        "command": rch_path
    })

# Write updated settings
with open(settings_path, "w") as f:
    json.dump(settings, f, indent=2)

print("RCH hook configured successfully")
PYTHON
}
```

### 12.2 Worker Discovery

Automatically find workers from SSH config and zshrc:

```rust
pub fn discover_workers() -> Vec<WorkerConfig> {
    let mut workers = Vec::new();

    // 1. Parse ~/.ssh/config
    let ssh_config = home_dir().join(".ssh/config");
    if ssh_config.exists() {
        workers.extend(parse_ssh_config(&ssh_config));
    }

    // 2. Parse zshrc aliases
    let zshrc = home_dir().join(".zshrc.local");
    if zshrc.exists() {
        workers.extend(parse_zshrc_ssh_aliases(&zshrc));
    }

    // 3. Filter to likely compilation workers
    // (exclude Macs, Windows, non-Linux hosts)
    workers.retain(|w| is_linux_worker(w));

    // 4. Probe each for capabilities
    for worker in &mut workers {
        if let Ok(caps) = probe_worker_capabilities(worker) {
            worker.total_slots = caps.cpu_cores;
            worker.has_rust = caps.has_rustc;
            worker.has_cpp = caps.has_gcc || caps.has_clang;
        }
    }

    workers
}

fn parse_zshrc_ssh_aliases(path: &Path) -> Vec<WorkerConfig> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let mut workers = Vec::new();

    // Regex: alias name='ssh -i key user@host'
    let re = Regex::new(r#"alias\s+(\w+)='ssh\s+(?:-i\s+(\S+)\s+)?(\w+)@(\S+)'"#).unwrap();

    for cap in re.captures_iter(&content) {
        workers.push(WorkerConfig {
            id: cap[1].to_string(),
            user: cap[3].to_string(),
            host: cap[4].to_string(),
            identity_file: cap.get(2).map(|m| PathBuf::from(m.as_str())),
            ..Default::default()
        });
    }

    workers
}
```

### 12.3 Idempotent Remote Setup

```bash
# setup_worker.sh - Idempotent worker setup script

#!/usr/bin/env bash
set -euo pipefail

MARKER_FILE="/var/lib/rch/.setup_complete"
VERSION_FILE="/var/lib/rch/.version"
REQUIRED_VERSION="0.1.0"

# Skip if already set up with correct version
if [[ -f "$MARKER_FILE" ]] && [[ -f "$VERSION_FILE" ]]; then
    installed_version=$(cat "$VERSION_FILE")
    if [[ "$installed_version" == "$REQUIRED_VERSION" ]]; then
        echo "RCH worker already at version $REQUIRED_VERSION"
        exit 0
    fi
fi

echo "Setting up RCH worker..."

# 1. Create directories
sudo mkdir -p /var/lib/rch
sudo mkdir -p /tmp/rch
sudo chown -R "$USER:$USER" /tmp/rch

# 2. Install dependencies
sudo apt-get update
sudo apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    zstd \
    rsync

# 3. Install Rust toolchain
if ! command -v rustup &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain nightly
fi
source ~/.cargo/env
rustup default nightly
rustup update

# 4. Install sccache
cargo install sccache || true
echo 'export RUSTC_WRAPPER=sccache' >> ~/.bashrc

# 5. Install C/C++ tools
sudo apt-get install -y gcc g++ clang ccache

# 6. Mark complete
echo "$REQUIRED_VERSION" | sudo tee "$VERSION_FILE"
sudo touch "$MARKER_FILE"

echo "RCH worker setup complete!"
```

---

## 13. Performance Optimization

### 13.1 SSH Connection Multiplexing

Reuse SSH connections for massive speedup:

```rust
pub struct SshConnectionPool {
    connections: HashMap<WorkerId, Arc<AsyncSshSession>>,
    control_path: PathBuf,
}

impl SshConnectionPool {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
            control_path: PathBuf::from("/tmp/rch-ssh"),
        }
    }

    pub async fn get_or_create(&mut self, worker: &Worker) -> Arc<AsyncSshSession> {
        if let Some(conn) = self.connections.get(&worker.id) {
            if conn.is_alive().await {
                return conn.clone();
            }
        }

        let conn = Arc::new(self.create_connection(worker).await);
        self.connections.insert(worker.id.clone(), conn.clone());
        conn
    }

    async fn create_connection(&self, worker: &Worker) -> AsyncSshSession {
        let control_socket = self.control_path.join(format!(
            "{}@{}:22", worker.user, worker.host
        ));

        AsyncSshSession::connect_with_control_master(
            &worker.host,
            &worker.user,
            worker.identity_file.as_ref(),
            &control_socket,
        ).await.expect("SSH connection failed")
    }
}

// SSH config for ControlMaster
pub fn ssh_args(worker: &Worker, control_path: &Path) -> Vec<String> {
    vec![
        "-o".into(), "ControlMaster=auto".into(),
        "-o".into(), format!("ControlPath={}", control_path.display()),
        "-o".into(), "ControlPersist=600".into(),
        "-o".into(), "StrictHostKeyChecking=accept-new".into(),
        "-o".into(), "Compression=yes".into(),
        "-i".into(), worker.identity_file.display().to_string(),
    ]
}
```

### 13.2 Parallel Transfer Pipelines

Overlap transfer and compilation:

```rust
/// Pipelining: start compilation while still transferring
pub async fn pipelined_remote_execution(
    ctx: &CompilationContext,
    worker: &Worker,
) -> Result<CompilationResult> {
    let remote_path = compute_remote_path(ctx);

    // Start transfer
    let transfer_handle = tokio::spawn({
        let worker = worker.clone();
        let project = ctx.project_root.clone();
        let remote = remote_path.clone();
        async move {
            transfer_project(&project, &worker, &remote, &Default::default()).await
        }
    });

    // Wait for essential files to transfer, then start compilation
    // (Cargo.toml, src/lib.rs must be present)
    wait_for_essential_files(&worker, &remote_path, &ctx.essential_files()).await?;

    // Start compilation while transfer completes
    let compile_handle = tokio::spawn({
        let worker = worker.clone();
        let command = ctx.command.clone();
        let remote = remote_path.clone();
        async move {
            // Small delay to let more files land
            tokio::time::sleep(Duration::from_millis(500)).await;

            let env = ExecutionEnvironment::for_cargo(&ctx);
            execute_remote_command(&worker, &command, &env, |_| {}).await
        }
    });

    // Wait for both
    let (transfer_result, compile_result) = tokio::join!(transfer_handle, compile_handle);

    transfer_result??;
    let status = compile_result??;

    if !status.success() {
        return Err(CompilationError::Failed { exit_code: status.code() });
    }

    // Return artifacts
    return_artifacts(&worker, &remote_path, &ctx.target_dir).await?;

    Ok(CompilationResult::Success)
}
```

### 13.3 Compression Tuning

Dynamic compression level based on network:

```rust
pub fn optimal_compression_level(
    file_size: u64,
    bandwidth_mbps: f64,
    cpu_speed_score: f64,
) -> u8 {
    // Lower bandwidth = higher compression worth it
    // Higher CPU = higher compression affordable

    let bandwidth_factor = 1.0 / (bandwidth_mbps / 100.0);
    let cpu_factor = cpu_speed_score / 100.0;

    let optimal = (bandwidth_factor * cpu_factor * 10.0) as u8;

    // Clamp to reasonable range
    optimal.clamp(1, 9)
}

// For large files, consider parallel zstd
pub async fn compress_parallel(
    source: &Path,
    dest: &Path,
    level: u8,
    threads: usize,
) -> Result<()> {
    let status = Command::new("zstd")
        .args(["-T", &threads.to_string()])
        .args(["-", &level.to_string()])
        .arg(source)
        .arg("-o")
        .arg(dest)
        .status()
        .await?;

    if !status.success() {
        return Err(CompressionError::Failed);
    }

    Ok(())
}
```

### 13.4 Caching Strategies

Multi-level caching for maximum reuse:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         CACHING ARCHITECTURE                             │
└─────────────────────────────────────────────────────────────────────────┘

Level 1: Local Daemon Cache
├── Worker selection decisions (TTL: 5s)
├── Project hash → remote path mapping (TTL: 1h)
└── Worker health status (TTL: 30s)

Level 2: Remote Project Cache
├── /tmp/rch/{project}_{hash}/
├── Full project snapshot
├── LRU eviction (keep last N projects)
└── Shared across compilations

Level 3: Compilation Cache (sccache/ccache)
├── SCCACHE_DIR=/var/cache/sccache
├── Shared across all projects
├── Cross-worker via S3/Redis (optional)
└── 50-80% hit rate on incremental builds

Level 4: Artifact Cache
├── /tmp/rch/{project}_{hash}/target/
├── Incremental compilation state
└── Dramatic speedup for edit→compile→run cycles
```

---

## 14. Failure Modes & Resilience

### 14.1 Failure Taxonomy

| Failure | Detection | Response |
|---------|-----------|----------|
| Worker unreachable | SSH timeout | Mark degraded, try next worker |
| Worker overloaded | Slot exhaustion | Queue or fallback to local |
| Transfer failed | rsync exit code | Retry with exponential backoff |
| Compilation failed | Exit code != 0 | Report to agent, no fallback |
| Artifact transfer failed | rsync exit code | Retry, keep remote artifacts |
| Daemon unavailable | Socket timeout | Direct SSH, no slot tracking |
| Network partition | Multiple failures | Circuit breaker, local fallback |

### 14.2 Circuit Breaker Pattern

```rust
pub struct CircuitBreaker {
    state: AtomicU8,  // 0=Closed, 1=Open, 2=HalfOpen
    failure_count: AtomicU32,
    last_failure: AtomicU64,

    // Config
    failure_threshold: u32,
    recovery_timeout: Duration,
}

impl CircuitBreaker {
    pub fn allow_request(&self) -> bool {
        match self.state.load(Ordering::Acquire) {
            0 => true,  // Closed: allow
            1 => {      // Open: check if recovery time passed
                let now = unix_timestamp();
                let last = self.last_failure.load(Ordering::Relaxed);
                if now - last > self.recovery_timeout.as_secs() {
                    self.state.store(2, Ordering::Release);  // HalfOpen
                    true
                } else {
                    false
                }
            }
            2 => true,  // HalfOpen: allow one probe
            _ => false,
        }
    }

    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
        self.state.store(0, Ordering::Release);  // Close
    }

    pub fn record_failure(&self) {
        let count = self.failure_count.fetch_add(1, Ordering::AcqRel);
        self.last_failure.store(unix_timestamp(), Ordering::Relaxed);

        if count + 1 >= self.failure_threshold {
            self.state.store(1, Ordering::Release);  // Open
        }
    }
}
```

### 14.3 Graceful Degradation

```rust
pub async fn execute_with_graceful_degradation(
    ctx: &CompilationContext,
    daemon: &Daemon,
) -> Result<CompilationResult> {
    // Tier 1: Optimal remote execution
    if let Some(worker) = daemon.select_optimal_worker(ctx).await {
        match execute_on_worker(ctx, &worker).await {
            Ok(result) => return Ok(result),
            Err(e) => warn!("Tier 1 failed: {}", e),
        }
    }

    // Tier 2: Any available worker
    for worker in daemon.all_healthy_workers() {
        if worker.has_any_slots() {
            match execute_on_worker(ctx, &worker).await {
                Ok(result) => return Ok(result),
                Err(e) => warn!("Worker {} failed: {}", worker.id, e),
            }
        }
    }

    // Tier 3: Queue and wait
    if daemon.config.enable_queueing {
        info!("All workers busy, queueing...");
        let worker = daemon.wait_for_worker(ctx.estimated_cores, Duration::from_secs(60)).await?;
        match execute_on_worker(ctx, &worker).await {
            Ok(result) => return Ok(result),
            Err(e) => warn!("Queued execution failed: {}", e),
        }
    }

    // Tier 4: Local fallback (with warning)
    eprintln!("\x1b[33m[RCH] All remote workers unavailable, falling back to local\x1b[0m");
    execute_local(ctx).await
}
```

### 14.4 State Recovery

Recover from crashes cleanly:

```rust
impl Daemon {
    pub async fn recover_state(&mut self) -> Result<()> {
        // 1. Check for orphaned slot reservations
        for worker in self.workers.values_mut() {
            // Query worker for actual running compilations
            let active = query_active_compilations(worker).await?;

            // Reset slots to match reality
            worker.used_slots.store(active.len() as u32, Ordering::Release);
        }

        // 2. Clean up stale remote project directories
        for worker in self.workers.values() {
            cleanup_stale_projects(worker, Duration::from_hours(24)).await?;
        }

        // 3. Verify SSH connections
        self.ssh_pool.verify_all().await;

        Ok(())
    }
}

async fn query_active_compilations(worker: &Worker) -> Result<Vec<ProcessInfo>> {
    let output = ssh_command(worker, "pgrep -af 'cargo|rustc|gcc|clang' || true").await?;

    output
        .lines()
        .filter_map(|line| ProcessInfo::parse(line))
        .collect()
}
```

### 14.5 Persistent State & Session Continuity

For long-running compilations, daemon restarts must not lose track of in-flight jobs.
The daemon persists job state to disk, enabling recovery after crashes or restarts.

**Persistence Goals:**
1. **In-flight job tracking**: Know what compilations are running on which workers
2. **Slot reservation consistency**: Don't over-commit workers after restart
3. **Result retrieval**: Agents can reconnect and get compilation results
4. **Deduplication continuity**: Share results even across daemon restarts

```rust
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};

/// Persistent state stored on disk
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PersistentState {
    /// Version for migration compatibility
    pub version: u32,

    /// In-flight compilations
    pub in_flight_jobs: Vec<PersistedJob>,

    /// Worker slot reservations (worker_id -> reserved_slots)
    pub slot_reservations: HashMap<String, u32>,

    /// Recent compilation results (for deduplication)
    pub result_cache: Vec<CachedResult>,

    /// Affinity data (project_hash -> worker preferences)
    pub affinity_cache: HashMap<String, WorkerAffinityData>,

    /// Last checkpoint timestamp
    pub last_checkpoint: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedJob {
    /// Unique job ID
    pub id: String,

    /// Worker executing this job
    pub worker_id: String,

    /// Remote path where project is stored
    pub remote_path: String,

    /// Project identifier (for deduplication)
    pub project_hash: String,

    /// Command being executed
    pub command: String,

    /// Started timestamp (Unix epoch ms)
    pub started_at: u64,

    /// Expected timeout (ms from start)
    pub timeout_ms: u64,

    /// State at last checkpoint
    pub state: JobState,

    /// Requesting agent (for result delivery)
    pub requester: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobState {
    Transferring,
    Compiling,
    ReturningArtifacts,
    Completed,
    Failed,
    Unknown,  // For recovery
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResult {
    pub project_hash: String,
    pub command_hash: String,
    pub exit_code: i32,
    pub completed_at: u64,
    pub ttl_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerAffinityData {
    pub worker_scores: HashMap<String, f64>,
    pub last_used_worker: Option<String>,
    pub last_updated: u64,
}

const STATE_FILE: &str = "/var/lib/rch/daemon_state.json";
const STATE_VERSION: u32 = 1;

impl PersistentState {
    /// Load state from disk, or create fresh if not exists/corrupted
    pub fn load() -> Self {
        let path = PathBuf::from(STATE_FILE);

        if !path.exists() {
            return Self::default();
        }

        match File::open(&path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                match serde_json::from_reader(reader) {
                    Ok(state) => {
                        let state: PersistentState = state;
                        // Version migration if needed
                        if state.version != STATE_VERSION {
                            tracing::warn!(
                                "State version mismatch ({} vs {}), migrating",
                                state.version, STATE_VERSION
                            );
                            Self::migrate(state)
                        } else {
                            state
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse state file: {}", e);
                        Self::backup_corrupted(&path);
                        Self::default()
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to open state file: {}", e);
                Self::default()
            }
        }
    }

    /// Save state to disk atomically
    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = PathBuf::from(STATE_FILE);
        let temp_path = path.with_extension("json.tmp");

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write to temp file
        let file = File::create(&temp_path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, self)?;

        // Atomic rename
        fs::rename(&temp_path, &path)?;

        Ok(())
    }

    /// Checkpoint: save current state periodically
    pub fn checkpoint(&mut self) {
        self.last_checkpoint = unix_timestamp_ms();

        if let Err(e) = self.save() {
            tracing::error!("Failed to checkpoint state: {}", e);
        }
    }

    fn migrate(old: Self) -> Self {
        // Add migration logic as versions evolve
        Self {
            version: STATE_VERSION,
            ..old
        }
    }

    fn backup_corrupted(path: &Path) {
        let backup_path = path.with_extension(format!(
            "corrupted.{}.json",
            unix_timestamp_ms()
        ));
        let _ = fs::rename(path, backup_path);
    }
}

/// State manager with periodic checkpointing
pub struct StateManager {
    state: RwLock<PersistentState>,
    checkpoint_interval: Duration,
    last_checkpoint: Instant,
}

impl StateManager {
    pub fn new(checkpoint_interval: Duration) -> Self {
        let state = PersistentState::load();

        Self {
            state: RwLock::new(state),
            checkpoint_interval,
            last_checkpoint: Instant::now(),
        }
    }

    /// Record a new in-flight job
    pub fn record_job_start(&self, job: PersistedJob) {
        let mut state = self.state.write().unwrap();

        // Update slot reservations
        *state.slot_reservations
            .entry(job.worker_id.clone())
            .or_insert(0) += 1;

        state.in_flight_jobs.push(job);
        self.maybe_checkpoint(&mut state);
    }

    /// Update job state
    pub fn update_job_state(&self, job_id: &str, new_state: JobState) {
        let mut state = self.state.write().unwrap();

        if let Some(job) = state.in_flight_jobs.iter_mut().find(|j| j.id == job_id) {
            job.state = new_state;
        }

        self.maybe_checkpoint(&mut state);
    }

    /// Record job completion
    pub fn record_job_completion(&self, job_id: &str, exit_code: i32) {
        let mut state = self.state.write().unwrap();

        // Find and remove the job
        if let Some(pos) = state.in_flight_jobs.iter().position(|j| j.id == job_id) {
            let job = state.in_flight_jobs.remove(pos);

            // Update slot reservations
            if let Some(slots) = state.slot_reservations.get_mut(&job.worker_id) {
                *slots = slots.saturating_sub(1);
            }

            // Cache the result for deduplication
            let result = CachedResult {
                project_hash: job.project_hash,
                command_hash: blake3_hash(&job.command),
                exit_code,
                completed_at: unix_timestamp_ms(),
                ttl_secs: 3600,  // 1 hour cache
            };
            state.result_cache.push(result);

            // Prune old results
            let cutoff = unix_timestamp_ms() - (3600 * 1000);
            state.result_cache.retain(|r| r.completed_at > cutoff);
        }

        self.maybe_checkpoint(&mut state);
    }

    /// Update worker affinity
    pub fn update_affinity(&self, project_hash: &str, worker_id: &str, score_delta: f64) {
        let mut state = self.state.write().unwrap();

        let affinity = state.affinity_cache
            .entry(project_hash.to_string())
            .or_insert_with(|| WorkerAffinityData {
                worker_scores: HashMap::new(),
                last_used_worker: None,
                last_updated: unix_timestamp_ms(),
            });

        *affinity.worker_scores.entry(worker_id.to_string()).or_insert(0.5) += score_delta;
        affinity.last_used_worker = Some(worker_id.to_string());
        affinity.last_updated = unix_timestamp_ms();

        // Clamp scores to [0, 1]
        for score in affinity.worker_scores.values_mut() {
            *score = score.clamp(0.0, 1.0);
        }
    }

    /// Get cached result if available (for deduplication)
    pub fn get_cached_result(&self, project_hash: &str, command: &str) -> Option<CachedResult> {
        let state = self.state.read().unwrap();
        let command_hash = blake3_hash(command);
        let now = unix_timestamp_ms();

        state.result_cache.iter()
            .find(|r| {
                r.project_hash == project_hash &&
                r.command_hash == command_hash &&
                r.completed_at + (r.ttl_secs * 1000) > now
            })
            .cloned()
    }

    fn maybe_checkpoint(&self, state: &mut PersistentState) {
        // Note: In real impl, track time properly
        state.checkpoint();
    }
}
```

#### 14.5.1 Recovery After Daemon Restart

```rust
impl Daemon {
    /// Full recovery procedure after restart
    pub async fn recover_from_persistent_state(&mut self) -> Result<()> {
        let state = self.state_manager.state.read().unwrap();

        tracing::info!(
            "Recovering state: {} in-flight jobs, {} slot reservations",
            state.in_flight_jobs.len(),
            state.slot_reservations.len()
        );

        // 1. Validate in-flight jobs against actual worker state
        let mut jobs_to_remove = Vec::new();

        for job in &state.in_flight_jobs {
            let worker = match self.workers.get(&job.worker_id) {
                Some(w) => w,
                None => {
                    tracing::warn!("Worker {} no longer exists, marking job {} orphaned",
                        job.worker_id, job.id);
                    jobs_to_remove.push(job.id.clone());
                    continue;
                }
            };

            // Check if job is actually still running
            match self.probe_job_status(worker, &job).await {
                JobProbeResult::StillRunning => {
                    tracing::info!("Job {} still running on {}", job.id, worker.id);
                    // Re-reserve slots
                    worker.reserve_slots_unchecked(1);
                }
                JobProbeResult::Completed(exit_code) => {
                    tracing::info!("Job {} completed with exit code {}", job.id, exit_code);
                    // Record completion and notify if requester known
                    drop(state);  // Release read lock
                    self.state_manager.record_job_completion(&job.id, exit_code);
                    jobs_to_remove.push(job.id.clone());
                }
                JobProbeResult::Gone => {
                    tracing::warn!("Job {} no longer exists on worker", job.id);
                    jobs_to_remove.push(job.id.clone());
                }
                JobProbeResult::ProbeError(e) => {
                    tracing::error!("Failed to probe job {}: {}", job.id, e);
                    // Mark as unknown, will be cleaned up on timeout
                }
            }
        }

        // 2. Clean up orphaned jobs
        drop(state);  // Release lock before modifications
        {
            let mut state = self.state_manager.state.write().unwrap();
            state.in_flight_jobs.retain(|j| !jobs_to_remove.contains(&j.id));

            // Recalculate slot reservations from actual jobs
            state.slot_reservations.clear();
            for job in &state.in_flight_jobs {
                *state.slot_reservations.entry(job.worker_id.clone()).or_insert(0) += 1;
            }
        }

        // 3. Restore affinity data to workers
        let state = self.state_manager.state.read().unwrap();
        for (project_hash, affinity) in &state.affinity_cache {
            self.affinity_tracker.restore(project_hash, affinity.clone());
        }

        tracing::info!("State recovery complete");
        Ok(())
    }

    async fn probe_job_status(&self, worker: &Worker, job: &PersistedJob) -> JobProbeResult {
        // Check if the process is still running
        let check_cmd = format!(
            "pgrep -f '{remote_path}' > /dev/null && echo RUNNING || echo GONE",
            remote_path = job.remote_path
        );

        match ssh_command_with_timeout(worker, &check_cmd, Duration::from_secs(10)).await {
            Ok(output) => {
                if output.trim() == "RUNNING" {
                    JobProbeResult::StillRunning
                } else {
                    // Check for exit status file
                    let status_file = format!("{}/.rch_exit_status", job.remote_path);
                    let status_cmd = format!("cat {} 2>/dev/null || echo NONE", status_file);

                    match ssh_command_with_timeout(worker, &status_cmd, Duration::from_secs(5)).await {
                        Ok(status) if status.trim() != "NONE" => {
                            let exit_code = status.trim().parse().unwrap_or(-1);
                            JobProbeResult::Completed(exit_code)
                        }
                        _ => JobProbeResult::Gone
                    }
                }
            }
            Err(e) => JobProbeResult::ProbeError(e.to_string()),
        }
    }
}

enum JobProbeResult {
    StillRunning,
    Completed(i32),
    Gone,
    ProbeError(String),
}
```

#### 14.5.2 Graceful Shutdown with State Preservation

```rust
impl Daemon {
    /// Graceful shutdown: checkpoint state and notify agents
    pub async fn graceful_shutdown(&mut self) -> Result<()> {
        tracing::info!("Starting graceful shutdown...");

        // 1. Stop accepting new jobs
        self.accepting_jobs.store(false, Ordering::Release);

        // 2. Wait for short-running jobs to complete (with timeout)
        let shutdown_timeout = Duration::from_secs(30);
        let start = Instant::now();

        while start.elapsed() < shutdown_timeout {
            let state = self.state_manager.state.read().unwrap();
            let running_count = state.in_flight_jobs.len();
            drop(state);

            if running_count == 0 {
                tracing::info!("All jobs completed");
                break;
            }

            tracing::info!(
                "Waiting for {} jobs to complete ({:?} remaining)",
                running_count,
                shutdown_timeout.saturating_sub(start.elapsed())
            );

            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        // 3. Final state checkpoint
        {
            let mut state = self.state_manager.state.write().unwrap();

            // Mark any remaining jobs as unknown (will be recovered on restart)
            for job in &mut state.in_flight_jobs {
                if job.state == JobState::Compiling {
                    job.state = JobState::Unknown;
                }
            }

            state.checkpoint();
        }

        // 4. Close SSH connections gracefully
        self.ssh_pool.close_all().await;

        tracing::info!("Graceful shutdown complete");
        Ok(())
    }
}

/// Signal handler for graceful shutdown
pub async fn setup_signal_handlers(daemon: Arc<Mutex<Daemon>>) {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM handler");
    let mut sigint = signal(SignalKind::interrupt()).expect("SIGINT handler");

    tokio::select! {
        _ = sigterm.recv() => {
            tracing::info!("Received SIGTERM");
        }
        _ = sigint.recv() => {
            tracing::info!("Received SIGINT");
        }
    }

    let mut daemon = daemon.lock().await;
    if let Err(e) = daemon.graceful_shutdown().await {
        tracing::error!("Error during shutdown: {}", e);
    }
}
```

---

## 15. Security Considerations

### 15.1 SSH Key Management

```rust
// Only use keys explicitly configured or from ssh-agent
pub fn validate_identity_file(path: &Path) -> Result<()> {
    // Must exist
    if !path.exists() {
        return Err(SecurityError::KeyNotFound(path.to_path_buf()));
    }

    // Must have restricted permissions
    let metadata = std::fs::metadata(path)?;
    let mode = metadata.permissions().mode();

    if mode & 0o077 != 0 {
        return Err(SecurityError::KeyPermissionsTooOpen {
            path: path.to_path_buf(),
            mode,
        });
    }

    Ok(())
}
```

### 15.2 Remote Command Sanitization

```rust
// Never pass user input directly to shell
pub fn sanitize_command(cmd: &str) -> Result<String> {
    // Allowlist of safe command patterns
    let safe_patterns = [
        r"^cargo\s+(build|test|run|check|clippy|doc|bench)",
        r"^rustc\s+",
        r"^gcc\s+",
        r"^clang\s+",
        r"^make\b",
    ];

    let normalized = normalize_command(cmd);

    for pattern in &safe_patterns {
        if Regex::new(pattern).unwrap().is_match(&normalized) {
            // Command matches safe pattern
            // Still escape for shell safety
            return Ok(shell_escape::escape(Cow::Borrowed(cmd)).into_owned());
        }
    }

    Err(SecurityError::UnsafeCommand(cmd.to_string()))
}
```

### 15.3 Project Isolation

```rust
// Each project gets isolated directory
pub fn compute_remote_path(ctx: &CompilationContext) -> PathBuf {
    let project_name = ctx.project_root
        .file_name()
        .unwrap()
        .to_string_lossy()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect::<String>();

    let hash = compute_cache_key(&ctx.project_root).unwrap_or_else(|_| {
        // Fallback to random
        uuid::Uuid::new_v4().to_string()[..8].to_string()
    });

    PathBuf::from(format!("/tmp/rch/{}_{}", project_name, hash))
}
```

### 15.4 Network Security

```toml
# Recommended SSH config for workers

Host rch-*
    # Disable password auth
    PasswordAuthentication no

    # Use modern key exchange
    KexAlgorithms curve25519-sha256,curve25519-sha256@libssh.org

    # Limit to specific users
    AllowUsers ubuntu

    # Disable forwarding
    AllowTcpForwarding no
    X11Forwarding no
    AllowAgentForwarding no
```

---

## 16. Observability & Metrics

### 16.1 Prometheus Metrics

```rust
use prometheus::{
    register_counter, register_gauge, register_histogram,
    Counter, Gauge, Histogram, HistogramOpts,
};

lazy_static! {
    // Compilation metrics
    static ref COMPILATIONS_TOTAL: Counter = register_counter!(
        "rch_compilations_total",
        "Total compilation requests"
    ).unwrap();

    static ref COMPILATIONS_REMOTE: Counter = register_counter!(
        "rch_compilations_remote_total",
        "Compilations executed remotely"
    ).unwrap();

    static ref COMPILATIONS_LOCAL_FALLBACK: Counter = register_counter!(
        "rch_compilations_local_fallback_total",
        "Compilations fallen back to local"
    ).unwrap();

    // Latency metrics
    static ref HOOK_LATENCY: Histogram = register_histogram!(
        "rch_hook_latency_seconds",
        "Hook processing latency",
        vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]
    ).unwrap();

    static ref TRANSFER_LATENCY: Histogram = register_histogram!(
        "rch_transfer_latency_seconds",
        "Code transfer latency",
        vec![0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]
    ).unwrap();

    static ref COMPILE_LATENCY: Histogram = register_histogram!(
        "rch_compile_latency_seconds",
        "Remote compilation latency",
        vec![1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0]
    ).unwrap();

    // Worker metrics
    static ref WORKER_SLOTS_AVAILABLE: GaugeVec = register_gauge_vec!(
        "rch_worker_slots_available",
        "Available compilation slots per worker",
        &["worker"]
    ).unwrap();

    static ref WORKER_SPEED_SCORE: GaugeVec = register_gauge_vec!(
        "rch_worker_speed_score",
        "Performance score per worker",
        &["worker"]
    ).unwrap();
}
```

### 16.2 Structured Logging

```rust
use tracing::{info, warn, error, span, Level};

pub async fn execute_remote_with_tracing(
    ctx: &CompilationContext,
    worker: &Worker,
) -> Result<CompilationResult> {
    let span = span!(
        Level::INFO,
        "remote_compilation",
        project = %ctx.project_root.display(),
        worker = %worker.id,
        command = %ctx.command,
    );
    let _guard = span.enter();

    info!(
        estimated_cores = ctx.estimated_cores,
        "Starting remote compilation"
    );

    let transfer_start = Instant::now();
    transfer_project(&ctx.project_root, worker, &remote_path, &Default::default()).await?;
    info!(
        duration_ms = transfer_start.elapsed().as_millis() as u64,
        "Transfer complete"
    );

    let compile_start = Instant::now();
    let status = execute_remote_command(worker, &ctx.command, &env, |_| {}).await?;
    info!(
        duration_ms = compile_start.elapsed().as_millis() as u64,
        exit_code = status.code().unwrap_or(-1),
        "Compilation complete"
    );

    // ...
}
```

### 16.3 Dashboard

JSON output for integration with monitoring:

```rust
#[derive(Serialize)]
pub struct DaemonStatus {
    pub uptime_seconds: u64,
    pub active_compilations: u32,
    pub total_compilations: u64,
    pub workers: Vec<WorkerStatus>,
}

#[derive(Serialize)]
pub struct WorkerStatus {
    pub id: String,
    pub host: String,
    pub status: String,
    pub total_slots: u32,
    pub used_slots: u32,
    pub speed_score: f64,
    pub last_heartbeat: String,
    pub active_jobs: Vec<JobInfo>,
}

impl Daemon {
    pub fn status(&self) -> DaemonStatus {
        DaemonStatus {
            uptime_seconds: self.start_time.elapsed().as_secs(),
            active_compilations: self.workers.values()
                .map(|w| w.used_slots.load(Ordering::Relaxed))
                .sum(),
            total_compilations: self.metrics.total_compilations.load(Ordering::Relaxed),
            workers: self.workers.values().map(|w| w.status()).collect(),
        }
    }
}
```

---

## 17. CLI Interface

### 17.1 Command Structure

```
rch - Remote Compilation Helper

USAGE:
    rch [OPTIONS] <COMMAND>

COMMANDS:
    hook        Run as PreToolUse hook (called by Claude Code)
    daemon      Manage the local daemon
    workers     Manage worker fleet
    status      Show system status
    config      Manage configuration
    benchmark   Run performance benchmarks
    install     Install/update RCH
    help        Print this message

GLOBAL OPTIONS:
    -v, --verbose    Verbose output
    -q, --quiet      Suppress non-error output
        --json       Output as JSON
```

### 17.2 Subcommand Details

```
rch daemon
    start       Start the daemon
    stop        Stop the daemon
    restart     Restart the daemon
    status      Show daemon status
    logs        Tail daemon logs

rch workers
    list        List all configured workers
    add         Add a new worker
    remove      Remove a worker
    probe       Test connectivity to workers
    benchmark   Run benchmarks on workers
    sync        Synchronize toolchains

rch status
    -w, --workers    Show worker details
    -j, --jobs       Show active jobs
    -m, --metrics    Show metrics

rch config
    show        Show current configuration
    edit        Open config in editor
    validate    Validate configuration
    reset       Reset to defaults

rch install
    --fleet         Install on all workers
    --worker-only   Install worker agent only
    --easy-mode     Non-interactive setup
    --from-source   Build from source
```

### 17.3 Usage Examples

```bash
# Start daemon
rch daemon start

# Check status
rch status
# Output:
# RCH Status
# ├── Daemon: running (pid 12345, uptime 2h 34m)
# ├── Active compilations: 3
# └── Workers:
#     ├── css (203.0.113.20): healthy, 8/32 slots, score 87.3
#     ├── csd (198.51.100.20): healthy, 2/16 slots, score 72.1
#     ├── yto (203.0.113.30): healthy, 4/8 slots, score 68.9
#     └── fmd (192.0.2.40): degraded, 0/8 slots, score 65.2

# Add new worker
rch workers add --id newbox --host 203.0.113.50 --user ubuntu

# Benchmark all workers
rch workers benchmark --all

# View active jobs
rch status --jobs
# Output:
# Active Jobs
# ├── job-a1b2c3: cargo build --release (css, 4 slots, 45s elapsed)
# ├── job-d4e5f6: cargo test (csd, 2 slots, 12s elapsed)
# └── job-g7h8i9: gcc -O3 main.c (yto, 1 slot, 3s elapsed)

# Force local execution
RCH_LOCAL_ONLY=1 cargo build

# Bypass RCH entirely
RCH_BYPASS=1 cargo build
```

---

## 18. Implementation Roadmap

### Phase 1: Foundation (Week 1)

**Goal**: Basic end-to-end remote compilation for Rust

- [ ] Project scaffold with Cargo workspace
- [ ] Hook JSON parsing and command classification
- [ ] Single-worker SSH execution
- [ ] Basic rsync transfer pipeline
- [ ] Artifact return
- [ ] Installation script (local only)

**Deliverable**: `cargo build` works on a single remote worker

### Phase 2: Fleet Management (Week 2)

**Goal**: Multi-worker orchestration

- [ ] Local daemon with Unix socket API
- [ ] Worker configuration system
- [ ] Slot tracking and reservation
- [ ] Worker health monitoring
- [ ] Worker selection algorithm
- [ ] SSH connection pooling

**Deliverable**: Automatic worker selection with slot management

### Phase 3: Performance (Week 3)

**Goal**: Competitive with local compilation

- [ ] SSH multiplexing
- [ ] Incremental rsync
- [ ] Project caching on workers
- [ ] Compression tuning
- [ ] Parallel transfer option
- [ ] Benchmark integration (cloud_benchmarker or inline)

**Deliverable**: <15% overhead vs. local for warm cache

### Phase 4: Resilience (Week 4)

**Goal**: Production-ready reliability

- [ ] Circuit breaker pattern
- [ ] Graceful degradation / local fallback
- [ ] Retry logic with exponential backoff
- [ ] State recovery after daemon restart
- [ ] Comprehensive error handling

**Deliverable**: Handles network failures gracefully

### Phase 5: Polish (Week 5)

**Goal**: Complete product

- [ ] C/C++ support (gcc, clang, make)
- [ ] Fleet installation script
- [ ] Toolchain synchronization
- [ ] CLI interface completion
- [ ] Prometheus metrics
- [ ] Structured logging
- [ ] Documentation
- [ ] Tests

**Deliverable**: Full release

### Phase 6: Advanced Features (Future)

- [ ] Cross-compilation support
- [ ] Build artifact caching (sccache integration)
- [ ] Queue system for overloaded workers
- [ ] Web dashboard
- [ ] Auto-scaling (spin up cloud instances)
- [ ] Multi-project dependency awareness

---

## 19. File Structure

```
remote_compilation_helper/
├── Cargo.toml                  # Workspace manifest
├── Cargo.lock
├── README.md
├── AGENTS.md                   # AI agent guidelines
├── LICENSE
│
├── install.sh                  # Main installer
├── install.ps1                 # Windows installer
│
├── rch/                        # Hook CLI binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs             # Entry point
│       ├── hook.rs             # Claude Code hook protocol
│       ├── classify.rs         # Command classification
│       ├── transfer.rs         # rsync integration
│       ├── execute.rs          # Remote execution
│       ├── artifacts.rs        # Artifact return
│       └── config.rs           # Configuration loading
│
├── rchd/                       # Local daemon binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs             # Daemon entry point
│       ├── api.rs              # Unix socket API
│       ├── workers.rs          # Worker management
│       ├── slots.rs            # Slot tracking
│       ├── health.rs           # Health monitoring
│       ├── selection.rs        # Worker selection
│       ├── ssh_pool.rs         # Connection pooling
│       └── metrics.rs          # Prometheus metrics
│
├── rch-wkr/                    # Worker agent binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs             # Worker entry point
│       ├── executor.rs         # Compilation execution
│       ├── cache.rs            # Project cache management
│       └── health.rs           # Health check responder
│
├── rch-common/                 # Shared library
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── types.rs            # Common types
│       ├── protocol.rs         # IPC protocol definitions
│       ├── ssh.rs              # SSH utilities
│       ├── compression.rs      # zstd helpers
│       └── patterns.rs         # Command patterns
│
├── config/                     # Default configs
│   ├── config.toml.example
│   └── workers.toml.example
│
├── scripts/
│   ├── setup_worker.sh         # Worker setup script
│   ├── benchmark.sh            # Quick benchmark
│   └── e2e_test.sh             # End-to-end tests
│
├── docs/
│   ├── architecture.md
│   ├── configuration.md
│   ├── troubleshooting.md
│   └── performance-tuning.md
│
└── tests/
    ├── integration/
    │   ├── hook_test.rs
    │   ├── transfer_test.rs
    │   └── execution_test.rs
    └── e2e/
        └── full_workflow_test.rs
```

---

## Appendices

### Appendix A: Hook Protocol Reference

**Input (from Claude Code):**
```json
{
  "session_id": "abc123",
  "tool_name": "Bash",
  "tool_input": {
    "command": "cargo build --release",
    "description": "Build the project in release mode"
  }
}
```

**Output (allow - remote executed):**
```json
{}
```
Or simply exit with no stdout.

**Output (block - if needed):**
```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "deny",
    "permissionDecisionReason": "RCH: Remote execution failed, please retry"
  }
}
```

### Appendix B: Worker Probe Script

```bash
#!/usr/bin/env bash
# probe_worker.sh - Quick worker capability check

set -euo pipefail

HOST="$1"
USER="${2:-ubuntu}"
KEY="${3:-}"

SSH_OPTS="-o StrictHostKeyChecking=accept-new -o ConnectTimeout=5"
if [[ -n "$KEY" ]]; then
    SSH_OPTS="$SSH_OPTS -i $KEY"
fi

echo "Probing $USER@$HOST..."

ssh $SSH_OPTS "$USER@$HOST" << 'EOF'
echo "=== System Info ==="
uname -a
echo "Cores: $(nproc)"
echo "Memory: $(free -h | awk '/^Mem:/ {print $2}')"

echo ""
echo "=== Rust ==="
if command -v rustc &> /dev/null; then
    rustc --version
    cargo --version
else
    echo "Not installed"
fi

echo ""
echo "=== C/C++ ==="
gcc --version 2>/dev/null | head -1 || echo "GCC: Not installed"
clang --version 2>/dev/null | head -1 || echo "Clang: Not installed"

echo ""
echo "=== Tools ==="
zstd --version 2>/dev/null || echo "zstd: Not installed"
rsync --version 2>/dev/null | head -1 || echo "rsync: Not installed"
EOF
```

### Appendix C: Performance Comparison

Expected performance for different scenarios:

| Scenario | Local | Remote (Cold) | Remote (Warm) | Overhead |
|----------|-------|---------------|---------------|----------|
| Fresh `cargo build` (medium project) | 60s | 75s | 62s | +3% |
| Incremental `cargo build` (1 file changed) | 8s | 12s | 9s | +12% |
| `cargo build --release` (large project) | 180s | 195s | 182s | +1% |
| `cargo test` | 45s | 58s | 48s | +7% |
| `gcc -O3` (single file) | 2s | 5s | 3s | +50% |
| `make -j16` (large C project) | 120s | 135s | 122s | +2% |

**Key Insight**: Overhead is amortized for longer compilations. Short compilations have higher relative overhead.

### Appendix D: Troubleshooting

| Symptom | Likely Cause | Solution |
|---------|--------------|----------|
| "No available workers" | All workers at capacity | Wait, or add more workers |
| Transfer timeout | Large codebase, slow network | Increase `transfer.timeout_sec` |
| "Permission denied" | SSH key issue | Check `identity_file` config |
| Compilation fails remotely | Missing dependencies | Run toolchain sync |
| Artifacts missing | Return transfer failed | Check worker disk space |
| Daemon won't start | Port/socket in use | `rch daemon stop` first |

---

## Summary

The **Remote Compilation Helper (RCH)** is a comprehensive solution for offloading compilation work from an overloaded agent workstation to a fleet of remote workers. By intercepting compilation commands via Claude Code's PreToolUse hook and transparently executing them remotely, RCH enables:

1. **15+ agents** compiling simultaneously without local CPU contention
2. **Zero agent awareness** - agents issue normal `cargo build` commands
3. **Intelligent worker selection** based on available slots and speed scores
4. **Efficient transfer** using zstd compression and rsync delta encoding
5. **Graceful fallback** to local execution when workers are unavailable
6. **Easy deployment** via single-command installer

The system is designed with the same principles as DCG: performance-critical (<5ms hook latency), fail-open, and deeply integrated with the Claude Code ecosystem.

---

*Document Version: 1.0*
*Last Updated: 2026-01-16*
*Author: Claude (with human guidance)*
