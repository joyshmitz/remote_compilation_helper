#!/usr/bin/env bash
# scripts/test-installer-linux.sh
#
# bd-1ck8: Linux installer verification script.
# This script downloads the upstream installer, runs it in an isolated HOME, and
# validates that the installed binaries execute correctly.
#
# Usage:
#   ./scripts/test-installer-linux.sh
#
# Environment overrides:
#   INSTALLER_URL        Override installer URL (default: GitHub raw main/install.sh)
#   KEEP_TMP=1           Keep temporary directory for debugging (no cleanup)
#   EXTRA_INSTALL_ARGS   Extra args passed to install.sh (space-separated)
#   LOG_DIR              Where to write JSONL log (default: ./target)
#
# Notes:
# - The install is isolated via HOME + RCH_INSTALL_DIR + RCH_CONFIG_DIR.
# - The installer is run with --no-service and RCH_NO_HOOK=1 to avoid touching
#   system services and Claude Code hooks during verification.
#
# Exit codes:
#   0 - success
#   1 - failure

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Source common helpers if available.
if [[ -f "${SCRIPT_DIR}/lib/e2e_common.sh" ]]; then
    # shellcheck disable=SC1091
    source "${SCRIPT_DIR}/lib/e2e_common.sh"
fi

TEST_NAME="installer_linux"

log_dir="${LOG_DIR:-${PROJECT_ROOT}/target}"
mkdir -p "$log_dir"
log_file="${log_dir}/${TEST_NAME}_$(date -u '+%Y%m%dT%H%M%SZ').jsonl"

log_json() {
    local phase="$1"
    local message="$2"
    local extra="${3:-{}}"
    local ts
    ts="$(date -u '+%Y-%m-%dT%H:%M:%S.%3NZ' 2>/dev/null || date -u '+%Y-%m-%dT%H:%M:%SZ')"
    printf '{"ts":"%s","test":"%s","phase":"%s","message":"%s",%s}\n' \
        "$ts" \
        "$TEST_NAME" \
        "$phase" \
        "${message//\"/\\\"}" \
        "${extra#\{}" | sed 's/,}$/}/' | tee -a "$log_file"
}

die() {
    log_json "error" "$1" '{"result":"fail"}'
    echo "[ERROR] $1" >&2
    exit 1
}

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "Missing required command: $1"
}

install_url_default="https://raw.githubusercontent.com/Dicklesworthstone/remote_compilation_helper/main/install.sh"
install_url="${INSTALLER_URL:-$install_url_default}"

tmp_root="$(mktemp -d -t rch-installer-linux.XXXXXX)"
tmp_home="${tmp_root}/home"
tmp_bin="${tmp_root}/bin"
tmp_config="${tmp_root}/config"
installer_path="${tmp_root}/install.sh"

cleanup() {
    if [[ "${KEEP_TMP:-0}" == "1" ]]; then
        log_json "cleanup" "KEEP_TMP=1 set; leaving temp directory" "{\"tmp_root\":\"$tmp_root\"}"
        return
    fi
    log_json "cleanup" "Removing temp directory" "{\"tmp_root\":\"$tmp_root\"}"
    rm -rf "$tmp_root"
}
trap cleanup EXIT

log_json "setup" "Starting Linux installer verification" "{\"installer_url\":\"$install_url\",\"log_file\":\"$log_file\"}"

require_cmd bash
require_cmd curl
require_cmd chmod
require_cmd mktemp

mkdir -p "$tmp_home" "$tmp_bin" "$tmp_config"

cache_bust_url="$install_url"
ts="$(date +%s)"
if [[ "$cache_bust_url" == *"?"* ]]; then
    cache_bust_url="${cache_bust_url}&ts=${ts}"
else
    cache_bust_url="${cache_bust_url}?ts=${ts}"
fi

log_json "download" "Downloading installer" "{\"url\":\"$cache_bust_url\",\"dest\":\"$installer_path\"}"
curl -fsSL "$cache_bust_url" -o "$installer_path" || die "Failed to download installer from $cache_bust_url"
chmod +x "$installer_path"

# Extra args support (space-separated).
extra_install_args=()
if [[ -n "${EXTRA_INSTALL_ARGS:-}" ]]; then
    # shellcheck disable=SC2206
    extra_install_args=(${EXTRA_INSTALL_ARGS})
fi

log_json "install" "Running installer (isolated)" "{\"home\":\"$tmp_home\",\"install_dir\":\"$tmp_bin\",\"config_dir\":\"$tmp_config\"}"

PATH="$tmp_bin:$PATH" \
HOME="$tmp_home" \
RCH_INSTALL_DIR="$tmp_bin" \
RCH_CONFIG_DIR="$tmp_config" \
RCH_NO_HOOK=1 \
RCH_SKIP_DOCTOR=1 \
NO_GUM=1 \
bash "$installer_path" --local --no-service --no-gum --yes "${extra_install_args[@]}" \
    >"${tmp_root}/installer.stdout" 2>"${tmp_root}/installer.stderr" || die "Installer exited non-zero; see $tmp_root/installer.stderr"

log_json "verify" "Installer completed" "{\"stdout\":\"${tmp_root}/installer.stdout\",\"stderr\":\"${tmp_root}/installer.stderr\"}"

# Validate binaries exist.
[[ -x "${tmp_bin}/rch" ]] || die "Expected executable not found: ${tmp_bin}/rch"
[[ -x "${tmp_bin}/rchd" ]] || die "Expected executable not found: ${tmp_bin}/rchd"

log_json "verify" "Installed binaries present" "{\"rch\":\"${tmp_bin}/rch\",\"rchd\":\"${tmp_bin}/rchd\"}"

# Validate --version output.
rch_version="$("${tmp_bin}/rch" --version 2>/dev/null || true)"
rchd_version="$("${tmp_bin}/rchd" --version 2>/dev/null || true)"

[[ "$rch_version" == rch\ * ]] || die "Unexpected rch --version output: $rch_version"
[[ "$rchd_version" == rchd\ * ]] || die "Unexpected rchd --version output: $rchd_version"

log_json "verify" "Version output OK" "{\"rch\":\"$rch_version\",\"rchd\":\"$rchd_version\"}"

# Basic CLI smoke tests (no daemon/service required).
"${tmp_bin}/rch" --help >/dev/null 2>&1 || die "rch --help failed"
"${tmp_bin}/rch" completions generate bash >/dev/null 2>&1 || die "rch completions generate bash failed"
"${tmp_bin}/rch" workers list >/dev/null 2>&1 || die "rch workers list failed"

log_json "verify" "CLI smoke tests OK" '{"result":"pass"}'
log_json "summary" "Installer verification passed" "{\"result\":\"pass\",\"tmp_root\":\"$tmp_root\"}"
