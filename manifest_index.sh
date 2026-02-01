#!/usr/bin/env bash
# ACFS manifest index (auto-generated).
# Format: "<sha256>  <path>" (sha256sum -c compatible)
# Usage:
#   ./manifest_index.sh --print
#   ./manifest_index.sh --verify

set -euo pipefail

manifest_entries() {
  cat <<'MANIFEST_EOF'
c75a92396c85f475c70407daeb701f82941e20f551df446897865389042df127  .claude/skills/rch/SKILL.md
a1453213c677ea95576dc4694a1322dfc6c36e62123e013791f2f6ceb06b151e  .claude/skills/rch/assets/workers-template.toml
284b423271685b94c1ad11c30ff2a40e084af2b2b727c553522372b32d623464  .claude/skills/rch/references/COMMANDS.md
2264a71e3b6aa4bbe8deeccb4fcb0b6c24da7af7280a5d8d69789a195cf2c5ef  .claude/skills/rch/references/HOOKS.md
8eca0114755171a2c6fbc61f2693c21df9d83f3f0cf091cee033fc8364d72dbc  .claude/skills/rch/references/TROUBLESHOOTING.md
8b5721ff489f211424859e681c2e9a6b8d907367a5179a34925c060811d2500b  .claude/skills/rch/references/WORKERS.md
80ae77f487d1c9c1a0168a598ad919b1cf8b266482c3b9c32b94519056de56dd  .claude/skills/rch/scripts/validate-setup.sh
f9b8937d4f50fba956f2df24f6a0e43633a78d932175d62c48f1bc5c5b43bc64  .claude/skills/remote-compilation-helper-setup/SKILL.md
a1453213c677ea95576dc4694a1322dfc6c36e62123e013791f2f6ceb06b151e  .claude/skills/remote-compilation-helper-setup/assets/workers-template.toml
2264a71e3b6aa4bbe8deeccb4fcb0b6c24da7af7280a5d8d69789a195cf2c5ef  .claude/skills/remote-compilation-helper-setup/references/HOOKS.md
8eca0114755171a2c6fbc61f2693c21df9d83f3f0cf091cee033fc8364d72dbc  .claude/skills/remote-compilation-helper-setup/references/TROUBLESHOOTING.md
8b5721ff489f211424859e681c2e9a6b8d907367a5179a34925c060811d2500b  .claude/skills/remote-compilation-helper-setup/references/WORKERS.md
80ae77f487d1c9c1a0168a598ad919b1cf8b266482c3b9c32b94519056de56dd  .claude/skills/remote-compilation-helper-setup/scripts/validate-setup.sh
MANIFEST_EOF
}

case "${1:---print}" in
  --print)
    manifest_entries
    ;;
  --verify)
    manifest_entries | sha256sum -c -
    ;;
  *)
    echo "Usage: $0 [--print|--verify]" >&2
    exit 2
    ;;
esac
