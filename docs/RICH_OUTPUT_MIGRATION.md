# Rich Output Migration Guide

This guide explains the changes introduced by the `rich_rust` integration in RCH and how to adapt your workflows if necessary.

## Overview

RCH now features "rich" terminal output, providing a more modern and informative user experience. This includes:
- **Colored Status**: Instant visual feedback on worker health (Green/Yellow/Red).
- **Tables & Panels**: Structured data display for `status`, `workers`, and `config` commands.
- **Progress Bars**: Real-time visualization for transfers and compilations.
- **Context Awareness**: Automatically adapts to your terminal capabilities (TTY, CI, pipe).

## Output Modes

RCH automatically selects the best output mode based on the environment:

| Mode | Description | Detected When |
|------|-------------|---------------|
| **Interactive** | Full rich output with colors, tables, and spinners. | `stderr` is a TTY (human usage). |
| **Hook** | Minimal, protocol-compliant JSON output for AI agents. | `stdin` receives JSON or `RCH_HOOK_MODE=1`. |
| **Machine** | Structured JSON output for scripts. | `--json` flag or `RCH_JSON=1`. |
| **Colored** | ANSI colors enabled, but no interactive widgets. | `FORCE_COLOR=1` (without TTY). |
| **Plain** | Pure text, no colors or formatting. | `NO_COLOR=1` or `FORCE_COLOR=0`. |

## For Scripts and Automation

If you parse RCH output in scripts, **you should use Machine mode**.

### Recommended: Use JSON
The `--json` flag (or `RCH_JSON=1`) guarantees stable, parseable output.

```bash
# Old way (fragile text parsing)
STATUS=$(rch status | grep "Daemon" | awk '{print $2}')

# New way (robust JSON parsing)
STATUS=$(rch status --json | jq -r '.data.daemon.status')
```

### Legacy Text Parsing
If you must parse human output, ensure you disable colors and formatting:

```bash
# Disable all decorations
export NO_COLOR=1
rch workers list | grep "cpu-worker"
```

## Environment Variables

| Variable | Effect |
|----------|--------|
| `RCH_JSON=1` | Forces JSON output (Machine mode). |
| `NO_COLOR=1` | Disables all colors and rich formatting (Plain mode). |
| `FORCE_COLOR=1` | Forces color output even if not a TTY. |
| `FORCE_COLOR=0` | Explicitly disables color (equivalent to `NO_COLOR=1`). |
| `RCH_HOOK_MODE=1` | Forces Hook mode (used internally by AI agents). |

## Troubleshooting

### Garbled Output?
If you see raw ANSI escape codes (e.g., `[32m✔[0m`) in logs or files:
- Set `NO_COLOR=1` when redirecting output if your viewer doesn't support ANSI.
- Check if your terminal `TERM` environment variable is set correctly (e.g., `xterm-256color`).

### Missing Colors?
If output is monochrome when it should be colored:
- Ensure `NO_COLOR` is NOT set.
- Try setting `FORCE_COLOR=1` to force color enablement.

### Hook Issues?
If Claude Code or other agents fail to recognize RCH output:
- Ensure `RCH_HOOK_MODE` is not accidentally set in your interactive shell.
- RCH automatically detects hook invocation, so manual configuration is rarely needed.

## Reverting to Plain Output

If you prefer the old, plain output style or encounter compatibility issues, you can disable rich output globally:

```bash
# Add to your shell profile (.bashrc, .zshrc)
export NO_COLOR=1
```

This will strip all colors, tables, and panels, reverting to a simple text format compatible with legacy parsing.
