//! README regression tests.
//!
//! Bead reference: bd-1gu1 (Tests: README quick-install + diagram presence)

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().unwrap_or(&manifest_dir).to_path_buf()
}

#[test]
fn readme_contains_quick_install_and_diagram() {
    let readme_path = repo_root().join("README.md");
    let readme = std::fs::read_to_string(&readme_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", readme_path.display()));

    let required = [
        (
            "easy_mode_curl_one_liner",
            r#"curl -fsSL "https://raw.githubusercontent.com/Dicklesworthstone/remote_compilation_helper/main/install.sh?$(date +%s)" | bash -s -- --easy-mode"#,
        ),
        ("rch_diagram_webp_reference", "rch_diagram.webp"),
    ];

    let mut missing = Vec::new();
    for (name, snippet) in required {
        if !readme.contains(snippet) {
            missing.push(format!("{name} (expected to contain: {snippet})"));
        }
    }

    assert!(
        missing.is_empty(),
        "README regression: missing snippet(s) in {}: {}",
        readme_path.display(),
        missing.join("; ")
    );
}
