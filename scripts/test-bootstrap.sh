#!/usr/bin/env bash
#
# test-bootstrap.sh - ACFS bootstrap verification (end-to-end)
#
# This script validates that a *fresh clone* of the repo contains a valid
# `manifest_index.sh` and that every referenced skill file exists.
#
# It is intentionally self-contained and CI-friendly: all work happens in a
# temporary directory, and the clone is cleaned up on exit (unless --keep-tmp).
#
# Usage:
#   ./scripts/test-bootstrap.sh
#   ./scripts/test-bootstrap.sh --repo <url-or-path> [--ref <git-ref>] [--keep-tmp]
#
# Environment overrides:
#   RCH_BOOTSTRAP_REPO=<url-or-path>
#   RCH_BOOTSTRAP_REF=<git-ref>
#   RCH_BOOTSTRAP_KEEP_TMP=1
#   RCH_BOOTSTRAP_TMP_ROOT=/tmp
#
# Exit codes:
#   0 - pass
#   1 - fail
#   2 - usage error
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# shellcheck source=lib/e2e_common.sh
source "$SCRIPT_DIR/lib/e2e_common.sh"

LOG_PREFIX="[bootstrap]"

BOOTSTRAP_TMPDIR=""
BOOTSTRAP_KEEP_TMP=""

log() {
  printf '%s %s\n' "$LOG_PREFIX" "$*"
}

die() {
  printf '%s ERROR: %s\n' "$LOG_PREFIX" "$*" >&2
  exit 1
}

usage() {
  cat <<'EOF'
Usage: ./scripts/test-bootstrap.sh [options]

Options:
  --repo <url-or-path>   Repo to clone (default: current repo path)
  --ref <git-ref>        Git ref to checkout (branch/tag/sha)
  --tmp-root <dir>       Where to create temp dir (default: mktemp default)
  --keep-tmp             Keep temp dir for debugging
  -h, --help             Show help
EOF
}

require_cmd() {
  local cmd="$1"
  command -v "$cmd" >/dev/null 2>&1 || die "missing required command: $cmd"
}

require_file() {
  local path="$1"
  [[ -f "$path" ]] || die "missing required file: $path"
}

hash_verify_cmd() {
  if command -v sha256sum >/dev/null 2>&1; then
    echo "sha256sum -c -"
    return 0
  fi

  # macOS fallback
  if command -v shasum >/dev/null 2>&1; then
    echo "shasum -a 256 -c -"
    return 0
  fi

  die "no SHA256 verify tool found (need sha256sum or shasum)"
}

manifest_print() {
  bash ./manifest_index.sh --print
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

  log "PASS: manifest format ok (${line_count} entries)"
}

assert_manifest_files_exist() {
  local missing=0

  while IFS= read -r line; do
    [[ -n "$line" ]] || continue
    local path
    path="$(printf '%s\n' "$line" | awk '{print $2}')"

    # Best-effort guard: all entries should be under .claude/skills/
    case "$path" in
      .claude/skills/*) ;;
      *)
        log "FAIL: manifest entry not under .claude/skills/: $path"
        missing=$((missing + 1))
        continue
        ;;
    esac

    if [[ ! -f "$path" ]]; then
      log "FAIL: missing file referenced by manifest: $path"
      missing=$((missing + 1))
    fi
  done < <(manifest_print)

  [[ "$missing" -eq 0 ]] || die "manifest references ${missing} missing/invalid path(s)"
  log "PASS: all manifest referenced files exist"
}

assert_manifest_verify_passes() {
  local verify
  verify="$(hash_verify_cmd)"

  if ! manifest_print | eval "$verify" >/dev/null; then
    die "manifest checksum verification failed"
  fi

  log "PASS: manifest checksum verification ok"
}

run_manifest_sync_tests_if_available() {
  if [[ -x ./tests/manifest_sync_test.sh ]]; then
    log "Running ./tests/manifest_sync_test.sh ..."
    if bash ./tests/manifest_sync_test.sh; then
      log "PASS: manifest_sync_test.sh"
    else
      die "manifest_sync_test.sh failed"
    fi
  else
    log "SKIP: ./tests/manifest_sync_test.sh not present"
  fi
}

main() {
  local repo="${RCH_BOOTSTRAP_REPO:-}"
  local ref="${RCH_BOOTSTRAP_REF:-}"
  BOOTSTRAP_KEEP_TMP="${RCH_BOOTSTRAP_KEEP_TMP:-}"
  local tmp_root="${RCH_BOOTSTRAP_TMP_ROOT:-}"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --repo)
        repo="${2:-}"
        shift 2
        ;;
      --ref)
        ref="${2:-}"
        shift 2
        ;;
      --tmp-root)
        tmp_root="${2:-}"
        shift 2
        ;;
      --keep-tmp)
        BOOTSTRAP_KEEP_TMP="1"
        shift
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        usage >&2
        exit 2
        ;;
    esac
  done

  [[ -n "$repo" ]] || repo="$PROJECT_ROOT"

  require_cmd bash
  require_cmd git
  require_cmd awk
  require_cmd wc

  local tmpdir
  if [[ -n "$tmp_root" ]]; then
    mkdir -p "$tmp_root"
    tmpdir="$(mktemp -d "${tmp_root%/}/rch-bootstrap-XXXXXX")"
  else
    tmpdir="$(mktemp -d)"
  fi
  BOOTSTRAP_TMPDIR="$tmpdir"

  cleanup() {
    local exit_code=$?
    if [[ -n "$BOOTSTRAP_KEEP_TMP" ]]; then
      log "Keeping temp dir: $BOOTSTRAP_TMPDIR (exit=$exit_code)"
      return
    fi
    rm -rf "$BOOTSTRAP_TMPDIR"
  }
  trap cleanup EXIT

  log "Temp dir: $tmpdir"
  log "Cloning repo: $repo"

  local clone_dir="$tmpdir/repo"
  git clone "$repo" "$clone_dir" >/dev/null 2>&1 || die "git clone failed"

  (
    cd "$clone_dir"

    if [[ -n "$ref" ]]; then
      log "Checking out ref: $ref"
      git checkout "$ref" >/dev/null 2>&1 || die "git checkout $ref failed"
    fi

    require_file ./manifest_index.sh
    require_file ./.claude/skills/rch/SKILL.md

    assert_manifest_format
    assert_manifest_files_exist
    assert_manifest_verify_passes
    run_manifest_sync_tests_if_available

    log "PASS: bootstrap verification complete"
  )
}

main "$@"
