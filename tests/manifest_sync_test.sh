#!/usr/bin/env bash
#
# ACFS manifest sync validation tests
#
# Verifies that `manifest_index.sh` matches the current `.claude/skills/**` tree:
# - File list is complete (no missing/extra entries)
# - SHA256 checksums are correct
# - `manifest_index.sh --verify` succeeds on a clean tree
# - Drift is detected (content change / manifest drift)
#
# Run: ./tests/manifest_sync_test.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

LOG_PREFIX="[manifest-sync]"

log() {
  printf '%s %s\n' "$LOG_PREFIX" "$*"
}

die() {
  printf '%s ERROR: %s\n' "$LOG_PREFIX" "$*" >&2
  exit 1
}

pass() {
  printf '%s PASS: %s\n' "$LOG_PREFIX" "$*"
}

require_file() {
  local path="$1"
  [[ -f "$path" ]] || die "missing required file: $path"
}

require_cmd() {
  local cmd="$1"
  command -v "$cmd" >/dev/null 2>&1 || die "missing required command: $cmd"
}

sha256_file() {
  local path="$1"

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$path" | awk '{print $1}'
    return 0
  fi

  # macOS fallback (not used by manifest_index.sh today, but helpful for local validation)
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$path" | awk '{print $1}'
    return 0
  fi

  die "no SHA256 tool found (need sha256sum or shasum)"
}

manifest_print() {
  bash ./manifest_index.sh --print
}

generate_manifest() {
  require_cmd awk
  require_cmd sort

  # Use `command find` to avoid interactive shell aliases (e.g., find -> fd).
  command find .claude/skills -type f -print | LC_ALL=C sort | while IFS= read -r path; do
    local hash
    hash="$(sha256_file "$path")"
    printf '%s  %s\n' "$hash" "$path"
  done
}

assert_manifest_format() {
  local line_count
  line_count="$(manifest_print | wc -l | tr -d ' ')"
  [[ "$line_count" -gt 0 ]] || die "manifest_index.sh --print produced no lines"

  # Format: "<64 hex>  <path>"
  if ! manifest_print | awk '
    BEGIN { ok = 1 }
    /^[0-9a-f]{64}  [^[:space:]].+$/ { next }
    { ok = 0; print "BAD LINE: " $0 > "/dev/stderr" }
    END { exit ok ? 0 : 1 }
  '; then
    die "manifest_index.sh --print output format invalid"
  fi

  pass "manifest_index.sh --print format ok (${line_count} entries)"
}

assert_manifest_matches_tree() {
  require_cmd diff

  local tmpdir expected_file actual_file
  tmpdir="$(mktemp -d)"
  expected_file="${tmpdir}/expected_manifest.txt"
  actual_file="${tmpdir}/actual_manifest.txt"

  generate_manifest >"$expected_file"
  manifest_print >"$actual_file"

  if ! diff -u "$expected_file" "$actual_file" >/dev/null; then
    log "manifest mismatch (showing diff):"
    diff -u "$expected_file" "$actual_file" || true
    return 1
  fi

  return 0
}

assert_verify_passes() {
  require_cmd sha256sum
  bash ./manifest_index.sh --verify >/dev/null
}

copy_workspace_to_tmp() {
  local tmpdir="$1"
  mkdir -p "$tmpdir"
  cp -a ./manifest_index.sh "$tmpdir/"
  cp -a ./.claude "$tmpdir/"
}

inject_manifest_entry() {
  local script_path="$1"
  local entry_line="$2"

  local tmp_out
  tmp_out="$(mktemp)"

  awk -v entry="$entry_line" '
    { print }
    $0 == "MANIFEST_EOF" { print entry }
  ' "$script_path" >"$tmp_out"

  # Replace in-place (tmp workspace only)
  cp "$tmp_out" "$script_path"
}

test_repo_tree_matches_manifest() {
  log "Test: repo tree matches manifest_index.sh --print"
  if ! assert_manifest_matches_tree; then
    die "manifest_index.sh does not match current .claude/skills tree"
  fi
  pass "repo tree matches manifest_index.sh"
}

test_manifest_verify_passes() {
  log "Test: manifest_index.sh --verify passes on clean tree"
  if ! assert_verify_passes; then
    die "manifest_index.sh --verify failed on clean tree"
  fi
  pass "manifest_index.sh --verify passes"
}

test_detects_content_drift() {
  log "Test: --verify detects content drift"

  local tmpdir
  tmpdir="$(mktemp -d)"
  copy_workspace_to_tmp "$tmpdir"

  (
    cd "$tmpdir"
    bash ./manifest_index.sh --verify >/dev/null
    printf '\n# drift\n' >>.claude/skills/rch/SKILL.md
    if bash ./manifest_index.sh --verify >/dev/null 2>&1; then
      die "expected verify to fail after content drift, but it succeeded (tmpdir=$tmpdir)"
    fi
  )

  pass "--verify fails after content drift (as expected)"
}

test_detects_manifest_missing_file() {
  log "Test: --verify detects manifest referencing missing file"

  local tmpdir
  tmpdir="$(mktemp -d)"
  copy_workspace_to_tmp "$tmpdir"

  (
    cd "$tmpdir"
    inject_manifest_entry ./manifest_index.sh \
      "0000000000000000000000000000000000000000000000000000000000000000  .claude/skills/rch/DOES_NOT_EXIST.md"
    if bash ./manifest_index.sh --verify >/dev/null 2>&1; then
      die "expected verify to fail when manifest references missing file, but it succeeded (tmpdir=$tmpdir)"
    fi
  )

  pass "--verify fails when manifest references missing file (as expected)"
}

test_detects_extra_file_list_drift() {
  log "Test: strict tree-vs-manifest check detects extra files"

  local tmpdir
  tmpdir="$(mktemp -d)"
  copy_workspace_to_tmp "$tmpdir"

  (
    cd "$tmpdir"
    printf 'extra\n' >.claude/skills/rch/EXTRA_FILE.md
    if assert_manifest_matches_tree; then
      die "expected strict tree-vs-manifest check to fail with extra file, but it succeeded (tmpdir=$tmpdir)"
    fi
  )

  pass "extra files are detected by strict tree-vs-manifest check (as expected)"
}

main() {
  require_file ./manifest_index.sh
  require_file ./.claude/skills/rch/SKILL.md
  require_cmd bash

  log "Running ACFS manifest sync validation tests..."

  assert_manifest_format
  test_repo_tree_matches_manifest
  test_manifest_verify_passes
  test_detects_content_drift
  test_detects_manifest_missing_file
  test_detects_extra_file_list_drift

  pass "all manifest sync tests passed"
}

main "$@"

