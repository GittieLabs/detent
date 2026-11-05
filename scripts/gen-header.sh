#!/usr/bin/env bash
# Regenerate firmware/include/detent_protocol.h from the protocol crate, the
# source of truth for the wire format (docs/protocol.md).
#
#   scripts/gen-header.sh           write the header
#   scripts/gen-header.sh --check   fail if the committed header is stale (CI)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="$root/firmware/include/detent_protocol.h"

generated="$(cargo run -q --manifest-path "$root/Cargo.toml" -p detent-protocol --bin gen-header)"

case "${1:-}" in
  "")
    mkdir -p "$(dirname "$out")"
    printf '%s\n' "$generated" >"$out"
    echo "gen-header: wrote ${out#"$root"/}"
    ;;
  --check)
    if ! diff -u "$out" <(printf '%s\n' "$generated") >&2; then
      echo "gen-header: ${out#"$root"/} is stale; run scripts/gen-header.sh" >&2
      exit 1
    fi
    echo "gen-header: ${out#"$root"/} is up to date"
    ;;
  *)
    echo "usage: scripts/gen-header.sh [--check]" >&2
    exit 2
    ;;
esac
