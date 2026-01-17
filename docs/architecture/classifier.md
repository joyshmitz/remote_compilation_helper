# 5-Tier Command Classifier

## Overview

The RCH classifier determines whether a command should be executed locally or remotely. It uses a 5-tier system optimized for fast rejection of non-compilation commands while accurately identifying compilation workloads.

**Design Goals:**
- 99% of commands are non-compilation; reject them in <0.1ms
- Compilation decisions must complete in <5ms
- Precision over recall: false positives are catastrophic, false negatives are acceptable
- Fail-open: on any error, allow local execution

## Tier Descriptions

### Tier 0: Instant Reject

**Latency:** ~0.01ms
**Purpose:** Reject empty or malformed commands immediately

- Empty command string → reject
- Non-Bash tool → pass through (only Bash commands are classified)

```rust
if command.is_empty() {
    return Classification::not_compilation("empty command");
}
```

### Tier 1: Structure Analysis

**Latency:** ~0.1ms
**Purpose:** Reject commands with shell metacharacters that complicate remote execution

**Rejects:**
- Piped commands: `cargo build | grep error`
- Redirected output: `cargo build > log.txt`
- Backgrounded: `cargo build &`
- Command chaining: `cargo build && ./run`
- Subshell capture: `$(cargo build)`

```rust
// Structural patterns that prevent interception
if command.contains('|') || command.contains('>') || command.contains('<') {
    return Classification::not_compilation("contains shell metacharacters");
}
```

**Rationale:** These patterns indicate complex shell interactions that don't translate cleanly to remote execution. The agent expects specific local behavior (output redirection, piping to another command).

### Tier 2: SIMD Keyword Filter

**Latency:** ~0.2ms
**Purpose:** Fast-path rejection for commands that don't contain any compilation-related keywords

**Keywords scanned (SIMD-accelerated with memchr):**
```rust
pub static COMPILATION_KEYWORDS: &[&str] = &[
    "cargo", "rustc",
    "gcc", "g++", "clang", "clang++", "cc", "c++",
    "make", "cmake", "ninja", "meson",
    "bun",
];
```

If none of these keywords appear in the command, it's instantly rejected without further analysis.

**Example matches:**
- `cargo build` → contains "cargo" → continue to Tier 3
- `ls -la` → no keywords → reject
- `git status` → no keywords → reject

### Tier 3: Negative Pattern Check

**Latency:** ~0.5ms
**Purpose:** Reject commands that contain compilation keywords but shouldn't be intercepted

**NEVER_INTERCEPT patterns (71 patterns):**

| Category | Examples |
|----------|----------|
| **Package management** | `cargo install`, `bun install`, `bun add`, `bun remove` |
| **Source modification** | `cargo fmt`, `rustfmt` |
| **Local execution** | `cargo run`, `bun run`, `bun dev`, `bun repl` |
| **Bundling** | `bun build` (creates local bundles) |
| **Package runners** | `bun x`, `bunx` (like npx) |
| **Version checks** | `cargo --version`, `rustc -V`, `gcc --version` |
| **Help** | `cargo --help`, `make --help` |
| **Maintenance** | `cargo clean`, `make clean` |
| **Interactive** | `bun test --watch`, `bun typecheck --watch` |

```rust
for pattern in NEVER_INTERCEPT {
    if command.starts_with(pattern) {
        return Classification::not_compilation("never-intercept pattern");
    }
}
```

### Tier 4: Full Classification

**Latency:** ~5ms
**Purpose:** Definitively classify the command and assign confidence score

**Command normalization:** Strips wrapper commands to find the actual command:
- `time cargo build` → `cargo build`
- `sudo cargo check` → `cargo check`
- `env RUSTFLAGS="-C target-cpu=native" cargo build` → `cargo build`

**Classification kinds:**

| Kind | Pattern | Confidence |
|------|---------|------------|
| `CargoBuild` | `cargo build`, `cargo b` | 0.95 |
| `CargoTest` | `cargo test`, `cargo t` | 0.90 |
| `CargoCheck` | `cargo check`, `cargo c` | 0.90 |
| `CargoClippy` | `cargo clippy` | 0.90 |
| `CargoDoc` | `cargo doc` | 0.85 |
| `Rustc` | `rustc file.rs` | 0.85 |
| `Gcc` | `gcc`, `cc` | 0.85 |
| `Gpp` | `g++`, `c++` | 0.85 |
| `Clang` | `clang` | 0.85 |
| `Clangpp` | `clang++` | 0.85 |
| `Make` | `make`, `make target` | 0.85 |
| `CmakeBuild` | `cmake --build` | 0.85 |
| `Ninja` | `ninja` | 0.85 |
| `Meson` | `meson compile` | 0.85 |
| `BunTest` | `bun test` | 0.90 |
| `BunTypecheck` | `bun typecheck` | 0.90 |

**Confidence threshold:** Default is 0.85. Commands below threshold run locally.

## Classification Output

```rust
pub struct Classification {
    /// Whether this is a compilation command
    pub is_compilation: bool,

    /// The type of compilation (if applicable)
    pub kind: Option<CompilationKind>,

    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,

    /// Human-readable reason for the classification
    pub reason: String,
}
```

## Performance Budget

| Tier | Target Latency | Panic Threshold | Memory |
|------|----------------|-----------------|--------|
| 0 | 0.01ms | 0.1ms | 0 |
| 1 | 0.1ms | 0.5ms | 0 |
| 2 | 0.2ms | 1ms | 0 |
| 3 | 0.5ms | 2ms | 0 |
| 4 | 5ms | 10ms | <100KB |
| **Total (compilation)** | <5ms | 10ms | <100KB |
| **Total (non-compilation)** | <1ms | 5ms | 0 |

## Edge Cases

### Wrapped Commands

Commands wrapped with common prefixes are unwrapped before classification:

```bash
# These are all classified as CargoBuild:
cargo build
time cargo build
sudo cargo build
env RUSTFLAGS="-C lto" cargo build
nice -n 10 cargo build
```

### Watch Modes

Watch modes are interactive and run locally:

```bash
bun test --watch    # NOT intercepted
bun test -w         # NOT intercepted
bun typecheck -w    # NOT intercepted
```

### Package Runners

Package runners execute arbitrary code and are not intercepted:

```bash
bun x eslint .      # NOT intercepted
bunx prettier       # NOT intercepted
```

## Testing the Classifier

```bash
# Test specific commands
rch classify "cargo build --release"
rch classify "bun test"
rch classify "gcc main.c -o main"

# Verbose output
RCH_LOG_LEVEL=debug rch classify "command"
```

## Extending the Classifier

To add support for a new build tool:

1. Add keyword to `COMPILATION_KEYWORDS` in `rch-common/src/patterns.rs`
2. Add any never-intercept patterns to `NEVER_INTERCEPT`
3. Add a new `CompilationKind` variant
4. Add classification logic in the Tier 4 section
5. Add comprehensive tests

Example for adding Meson support:

```rust
// In patterns.rs
pub static COMPILATION_KEYWORDS: &[&str] = &[
    // ... existing
    "meson",  // Add keyword
];

pub static NEVER_INTERCEPT: &[&str] = &[
    // ... existing
    "meson setup",     // Setup creates build directory
    "meson configure", // Configuration, not compilation
];

pub enum CompilationKind {
    // ... existing
    Meson,
}

// In classification logic
if cmd.starts_with("meson compile") {
    return Classification::compilation(
        CompilationKind::Meson,
        0.85,
        "meson compile command",
    );
}
```

## Related Documentation

- [ADR-001: Unix Socket IPC](../adr/001-unix-socket-ipc.md)
- [Transfer Pipeline](./transfer.md)
- [Worker Selection](./selection.md)
