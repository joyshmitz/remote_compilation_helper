//! Fixture validation: Rust project with build.rs
//!
//! This test module validates that the `with_build_rs` fixture is well-formed:
//! - `cargo build` succeeds (build.rs runs and generates OUT_DIR code)
//! - the generated file exists under target/debug/build/*/out/generated.rs
//! - `cargo run` output confirms the generated symbols are linked
//! - `cargo test` passes within the fixture itself

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

#[test]
fn test_build_rs_fixture_compiles() {
    let fixture_path = fixture_dir();
    let output = Command::new("cargo")
        .args(["build"])
        .current_dir(&fixture_path)
        .output()
        .expect("cargo build failed to start");

    assert!(
        output.status.success(),
        "Fixture build failed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_build_rs_generates_code() {
    let fixture_path = fixture_dir();

    // Build first (ignore output here; compile test asserts separately).
    let _ = Command::new("cargo")
        .args(["build"])
        .current_dir(&fixture_path)
        .output();

    let build_dir = fixture_path.join("target/debug/build");
    assert!(
        build_dir.exists(),
        "Build directory should exist after build: {}",
        build_dir.display()
    );

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
}

#[test]
fn test_build_rs_binary_output() {
    let fixture_path = fixture_dir();
    let output = Command::new("cargo")
        .args(["run"])
        .current_dir(&fixture_path)
        .output()
        .expect("cargo run failed to start");

    assert!(
        output.status.success(),
        "Fixture run failed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("build.rs ran: true"),
        "Output should confirm build.rs ran"
    );
    assert!(
        stdout.contains("Hello from build.rs generated code!"),
        "Output should include generated greeting"
    );
}

#[test]
fn test_build_rs_tests_pass() {
    let fixture_path = fixture_dir();
    let output = Command::new("cargo")
        .args(["test"])
        .current_dir(&fixture_path)
        .output()
        .expect("cargo test failed to start");

    assert!(
        output.status.success(),
        "Fixture tests failed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
