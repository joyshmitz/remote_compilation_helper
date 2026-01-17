# Testing Guide

This guide covers how to run, write, and debug tests in the RCH workspace.

## Quick Start

```bash
# Run all unit tests (library tests)
cargo test --workspace --lib

# Run all tests (unit + integration)
cargo test --workspace

# Run with logging output
RUST_LOG=debug cargo test test_name -- --nocapture
```

## Test Organization

- Unit tests: `src/*.rs` modules (`#[test]` and `#[tokio::test]`)
- Integration tests: crate-level `tests/` directories
  - `rch/tests/`
  - `rchd/tests/`
- E2E tests: `rchd/tests/e2e_*.rs` and `rch/tests/e2e_*.rs`
- Scripted E2E: `scripts/e2e_test.sh` (supports `RCH_MOCK_SSH=1`)

## Test Logging Standards

Use structured logging for clarity and postmortems:

```rust
info!("TEST: {}", test_name);
info!("INPUT: {:?}", input);
info!("EXPECTED: {:?}", expected);
info!("ACTUAL: {:?}", actual);
info!("PASS/FAIL: {}", result);
```

Keep logs concise, consistent, and aligned with assertions.

## Writing New Tests

### Unit Tests

- Prefer `#[test]` for sync code and `#[tokio::test]` for async paths.
- Use clear names: `test_<module>_<behavior>`.

### Integration & E2E Tests

RCH ships an E2E harness in `rch-common`:

```rust
use rch_common::e2e::e2e_test;

e2e_test!(test_basic_hook, |harness| {
    // Arrange
    let input = harness.hook_input("cargo build");

    // Act
    let output = harness.run_hook(input)?;

    // Assert
    harness.assert_allowed(&output)?;
    Ok(())
});
```

See `rchd/tests/e2e_*.rs` for real examples.

## Coverage

```bash
cargo llvm-cov --workspace --html
open target/llvm-cov/html/index.html
```

Notes:
- Install `cargo-llvm-cov` if missing: `cargo install cargo-llvm-cov`.
- Coverage can be slow on large workspaces.

## Debugging Failures

```bash
# Single test with full output
RUST_LOG=trace cargo test test_name -- --nocapture --test-threads=1

# Run just one crate
cargo test -p rch
cargo test -p rchd
cargo test -p rch-common
```

## CI/CD Integration

- All tests run on PRs (unit + integration).
- E2E tests use mock workers when possible.
- Coverage reports are generated and uploaded in CI.
