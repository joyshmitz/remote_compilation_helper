# Dependency Upgrade Log

**Date:** 2026-01-18  |  **Project:** remote_compilation_helper  |  **Language:** Rust

## Summary
- **Updated:** 11  |  **Skipped:** 2  |  **Failed:** 0  |  **Needs attention:** 6

## Environment
- **Rust:** 1.85.0-nightly (2024-12-31)
- **Toolchain:** nightly-2025-01-01

---

## Updates

### console: 0.15.11 → 0.16.2
- **Breaking:** None
- **Tests:** Passed

### dialoguer: 0.11.0 → 0.12.0
- **Breaking:** None
- **Tests:** Passed

### pulldown-cmark: 0.12.2 → 0.13.0
- **Breaking:** None
- **Tests:** Passed

### terminal_size: 0.3.0 → 0.4.3
- **Breaking:** None
- **Tests:** Passed

### indicatif: 0.17.11 → 0.18.3
- **Breaking:** None
- **Tests:** Passed

### object: 0.36.7 → 0.38.1
- **Breaking:** None
- **Tests:** Passed

### prometheus: 0.13.4 → 0.14.0
- **Breaking:** `with_label_values()` signature changed; `get_name()` deprecated
- **Migration:** Updated `inc_build_classification()` to use proper string lifetimes; replaced `get_name()` with `name()`
- **Tests:** Passed after fix

### toml: 0.8.23 → 0.9.11
- **Breaking:** None
- **Tests:** Passed

### cron: 0.12.1 → 0.15.0
- **Breaking:** None
- **Tests:** Passed

### tokio-cron-scheduler: 0.11.1 → 0.15.1
- **Breaking:** None
- **Tests:** Passed

### which: 7.0.3 → 8.0.0
- **Breaking:** None detected
- **Tests:** Passed

---

## Skipped

### ratatui: 0.29.0 → 0.30.0
- **Reason:** Requires Rust 1.86.0, project uses 1.85.0-nightly
- **Action:** Keep at 0.29.0 until toolchain is updated

### unicode-width: 0.2.0 → 0.2.2
- **Reason:** Conflicts with ratatui 0.29.0 which requires unicode-width =0.2.0
- **Action:** Will be updated when ratatui is upgraded

---

## Needs Attention

### opentelemetry: 0.27.1 → 0.31.0
- **Issue:** Major version jump with breaking API changes expected
- **Migration guide:** Review OpenTelemetry Rust changelog

### opentelemetry-otlp: 0.27.0 → 0.31.0
- **Issue:** Must update together with opentelemetry suite
- **Note:** Coordinate with opentelemetry update

### opentelemetry_sdk: 0.27.1 → 0.31.0
- **Issue:** Must update together with opentelemetry suite
- **Note:** Coordinate with opentelemetry update

### tracing-opentelemetry: 0.28.0 → 0.32.1
- **Issue:** Must update together with opentelemetry suite
- **Note:** Coordinate with opentelemetry update

### rusqlite: 0.32.1 → 0.38.0
- **Issue:** Major version jump, potential API changes
- **Action:** Research breaking changes before attempting

### reqwest: 0.12.28 → 0.13.1
- **Issue:** Major version jump, potential API changes
- **Action:** Research breaking changes before attempting

---

## Dev Dependencies Not Updated

| Package | Current | Latest | Notes |
|---------|---------|--------|-------|
| criterion | 0.5.1 | 0.7.0 | Benchmark framework, non-critical |
| colored | 2.2.0 | 3.1.1 | Only used in rch/Cargo.toml |
| whoami | 1.6.1 → 2.0.2 | 2.0.2 | Dev dependency in rchd |
