#!/usr/bin/env bash
#
# e2e_bd-3i0q.sh - Project-local .rchignore excludes
#
# Verifies:
# - `rch diagnose` detects a project-local `.rchignore` file
# - The reported exclude breakdown includes the expected `.rchignore` count
# - Behavior is stable without `.rchignore`
#
# Notes:
# - This script avoids cleanup commands (no rm -rf) by leaving temp dirs behind.
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
LOG_FILE="${PROJECT_ROOT}/target/e2e_bd-3i0q.jsonl"

timestamp() {
    date -u '+%Y-%m-%dT%H:%M:%S.%3NZ' 2>/dev/null || date -u '+%Y-%m-%dT%H:%M:%SZ'
}

log_json() {
    local phase="$1"
    local message="$2"
    local extra="${3:-}"
    if [[ -z "$extra" ]]; then
        extra='{}'
    fi
    local ts
    ts="$(timestamp)"
    jq -nc \
        --arg ts "$ts" \
        --arg test "bd-3i0q" \
        --arg phase "$phase" \
        --arg message "$message" \
        --argjson extra "$extra" \
        '{ts:$ts,test:$test,phase:$phase,message:$message} + $extra' \
        | tee -a "$LOG_FILE"
}

die() {
    log_json "error" "$*" '{"result":"fail"}'
    exit 1
}

check_dependencies() {
    log_json "setup" "Checking dependencies"
    for cmd in cargo jq; do
        command -v "$cmd" >/dev/null 2>&1 || die "Missing dependency: $cmd"
    done
}

build_rch() {
    local rch_bin="${PROJECT_ROOT}/target/debug/rch"
    if [[ -x "$rch_bin" ]]; then
        log_json "setup" "Using existing rch binary" "{\"path\":\"$rch_bin\"}" >/dev/null
        echo "$rch_bin"
        return
    fi
    log_json "setup" "Building rch (debug)" >/dev/null
    (cd "$PROJECT_ROOT" && cargo build -p rch >/dev/null 2>&1) || die "cargo build failed"
    [[ -x "$rch_bin" ]] || die "rch binary missing after build"
    echo "$rch_bin"
}

test_no_rchignore() {
    log_json "test" "Diagnose reports no .rchignore when absent"
    local rch_bin
    rch_bin="$(build_rch)"

    local tmp_dir
    tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/rchignore-none-XXXXXX")"

    local output
    output="$(cd "$tmp_dir" && "$rch_bin" diagnose "cargo build" 2>/dev/null)"

    if echo "$output" | grep -qE "^\\s*\\.rchignore:" && echo "$output" | grep -q "not found"; then
        log_json "verify" "No .rchignore detected as expected" '{"result":"pass"}'
    else
        die "Expected diagnose output to report .rchignore not found"
    fi
}

test_rchignore_detected() {
    log_json "test" "Diagnose detects .rchignore and reports pattern count"
    local rch_bin
    rch_bin="$(build_rch)"

    local tmp_dir
    tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/rchignore-present-XXXXXX")"

    cat > "$tmp_dir/.rchignore" <<EOF
large_data/
secrets/
EOF

    local output
    output="$(cd "$tmp_dir" && "$rch_bin" diagnose "cargo build" 2>/dev/null)"

    if ! echo "$output" | grep -qE "^\\s*\\.rchignore:"; then
        die "Expected diagnose output to include .rchignore section"
    fi
    if ! echo "$output" | grep -q "detected"; then
        die "Expected diagnose output to report .rchignore detected"
    fi
    if ! echo "$output" | grep -qE ",\\s*2 from \\.rchignore\\)"; then
        die "Expected exclude breakdown to include '2 from .rchignore'"
    fi

    log_json "verify" "Diagnose reports .rchignore detected with expected count" '{"result":"pass","rchignore_patterns":2}'
}

main() {
    : > "$LOG_FILE"
    check_dependencies

    log_json "setup" "Starting bd-3i0q E2E tests"
    test_no_rchignore
    test_rchignore_detected
    log_json "summary" "All bd-3i0q checks passed" '{"result":"pass","tests_run":2}'
}

main "$@"
