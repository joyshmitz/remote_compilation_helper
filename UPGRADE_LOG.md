# Dependency Upgrade Log

**Date:** 2026-01-25  |  **Project:** remote_compilation_helper  |  **Language:** Rust

## Summary
- **Updated:** 5  |  **Skipped:** 0  |  **Failed:** 0  |  **Needs attention:** 0

## Updates

### whoami: 1.5 → 2.0 (rch-common, rchd dev-dependency)
- **Breaking:** `username()` now returns `Result<String, Error>` instead of `String`
- **Migration:** Added `.unwrap_or_else(|_| "unknown".to_string())` to all call sites
- **Files changed:**
  - `rch-common/src/e2e/fixtures.rs:25`
  - `rch-common/src/ssh.rs:141`
  - `rchd/tests/e2e_self_test.rs:65`
  - `rchd/Cargo.toml` (dev-dependency version)
- **Tests:** ✓ Passed

### criterion: 0.5 → 0.8 (rch-common dev-dependency)
- **Breaking:** None observed
- **Tests:** ✓ Passed

### colored: 2 → 3 (rch)
- **Breaking:** MSRV bump to 1.80, `lazy_static` dependency removed
- **Migration:** None required - API compatible
- **Tests:** ✓ Passed

### rusqlite: 0.32 → 0.38 (rch-telemetry)
- **Breaking:** `u64`/`usize` no longer implement `FromSql` by default
- **Migration:** Changed `let total: u64` to `let total: i64` then cast to u64
- **Files changed:**
  - `rch-telemetry/src/storage/mod.rs:277-281`
- **Tests:** ✓ Passed

### reqwest: 0.12 → 0.13 (rch)
- **Breaking:** Default TLS changed to rustls, `query`/`form` features now optional
- **Migration:** None required - using `json` feature which is still supported
- **Notes:** Now using rustls as TLS backend by default (was native-tls)
- **Tests:** ✓ Passed

## Skipped

None

## Failed

None

## SQLite Improvements (rust-cli-with-sqlite skill)

Applied best practices from the rust-cli-with-sqlite skill to rch-telemetry:

### Enhanced SQLite Configuration
- **File:** `rch-telemetry/src/storage/mod.rs`
- Added `PRAGMA wal_autocheckpoint=1000` for controlled WAL growth
- Added `PRAGMA foreign_keys=ON` for referential integrity
- Added `busy_timeout(5 seconds)` for concurrent access handling
- Already had: `journal_mode=WAL`, `synchronous=NORMAL`

### Added Database Diagnostics
- **File:** `rch-telemetry/src/storage/mod.rs`
- Added `integrity_check()` method using `PRAGMA integrity_check`
- Added `stats()` method returning `StorageStats` struct with:
  - telemetry_snapshots count
  - hourly_aggregates count
  - speedscore_entries count
  - test_runs count
  - db_size_bytes
- Exported `StorageStats` from `rch-telemetry/src/lib.rs`

### Integrated with Doctor Command
- **File:** `rch/src/doctor.rs`
- Added new `check_telemetry_database()` check to `rch doctor`
- Checks: database existence, integrity, and optionally stats (in verbose mode)
- Provides actionable suggestions for corrupted databases

## Notes

- All workspace members now build and pass tests
- Rust Edition 2024 with nightly-2025-01-01 toolchain
- Full test suite: 780+ tests passing
