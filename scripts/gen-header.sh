#!/usr/bin/env bash
# Regenerate the firmware files that come from the protocol crate, the source
# of truth for the wire format (docs/protocol.md):
#   firmware/include/detent_protocol.h  constants (gen-header bin)
#   firmware/test/vectors.h             golden codec reports (gen-vectors bin, D-035)
#
#   scripts/gen-header.sh           write both files
#   scripts/gen-header.sh --check   fail if either committed file is stale (CI)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mode="${1:-}"
case "$mode" in
  "" | --check) ;;
  *)
    echo "usage: scripts/gen-header.sh [--check]" >&2
    exit 2
    ;;
esac

stale=0
generate() {
  local bin="$1" out="$root/$2" generated
  generated="$(cargo run -q --manifest-path "$root/Cargo.toml" -p detent-protocol --bin "$bin")"
  if [[ "$mode" == "--check" ]]; then
    if ! diff -u "$out" <(printf '%s\n' "$generated") >&2; then
      echo "gen-header: $2 is stale; run scripts/gen-header.sh" >&2
      stale=1
    else
      echo "gen-header: $2 is up to date"
    fi
  else
    mkdir -p "$(dirname "$out")"
    printf '%s\n' "$generated" >"$out"
    echo "gen-header: wrote $2"
  fi
}

generate gen-header firmware/include/detent_protocol.h
generate gen-vectors firmware/test/vectors.h
exit "$stale"
