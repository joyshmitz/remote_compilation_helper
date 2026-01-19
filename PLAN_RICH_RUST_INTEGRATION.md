# Comprehensive Plan: Integrating rich_rust into RCH

> **Goal**: Add beautiful, premium terminal output throughout RCH while preserving agent compatibility.

---

## Executive Summary

This plan integrates `rich_rust` into the Remote Compilation Helper (RCH) to provide stunning visual output for human operators watching AI agents work, while ensuring **zero interference** with the agents themselves. The key insight is that agents interact through structured protocols (JSON, exit codes, streamed output), while humans observe through status commands and daemon logs.

---

## Table of Contents

1. [Design Principles](#1-design-principles)
2. [Output Classification](#2-output-classification)
3. [Architecture Overview](#3-architecture-overview)
4. [Implementation Phases](#4-implementation-phases)
5. [Detailed Component Plans](#5-detailed-component-plans)
6. [Performance Considerations](#6-performance-considerations)
7. [Testing Strategy](#7-testing-strategy)
8. [Migration Path](#8-migration-path)

---

## 1. Design Principles

### 1.1 Agent-First Architecture

**The Cardinal Rule**: Agents are the primary users. Every design decision must ensure agents function identically whether rich output is enabled or not.

| Principle | Implementation |
|-----------|----------------|
| **Structured data is sacred** | JSON output, exit codes, and streamed compilation output are NEVER modified by rich formatting |
| **stderr for humans, stdout for machines** | Rich output goes to stderr; machine-parseable data goes to stdout |
| **Graceful degradation** | If rich_rust fails, fall back to plain text—never crash |
| **Environment respect** | Honor `NO_COLOR`, `FORCE_COLOR`, `--quiet`, `--json` flags |
| **Zero overhead in hot path** | Hook classification path must not import rich_rust |

### 1.2 Context Detection

RCH operates in multiple contexts with different output requirements:

| Context | Detection | Rich Output | Notes |
|---------|-----------|-------------|-------|
| **Hook mode** | `rch` called with JSON on stdin | **NO** | Pure JSON protocol |
| **CLI interactive** | TTY detected on stderr | **YES** | Full rich experience |
| **CLI piped** | No TTY on stderr | **Minimal** | Colors only if `FORCE_COLOR` |
| **Daemon foreground** | `rchd` with TTY | **YES** | Rich status, spinners |
| **Daemon background** | `rchd` as service | **NO** | Plain logs only |
| **Worker agent** | `rch-wkr execute` | **NO** | Passthrough only |

### 1.3 Progressive Enhancement

```
Level 0: Plain text (always works, agent-safe)
    ↓
Level 1: ANSI colors (most terminals)
    ↓
Level 2: Unicode boxes, spinners (UTF-8 terminals)
    ↓
Level 3: Full rich (tables, panels, progress bars)
```

---

## 2. Output Classification

### 2.1 Machine-Only Output (NEVER add rich formatting)

| Component | Output Type | Reason |
|-----------|-------------|--------|
| `rch` hook JSON responses | stdout | Agent protocol |
| `rch --format json` | stdout | Explicit machine format |
| Compilation output passthrough | stdout | Agent reads this |
| Test output streaming | stdout | Agent reads this |
| Exit codes | exit status | Agent logic depends on this |

### 2.2 Human-Facing Output (Add rich formatting)

| Component | Output Type | Rich Features |
|-----------|-------------|---------------|
| `rch status` | stderr | Tables, panels, colors |
| `rch workers list` | stderr | Tables with status indicators |
| `rch workers probe` | stderr | Progress spinners, success/fail colors |
| `rch workers benchmark` | stderr | Progress bars, result tables |
| `rchd` startup banner | stderr | ASCII art, version panel |
| `rchd` worker health | stderr | Status colors, tables |
| Error messages | stderr | Panels with context |
| Warning messages | stderr | Styled warnings |

### 2.3 Hybrid Output (Conditional formatting)

| Component | Condition | Behavior |
|-----------|-----------|----------|
| `rch daemon logs` | `--rich` flag | Rich if flag, plain otherwise |
| Compilation errors | TTY + not hook | Rich panel with error |
| Transfer progress | TTY + not hook | Progress bar |

---

## 3. Architecture Overview

### 3.1 New Module Structure

```
rch/
├── src/
│   ├── ui/                          # NEW: Rich UI module
│   │   ├── mod.rs                   # Re-exports, context detection
│   │   ├── console.rs               # Wrapped Console with context awareness
│   │   ├── theme.rs                 # RCH color theme
│   │   ├── components/
│   │   │   ├── mod.rs
│   │   │   ├── banner.rs            # Startup banner
│   │   │   ├── status_table.rs      # Worker/job status tables
│   │   │   ├── progress.rs          # Transfer/benchmark progress
│   │   │   ├── error_panel.rs       # Error display
│   │   │   └── worker_card.rs       # Individual worker display
│   │   └── formatters/
│   │       ├── mod.rs
│   │       ├── worker.rs            # Worker formatting
│   │       ├── job.rs               # Job formatting
│   │       └── metrics.rs           # Metrics formatting
│   └── ...existing files

rchd/
├── src/
│   ├── ui/                          # NEW: Daemon UI module
│   │   ├── mod.rs
│   │   ├── dashboard.rs             # Live daemon dashboard
│   │   └── log_formatter.rs         # Rich log formatting
│   └── ...existing files

rch-common/
├── src/
│   ├── ui/                          # NEW: Shared UI utilities
│   │   ├── mod.rs
│   │   ├── theme.rs                 # Shared theme constants
│   │   ├── icons.rs                 # Unicode icons with fallbacks
│   │   └── context.rs               # Output context detection
│   └── ...existing files
```

### 3.2 Dependency Addition

```toml
# In workspace Cargo.toml
[workspace.dependencies]
rich_rust = { path = "/dp/rich_rust", features = ["full"] }

# Or when published:
# rich_rust = { version = "0.1", features = ["full"] }

# In rch/Cargo.toml
[dependencies]
rich_rust = { workspace = true }

# In rchd/Cargo.toml
[dependencies]
rich_rust = { workspace = true }

# In rch-common/Cargo.toml (minimal features)
[dependencies]
rich_rust = { workspace = true, default-features = false }
```

### 3.3 Context-Aware Console Wrapper

```rust
// rch-common/src/ui/context.rs

use std::io::IsTerminal;

/// Output context determines what level of rich formatting to use
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputContext {
    /// Hook mode - JSON only, no decoration
    Hook,
    /// Machine-readable output requested
    Machine,
    /// Interactive terminal
    Interactive,
    /// Non-interactive but colored
    Colored,
    /// Plain text only
    Plain,
}

impl OutputContext {
    pub fn detect() -> Self {
        // Check for explicit machine output
        if std::env::var("RCH_JSON").is_ok() {
            return Self::Machine;
        }

        // Check for hook mode (JSON on stdin)
        if is_hook_invocation() {
            return Self::Hook;
        }

        // Check NO_COLOR
        if std::env::var("NO_COLOR").is_ok() {
            return Self::Plain;
        }

        // Check if stderr is a terminal (we output rich to stderr)
        if std::io::stderr().is_terminal() {
            return Self::Interactive;
        }

        // Check FORCE_COLOR
        if std::env::var("FORCE_COLOR").is_ok() {
            return Self::Colored;
        }

        Self::Plain
    }

    pub fn supports_rich(&self) -> bool {
        matches!(self, Self::Interactive)
    }

    pub fn supports_color(&self) -> bool {
        matches!(self, Self::Interactive | Self::Colored)
    }

    pub fn is_machine(&self) -> bool {
        matches!(self, Self::Hook | Self::Machine)
    }
}

fn is_hook_invocation() -> bool {
    // Hook mode detected by:
    // 1. Stdin is not a terminal (piped JSON)
    // 2. First arg is not a subcommand
    !std::io::stdin().is_terminal()
        && std::env::args().nth(1).map_or(true, |a| !a.starts_with('-'))
}
```

---

## 4. Implementation Phases

### Phase 1: Foundation (Week 1)

**Goal**: Establish infrastructure without changing existing behavior.

1. **Add rich_rust dependency** to workspace
2. **Create ui module structure** in rch-common, rch, rchd
3. **Implement OutputContext** detection
4. **Create RchConsole wrapper** that respects context
5. **Define color theme** constants
6. **Add feature flag** `rich-ui` (default on)

**Deliverables**:
- `rch-common/src/ui/mod.rs` with context detection
- `rch-common/src/ui/theme.rs` with color constants
- Tests for context detection
- Feature flag in Cargo.toml

### Phase 2: CLI Commands (Week 2)

**Goal**: Rich output for status and worker commands.

1. **`rch status`** - Daemon status panel, worker table
2. **`rch workers list`** - Worker table with health indicators
3. **`rch workers probe`** - Progress spinner, success/fail indicators
4. **`rch workers benchmark`** - Progress bar, results table
5. **`rch status --jobs`** - Active jobs table

**Deliverables**:
- Status command with rich tables
- Worker commands with progress indicators
- All commands respect `--json` flag for machine output

### Phase 3: Daemon UI (Week 3)

**Goal**: Rich output for daemon startup and operation.

1. **Startup banner** - Version, config summary, ASCII art
2. **Worker online/offline** - Colored status messages
3. **Health check results** - Status table
4. **Request logging** - Formatted log entries (optional)

**Deliverables**:
- Daemon startup experience
- Rich log formatting option
- Dashboard mode (optional future work)

### Phase 4: Error Experience (Week 4)

**Goal**: Beautiful, informative error messages.

1. **Compilation errors** - Panel with context, suggestions
2. **Connection errors** - Worker info, retry suggestions
3. **Configuration errors** - What's wrong, how to fix
4. **Transfer errors** - Progress context, failure point

**Deliverables**:
- Error panel component
- Contextual error formatting
- Help text integration

### Phase 5: Progress & Feedback (Week 5)

**Goal**: Progress indicators for long operations.

1. **File transfer** - Progress bar with speed, ETA
2. **Benchmark runs** - Multi-phase progress
3. **Fleet installation** - Per-worker progress
4. **Artifact retrieval** - Download progress

**Deliverables**:
- Progress bar integration
- Multi-step progress tracking
- Speed and ETA display

### Phase 6: Polish & Documentation (Week 6)

**Goal**: Final polish and documentation.

1. **Theme refinement** - Color consistency, accessibility
2. **Documentation** - Update README, AGENTS.md
3. **Examples** - Screenshot gallery
4. **Performance audit** - Ensure no hot path impact

**Deliverables**:
- Polished visual experience
- Updated documentation
- Performance validation

---

## 5. Detailed Component Plans

### 5.1 Color Theme

```rust
// rch-common/src/ui/theme.rs

use rich_rust::prelude::*;

/// RCH brand colors and semantic styles
pub struct RchTheme;

impl RchTheme {
    // Brand colors
    pub const PRIMARY: &'static str = "#7C3AED";      // Purple
    pub const SECONDARY: &'static str = "#06B6D4";    // Cyan
    pub const ACCENT: &'static str = "#F59E0B";       // Amber

    // Semantic colors
    pub const SUCCESS: &'static str = "#10B981";      // Green
    pub const WARNING: &'static str = "#F59E0B";      // Amber
    pub const ERROR: &'static str = "#EF4444";        // Red
    pub const INFO: &'static str = "#3B82F6";         // Blue

    // Status colors
    pub const HEALTHY: &'static str = "#10B981";      // Green
    pub const DEGRADED: &'static str = "#F59E0B";     // Amber
    pub const UNREACHABLE: &'static str = "#EF4444";  // Red
    pub const DRAINING: &'static str = "#8B5CF6";     // Purple
    pub const DISABLED: &'static str = "#6B7280";     // Gray

    // Text colors
    pub const MUTED: &'static str = "#9CA3AF";        // Gray-400
    pub const DIM: &'static str = "#6B7280";          // Gray-500

    // Semantic styles
    pub fn success() -> Style {
        Style::new().color(Color::parse(Self::SUCCESS).unwrap())
    }

    pub fn error() -> Style {
        Style::new().bold().color(Color::parse(Self::ERROR).unwrap())
    }

    pub fn warning() -> Style {
        Style::new().color(Color::parse(Self::WARNING).unwrap())
    }

    pub fn info() -> Style {
        Style::new().color(Color::parse(Self::INFO).unwrap())
    }

    pub fn muted() -> Style {
        Style::new().color(Color::parse(Self::MUTED).unwrap())
    }

    pub fn worker_status(status: WorkerStatus) -> Style {
        let color = match status {
            WorkerStatus::Healthy => Self::HEALTHY,
            WorkerStatus::Degraded => Self::DEGRADED,
            WorkerStatus::Unreachable => Self::UNREACHABLE,
            WorkerStatus::Draining => Self::DRAINING,
            WorkerStatus::Disabled => Self::DISABLED,
        };
        Style::new().color(Color::parse(color).unwrap())
    }
}
```

### 5.2 Unicode Icons with ASCII Fallbacks

```rust
// rch-common/src/ui/icons.rs

use crate::ui::context::OutputContext;

/// Icons with automatic fallback for non-Unicode terminals
pub struct Icons;

impl Icons {
    pub fn check(ctx: OutputContext) -> &'static str {
        if ctx.supports_unicode() { "✓" } else { "[OK]" }
    }

    pub fn cross(ctx: OutputContext) -> &'static str {
        if ctx.supports_unicode() { "✗" } else { "[FAIL]" }
    }

    pub fn warning(ctx: OutputContext) -> &'static str {
        if ctx.supports_unicode() { "⚠" } else { "[WARN]" }
    }

    pub fn info(ctx: OutputContext) -> &'static str {
        if ctx.supports_unicode() { "ℹ" } else { "[INFO]" }
    }

    pub fn arrow_right(ctx: OutputContext) -> &'static str {
        if ctx.supports_unicode() { "→" } else { "->" }
    }

    pub fn bullet(ctx: OutputContext) -> &'static str {
        if ctx.supports_unicode() { "•" } else { "*" }
    }

    pub fn worker(ctx: OutputContext) -> &'static str {
        if ctx.supports_unicode() { "🖥" } else { "[W]" }
    }

    pub fn compile(ctx: OutputContext) -> &'static str {
        if ctx.supports_unicode() { "🔨" } else { "[C]" }
    }

    pub fn transfer(ctx: OutputContext) -> &'static str {
        if ctx.supports_unicode() { "📦" } else { "[T]" }
    }

    pub fn clock(ctx: OutputContext) -> &'static str {
        if ctx.supports_unicode() { "⏱" } else { "[T]" }
    }

    pub fn slot(ctx: OutputContext) -> &'static str {
        if ctx.supports_unicode() { "◉" } else { "o" }
    }

    pub fn slot_empty(ctx: OutputContext) -> &'static str {
        if ctx.supports_unicode() { "○" } else { "." }
    }
}
```

### 5.3 Worker Status Table

```rust
// rch/src/ui/components/status_table.rs

use rich_rust::prelude::*;
use rch_common::types::{WorkerState, WorkerStatus};
use crate::ui::theme::RchTheme;

pub fn render_worker_table(
    console: &Console,
    workers: &[WorkerState],
) {
    let mut table = Table::new()
        .title("Worker Fleet")
        .box_style(&rich_rust::r#box::ROUNDED)
        .border_style(Style::new().color(Color::parse(RchTheme::PRIMARY).unwrap()))
        .header_style(Style::new().bold())
        .with_column(Column::new("Worker").min_width(12))
        .with_column(Column::new("Status").min_width(10))
        .with_column(Column::new("Slots").justify(JustifyMethod::Center).min_width(12))
        .with_column(Column::new("Speed").justify(JustifyMethod::Right).min_width(8))
        .with_column(Column::new("Latency").justify(JustifyMethod::Right).min_width(10))
        .with_column(Column::new("Projects").min_width(15));

    for worker in workers {
        let status_style = RchTheme::worker_status(worker.status);
        let status_text = format_status(worker.status);

        let slots_display = format_slots(
            worker.used_slots,
            worker.config.total_slots,
        );

        let speed = worker.speed_score
            .map(|s| format!("{:.1}", s))
            .unwrap_or_else(|| "—".to_string());

        let latency = worker.last_latency_ms
            .map(|l| format!("{}ms", l))
            .unwrap_or_else(|| "—".to_string());

        let projects = worker.cached_projects
            .iter()
            .take(3)
            .map(|p| truncate_project(p, 15))
            .collect::<Vec<_>>()
            .join(", ");

        table.add_row(Row::new()
            .cell(&worker.config.id)
            .cell_styled(&status_text, status_style)
            .cell(&slots_display)
            .cell(&speed)
            .cell(&latency)
            .cell(&projects));
    }

    console.print_renderable(&table);
}

fn format_status(status: WorkerStatus) -> String {
    match status {
        WorkerStatus::Healthy => "● Healthy",
        WorkerStatus::Degraded => "◐ Degraded",
        WorkerStatus::Unreachable => "○ Offline",
        WorkerStatus::Draining => "◑ Draining",
        WorkerStatus::Disabled => "◌ Disabled",
    }.to_string()
}

fn format_slots(used: u32, total: u32) -> String {
    let bar_width = 8;
    let filled = ((used as f64 / total as f64) * bar_width as f64) as usize;
    let empty = bar_width - filled;

    format!(
        "{}{} {}/{}",
        "█".repeat(filled),
        "░".repeat(empty),
        used,
        total
    )
}

fn truncate_project(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_string()
    } else {
        format!("{}…", &name[..max_len - 1])
    }
}
```

### 5.4 Daemon Startup Banner

```rust
// rchd/src/ui/banner.rs

use rich_rust::prelude::*;
use crate::ui::theme::RchTheme;

const BANNER: &str = r#"
    ╭─────────────────────────────────────────╮
    │                                         │
    │   ██████╗  ██████╗██╗  ██╗██████╗      │
    │   ██╔══██╗██╔════╝██║  ██║██╔══██╗     │
    │   ██████╔╝██║     ███████║██║  ██║     │
    │   ██╔══██╗██║     ██╔══██║██║  ██║     │
    │   ██║  ██║╚██████╗██║  ██║██████╔╝     │
    │   ╚═╝  ╚═╝ ╚═════╝╚═╝  ╚═╝╚═════╝      │
    │                                         │
    │   Remote Compilation Helper Daemon      │
    │                                         │
    ╰─────────────────────────────────────────╯
"#;

pub fn print_startup_banner(
    console: &Console,
    version: &str,
    socket_path: &str,
    worker_count: usize,
    total_slots: u32,
) {
    // Print ASCII art banner
    console.print_styled(
        BANNER,
        Style::new().color(Color::parse(RchTheme::PRIMARY).unwrap())
    );

    // Version info
    console.print(&format!(
        "[bold]Version:[/] {} [dim]• Rust {}[/]",
        version,
        env!("RUSTC_VERSION", "nightly")
    ));

    console.line();

    // Configuration summary
    let mut info_table = Table::new()
        .box_style(&rich_rust::r#box::ROUNDED)
        .show_header(false)
        .padding((1, 0))
        .with_column(Column::new("Key").style(Style::new().bold()))
        .with_column(Column::new("Value"));

    info_table.add_row_cells(["Socket", socket_path]);
    info_table.add_row_cells(["Workers", &format!("{} online", worker_count)]);
    info_table.add_row_cells(["Total Slots", &total_slots.to_string()]);
    info_table.add_row_cells(["PID", &std::process::id().to_string()]);

    console.print_renderable(&info_table);

    console.line();
    console.rule(Some("Ready"));
}
```

### 5.5 Error Panel

```rust
// rch/src/ui/components/error_panel.rs

use rich_rust::prelude::*;
use crate::ui::theme::RchTheme;

pub struct ErrorDisplay {
    title: String,
    message: String,
    context: Vec<(String, String)>,
    suggestion: Option<String>,
}

impl ErrorDisplay {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            context: Vec::new(),
            suggestion: None,
        }
    }

    pub fn context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.push((key.into(), value.into()));
        self
    }

    pub fn suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    pub fn render(&self, console: &Console) {
        // Build content
        let mut content = Text::new(&self.message);
        content.append("\n", Style::default());

        // Add context
        if !self.context.is_empty() {
            content.append("\n", Style::default());
            for (key, value) in &self.context {
                content.append(&format!("  {} ", key), Style::new().bold());
                content.append(&format!("{}\n", value), RchTheme::muted());
            }
        }

        // Add suggestion
        if let Some(suggestion) = &self.suggestion {
            content.append("\n", Style::default());
            content.append("💡 ", Style::default());
            content.append(suggestion, Style::new().italic());
        }

        // Create panel
        let panel = Panel::from_rich_text(content)
            .title(&format!("✗ {}", self.title))
            .border_style(RchTheme::error())
            .rounded();

        // Print to stderr
        console.print_renderable(&panel);
    }
}

// Usage example:
// ErrorDisplay::new("Connection Failed", "Could not reach worker 'build-server'")
//     .context("Host", "203.0.113.10")
//     .context("Error", "Connection refused")
//     .suggestion("Check that the worker is running and SSH is configured")
//     .render(&console);
```

### 5.6 Progress Bar for Transfers

```rust
// rch/src/ui/components/progress.rs

use rich_rust::prelude::*;
use std::time::{Duration, Instant};
use crate::ui::theme::RchTheme;

pub struct TransferProgress {
    console: Console,
    start: Instant,
    total_bytes: u64,
    transferred: u64,
    phase: TransferPhase,
}

#[derive(Clone, Copy)]
pub enum TransferPhase {
    Uploading,
    Compiling,
    Downloading,
}

impl TransferProgress {
    pub fn new(console: Console, total_bytes: u64) -> Self {
        Self {
            console,
            start: Instant::now(),
            total_bytes,
            transferred: 0,
            phase: TransferPhase::Uploading,
        }
    }

    pub fn set_phase(&mut self, phase: TransferPhase) {
        self.phase = phase;
        self.transferred = 0;
    }

    pub fn update(&mut self, bytes: u64) {
        self.transferred = bytes;
        self.render();
    }

    fn render(&self) {
        let elapsed = self.start.elapsed();
        let percent = if self.total_bytes > 0 {
            (self.transferred as f64 / self.total_bytes as f64 * 100.0) as u64
        } else {
            0
        };

        let speed = if elapsed.as_secs_f64() > 0.0 {
            self.transferred as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };

        let phase_icon = match self.phase {
            TransferPhase::Uploading => "↑",
            TransferPhase::Compiling => "⚙",
            TransferPhase::Downloading => "↓",
        };

        let phase_name = match self.phase {
            TransferPhase::Uploading => "Uploading",
            TransferPhase::Compiling => "Compiling",
            TransferPhase::Downloading => "Downloading",
        };

        // Create progress bar
        let bar = ProgressBar::new()
            .completed(percent as usize)
            .total(100)
            .width(30)
            .style(BarStyle::Block);

        // Format speed
        let speed_str = format_bytes_speed(speed);

        // Print progress line (with carriage return to overwrite)
        self.console.print(&format!(
            "\r[bold]{} {}[/] {} [dim]{}[/]  ",
            phase_icon,
            phase_name,
            bar.render_string(),
            speed_str
        ));
    }

    pub fn finish(&self) {
        let elapsed = self.start.elapsed();
        self.console.print(&format!(
            "\r[bold green]✓[/] Transfer complete in {:.1}s\n",
            elapsed.as_secs_f64()
        ));
    }
}

fn format_bytes_speed(bytes_per_sec: f64) -> String {
    if bytes_per_sec >= 1_000_000_000.0 {
        format!("{:.1} GB/s", bytes_per_sec / 1_000_000_000.0)
    } else if bytes_per_sec >= 1_000_000.0 {
        format!("{:.1} MB/s", bytes_per_sec / 1_000_000.0)
    } else if bytes_per_sec >= 1_000.0 {
        format!("{:.1} KB/s", bytes_per_sec / 1_000.0)
    } else {
        format!("{:.0} B/s", bytes_per_sec)
    }
}
```

### 5.7 RchConsole Wrapper

```rust
// rch/src/ui/console.rs

use rich_rust::prelude::*;
use rch_common::ui::context::OutputContext;
use rch_common::ui::theme::RchTheme;

/// Context-aware console wrapper that respects output mode
pub struct RchConsole {
    inner: Console,
    context: OutputContext,
}

impl RchConsole {
    pub fn new() -> Self {
        let context = OutputContext::detect();
        let inner = Console::builder()
            .force_terminal(context.supports_rich())
            .build();

        Self { inner, context }
    }

    pub fn with_context(context: OutputContext) -> Self {
        let inner = Console::builder()
            .force_terminal(context.supports_rich())
            .build();

        Self { inner, context }
    }

    /// Print only if in interactive mode
    pub fn print_rich(&self, content: &str) {
        if self.context.supports_rich() {
            self.inner.print(content);
        }
    }

    /// Print with fallback to plain text
    pub fn print_or_plain(&self, rich: &str, plain: &str) {
        if self.context.supports_rich() {
            self.inner.print(rich);
        } else if !self.context.is_machine() {
            eprintln!("{}", plain);
        }
    }

    /// Print a renderable, or skip in machine mode
    pub fn print_renderable<R: Renderable>(&self, renderable: &R) {
        if self.context.supports_rich() {
            self.inner.print_renderable(renderable);
        }
    }

    /// Always print (for errors that should show regardless)
    pub fn print_error(&self, error: &ErrorDisplay) {
        if self.context.supports_rich() {
            error.render(&self.inner);
        } else {
            // Plain text fallback
            eprintln!("Error: {}", error.title);
            eprintln!("{}", error.message);
        }
    }

    /// Get the underlying console for advanced use
    pub fn inner(&self) -> &Console {
        &self.inner
    }

    /// Check if rich output is available
    pub fn is_rich(&self) -> bool {
        self.context.supports_rich()
    }

    /// Check if in machine mode
    pub fn is_machine(&self) -> bool {
        self.context.is_machine()
    }
}
```

---

## 6. Performance Considerations

### 6.1 Hot Path Protection

The hook classification path is the most performance-critical. It MUST NOT be affected by rich_rust.

```rust
// rch/src/hook.rs

// The classify function does NOT import any UI modules
pub fn classify_command(cmd: &str) -> Classification {
    // Pure logic, no UI
    // ...
}

// The hook handler only uses UI for errors, and only conditionally
pub fn handle_hook(input: HookInput) -> HookOutput {
    let classification = classify_command(&input.command);

    // Fast path: not compilation
    if !classification.is_compilation() {
        return HookOutput::allow();
    }

    // Slow path: compilation handling
    // UI is only used for errors, which is rare
    match execute_remote(classification) {
        Ok(result) => HookOutput::deny_with_result(result),
        Err(e) => {
            // Only render rich error if TTY and not in pure hook mode
            if should_render_error() {
                render_error(&e);
            }
            HookOutput::allow() // Fail-open
        }
    }
}
```

### 6.2 Lazy Loading

Rich_rust should be lazily loaded to avoid startup time impact:

```rust
use once_cell::sync::Lazy;

static CONSOLE: Lazy<RchConsole> = Lazy::new(|| RchConsole::new());
```

### 6.3 Feature Flag

Allow disabling rich output entirely at compile time:

```toml
[features]
default = ["rich-ui"]
rich-ui = ["rich_rust"]
```

```rust
#[cfg(feature = "rich-ui")]
mod ui;

#[cfg(not(feature = "rich-ui"))]
mod ui {
    pub struct RchConsole;
    impl RchConsole {
        pub fn new() -> Self { Self }
        pub fn print_rich(&self, _: &str) {}
        // ... stub implementations
    }
}
```

### 6.4 Benchmarks

Add benchmarks to ensure no regression:

```rust
// benches/ui_overhead.rs

#[bench]
fn bench_classify_without_ui(b: &mut Bencher) {
    let cmd = "cargo build --release";
    b.iter(|| classify_command(cmd));
}

#[bench]
fn bench_hook_fast_path(b: &mut Bencher) {
    let input = HookInput::new("Bash", "ls -la");
    b.iter(|| handle_hook(input.clone()));
}
```

---

## 7. Testing Strategy

### 7.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_detection_no_color() {
        std::env::set_var("NO_COLOR", "1");
        let ctx = OutputContext::detect();
        assert_eq!(ctx, OutputContext::Plain);
        std::env::remove_var("NO_COLOR");
    }

    #[test]
    fn test_context_detection_force_color() {
        std::env::set_var("FORCE_COLOR", "1");
        let ctx = OutputContext::detect();
        assert!(ctx.supports_color());
        std::env::remove_var("FORCE_COLOR");
    }

    #[test]
    fn test_theme_colors_parse() {
        // Ensure all theme colors are valid
        assert!(Color::parse(RchTheme::PRIMARY).is_ok());
        assert!(Color::parse(RchTheme::ERROR).is_ok());
        // ... etc
    }
}
```

### 7.2 Integration Tests

```rust
// tests/ui_integration.rs

#[test]
fn test_worker_table_renders() {
    let console = Console::builder().force_terminal(true).build();
    let workers = vec![/* test data */];

    // Should not panic
    render_worker_table(&console, &workers);
}

#[test]
fn test_error_panel_renders() {
    let console = Console::builder().force_terminal(true).build();

    ErrorDisplay::new("Test Error", "Test message")
        .context("Key", "Value")
        .suggestion("Try this")
        .render(&console);
}
```

### 7.3 Snapshot Tests

Use `insta` for snapshot testing of rendered output:

```rust
#[test]
fn test_worker_table_snapshot() {
    let output = capture_output(|| {
        render_worker_table(&console, &test_workers());
    });

    insta::assert_snapshot!(strip_ansi(&output));
}
```

### 7.4 CI Verification

```yaml
# .github/workflows/ci.yml
- name: Test with rich-ui
  run: cargo test --features rich-ui

- name: Test without rich-ui
  run: cargo test --no-default-features

- name: Test NO_COLOR behavior
  env:
    NO_COLOR: 1
  run: cargo test
```

---

## 8. Migration Path

### 8.1 Phase 1: Non-Breaking Addition

1. Add rich_rust dependency
2. Create ui modules with new code only
3. No changes to existing output
4. Feature flag `rich-ui` defaults to OFF

### 8.2 Phase 2: Opt-In Rich Output

1. Enable `rich-ui` by default
2. Add `--rich` flag to commands that benefit
3. Existing `--json` flag behavior unchanged
4. Document in CHANGELOG

### 8.3 Phase 3: Rich by Default

1. Rich output for all interactive commands
2. `--plain` flag for legacy plain output
3. Full documentation update
4. Screenshot gallery in README

### 8.4 Backwards Compatibility

| Flag | Behavior |
|------|----------|
| `--json` | Pure JSON, no decoration (unchanged) |
| `--quiet` | Minimal output (unchanged) |
| `--plain` | NEW: Force plain text, no ANSI |
| `--rich` | NEW: Force rich output even if piped |
| `NO_COLOR=1` | Disable colors (standard) |
| `FORCE_COLOR=1` | Enable colors even if piped |

---

## Appendix A: Example Output Mockups

### A.1 `rch status` Output

```
╭─────────────────────────────────────────────────────────────╮
│                     RCH Daemon Status                        │
├─────────────────────────────────────────────────────────────┤
│  Status   ● Running                                         │
│  Uptime   2h 34m 12s                                        │
│  Socket   ~/.cache/rch/rch.sock                             │
│  PID      12345                                             │
╰─────────────────────────────────────────────────────────────╯

╭───────────────────────── Worker Fleet ──────────────────────╮
│ Worker       │ Status     │ Slots        │ Speed  │ Latency │
├──────────────┼────────────┼──────────────┼────────┼─────────┤
│ build-server │ ● Healthy  │ ████░░░░ 8/32│  94.2  │   12ms  │
│ fra-worker   │ ● Healthy  │ ██░░░░░░ 4/16│  87.1  │   45ms  │
│ backup       │ ◐ Degraded │ ░░░░░░░░ 0/8 │  62.3  │  120ms  │
╰─────────────────────────────────────────────────────────────╯

╭───────────────────────── Active Jobs ───────────────────────╮
│ Project          │ Command              │ Worker       │ Time │
├──────────────────┼──────────────────────┼──────────────┼──────┤
│ rch              │ cargo build --release│ build-server │  14s │
│ my-project       │ cargo test           │ fra-worker   │   3s │
╰─────────────────────────────────────────────────────────────╯
```

### A.2 `rch workers probe --all` Output

```
Probing 3 workers...

  build-server  ● OK     32 slots   rust 1.87-nightly   12ms
  fra-worker    ● OK     16 slots   rust 1.87-nightly   45ms
  backup        ✗ FAIL   Connection refused

╭─────────────────────── Probe Summary ───────────────────────╮
│  Total Workers   3                                          │
│  Online          2 (48 slots)                               │
│  Offline         1                                          │
╰─────────────────────────────────────────────────────────────╯
```

### A.3 Error Panel

```
╭─────────────────── ✗ Connection Failed ─────────────────────╮
│                                                              │
│  Could not reach worker 'build-server'                      │
│                                                              │
│    Host    203.0.113.10                                     │
│    Port    22                                               │
│    Error   Connection refused                               │
│                                                              │
│  💡 Check that the worker is running and SSH is configured  │
│                                                              │
╰──────────────────────────────────────────────────────────────╯
```

---

## Appendix B: Files to Create/Modify

### New Files

```
rch-common/src/ui/mod.rs
rch-common/src/ui/context.rs
rch-common/src/ui/theme.rs
rch-common/src/ui/icons.rs

rch/src/ui/mod.rs
rch/src/ui/console.rs
rch/src/ui/components/mod.rs
rch/src/ui/components/banner.rs
rch/src/ui/components/status_table.rs
rch/src/ui/components/progress.rs
rch/src/ui/components/error_panel.rs
rch/src/ui/components/worker_card.rs
rch/src/ui/formatters/mod.rs
rch/src/ui/formatters/worker.rs
rch/src/ui/formatters/job.rs
rch/src/ui/formatters/metrics.rs

rchd/src/ui/mod.rs
rchd/src/ui/banner.rs
rchd/src/ui/dashboard.rs
rchd/src/ui/log_formatter.rs
```

### Modified Files

```
Cargo.toml                    # Add rich_rust to workspace dependencies
rch/Cargo.toml               # Add rich_rust dependency
rchd/Cargo.toml              # Add rich_rust dependency
rch-common/Cargo.toml        # Add rich_rust dependency

rch/src/lib.rs               # Add ui module
rch/src/commands.rs          # Update commands to use rich output
rchd/src/main.rs             # Add startup banner
rchd/src/api.rs              # Add rich logging option
```

---

## Appendix C: Checklist

### Pre-Implementation

- [ ] Review this plan with stakeholders
- [ ] Verify rich_rust path/version
- [ ] Create feature branch
- [ ] Set up benchmarks baseline

### Phase 1: Foundation

- [ ] Add rich_rust dependency
- [ ] Create ui module structure
- [ ] Implement OutputContext
- [ ] Define RchTheme
- [ ] Add feature flag
- [ ] Write unit tests

### Phase 2: CLI Commands

- [ ] Implement `rch status` rich output
- [ ] Implement `rch workers list` rich output
- [ ] Implement `rch workers probe` with progress
- [ ] Implement `rch workers benchmark` with progress
- [ ] Ensure --json flag works correctly
- [ ] Write integration tests

### Phase 3: Daemon UI

- [ ] Implement startup banner
- [ ] Implement worker status messages
- [ ] Add rich logging option
- [ ] Test in service mode (no rich)

### Phase 4: Error Experience

- [ ] Implement ErrorDisplay component
- [ ] Add contextual error formatting
- [ ] Test error rendering
- [ ] Add help text integration

### Phase 5: Progress & Feedback

- [ ] Implement TransferProgress
- [ ] Add benchmark progress
- [ ] Add fleet installation progress
- [ ] Test progress bar rendering

### Phase 6: Polish

- [ ] Color consistency audit
- [ ] Accessibility review
- [ ] Update README
- [ ] Update AGENTS.md
- [ ] Add screenshots
- [ ] Performance validation
- [ ] Final testing

---

*This plan provides a comprehensive roadmap for integrating rich_rust into RCH while maintaining full compatibility with AI coding agents.*
