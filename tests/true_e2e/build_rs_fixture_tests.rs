//! Fixture validation: Rust project with build.rs
//!
//! This test module validates that the `with_build_rs` fixture is well-formed:
//! - `cargo build` succeeds (build.rs runs and generates OUT_DIR code)
//! - the generated file exists under target/debug/build/*/out/generated.rs
//! - `cargo run` output confirms the generated symbols are linked
//! - `cargo test` passes within the fixture itself

use rch_common::e2e::{LogLevel, LogSource, TestLoggerBuilder};
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    manifest_dir
        .parent()
        .map(PathBuf::from)
        .unwrap_or(manifest_dir)
}

fn fixture_dir() -> PathBuf {
    repo_root().join("tests/true_e2e/fixtures/with_build_rs")
}

fn new_logger(test_name: &str) -> rch_common::e2e::TestLogger {
    TestLoggerBuilder::new(test_name)
        .print_realtime(true)
        .build()
}

#[test]
fn test_build_rs_fixture_compiles() {
    let logger = new_logger("test_build_rs_fixture_compiles");
    logger.info("TEST START: test_build_rs_fixture_compiles");

    let fixture_path = fixture_dir();
    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("test".to_string()),
        "Running cargo build",
        vec![
            ("phase".to_string(), "execute".to_string()),
            ("cwd".to_string(), fixture_path.display().to_string()),
            ("cmd".to_string(), "cargo build".to_string()),
        ],
    );

    let output = match Command::new("cargo")
        .args(["build"])
        .current_dir(&fixture_path)
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            logger.error(format!("cargo build failed to start: {e}"));
            panic!("cargo build failed to start: {e}");
        }
    };

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        logger.error("Fixture build failed");
        logger.error(format!("stdout:\n{stdout}"));
        logger.error(format!("stderr:\n{stderr}"));
        panic!("Fixture build failed.\nstdout:\n{stdout}\nstderr:\n{stderr}");
    }

    logger.info("TEST PASS: test_build_rs_fixture_compiles");
}

#[test]
fn test_build_rs_generates_code() {
    let logger = new_logger("test_build_rs_generates_code");
    logger.info("TEST START: test_build_rs_generates_code");

    let fixture_path = fixture_dir();

    // Build first (this test is standalone; validate build succeeded).
    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("test".to_string()),
        "Running cargo build",
        vec![
            ("phase".to_string(), "execute".to_string()),
            ("cwd".to_string(), fixture_path.display().to_string()),
            ("cmd".to_string(), "cargo build".to_string()),
        ],
    );

    let output = match Command::new("cargo")
        .args(["build"])
        .current_dir(&fixture_path)
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            logger.error(format!("cargo build failed to start: {e}"));
            panic!("cargo build failed to start: {e}");
        }
    };

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        logger.error("Fixture build failed");
        logger.error(format!("stdout:\n{stdout}"));
        logger.error(format!("stderr:\n{stderr}"));
        panic!("Fixture build failed.\nstdout:\n{stdout}\nstderr:\n{stderr}");
    }

    let build_dir = fixture_path.join("target/debug/build");
    if !build_dir.exists() {
        logger.error(format!(
            "Build directory should exist after build: {}",
            build_dir.display()
        ));
        panic!(
            "Build directory should exist after build: {}",
            build_dir.display()
        );
    }

    let mut found = None;
    for entry in std::fs::read_dir(&build_dir).expect("read_dir on target/debug/build failed") {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let candidate = path.join("out/generated.rs");
        if candidate.exists() {
            found = Some(candidate);
            break;
        }
    }

    let Some(generated) = found else {
        panic!(
            "generated.rs should exist under OUT_DIR (target/debug/build/*/out/generated.rs) for fixture {}",
            fixture_path.display()
        );
    };

    let contents = std::fs::read_to_string(&generated).expect("Failed to read generated.rs");
    assert!(
        contents.contains("BUILD_RS_RAN"),
        "generated.rs should contain BUILD_RS_RAN marker"
    );

    logger.info("TEST PASS: test_build_rs_generates_code");
}

#[test]
fn test_build_rs_binary_output() {
    let logger = new_logger("test_build_rs_binary_output");
    logger.info("TEST START: test_build_rs_binary_output");

    let fixture_path = fixture_dir();
    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("test".to_string()),
        "Running cargo run",
        vec![
            ("phase".to_string(), "execute".to_string()),
            ("cwd".to_string(), fixture_path.display().to_string()),
            ("cmd".to_string(), "cargo run".to_string()),
        ],
    );

    let output = match Command::new("cargo")
        .args(["run"])
        .current_dir(&fixture_path)
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            logger.error(format!("cargo run failed to start: {e}"));
            panic!("cargo run failed to start: {e}");
        }
    };

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        logger.error("Fixture run failed");
        logger.error(format!("stdout:\n{stdout}"));
        logger.error(format!("stderr:\n{stderr}"));
        panic!("Fixture run failed.\nstdout:\n{stdout}\nstderr:\n{stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("build.rs ran: true"),
        "Output should confirm build.rs ran"
    );
    assert!(
        stdout.contains("Hello from build.rs generated code!"),
        "Output should include generated greeting"
    );

    logger.info("TEST PASS: test_build_rs_binary_output");
}

#[test]
fn test_build_rs_tests_pass() {
    let logger = new_logger("test_build_rs_tests_pass");
    logger.info("TEST START: test_build_rs_tests_pass");

    let fixture_path = fixture_dir();
    logger.log_with_context(
        LogLevel::Info,
        LogSource::Custom("test".to_string()),
        "Running cargo test",
        vec![
            ("phase".to_string(), "execute".to_string()),
            ("cwd".to_string(), fixture_path.display().to_string()),
            ("cmd".to_string(), "cargo test".to_string()),
        ],
    );

    let output = match Command::new("cargo")
        .args(["test"])
        .current_dir(&fixture_path)
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            logger.error(format!("cargo test failed to start: {e}"));
            panic!("cargo test failed to start: {e}");
        }
    };

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        logger.error("Fixture tests failed");
        logger.error(format!("stdout:\n{stdout}"));
        logger.error(format!("stderr:\n{stderr}"));
        panic!("Fixture tests failed.\nstdout:\n{stdout}\nstderr:\n{stderr}");
    }

    logger.info("TEST PASS: test_build_rs_tests_pass");
}
