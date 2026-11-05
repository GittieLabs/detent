#!/usr/bin/env bash
# Build and sign a Detent dev build with a stable identity, so macOS
# permission grants (Accessibility) survive rebuilds (D-010).
#
#   export APPLE_SIGNING_IDENTITY="Apple Development: Your Name (TEAMID)"
#   scripts/build-dev.sh
#
# Output: target/debug/bundle/macos/Detent.app
set -euo pipefail

die() {
  echo "build-dev: $*" >&2
  exit 1
}

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
app="$root/target/debug/bundle/macos/Detent.app"

[[ "$(uname -s)" == "Darwin" ]] || die "macOS only."

identity="${APPLE_SIGNING_IDENTITY:-}"
if [[ -z "$identity" ]]; then
  die "APPLE_SIGNING_IDENTITY is not set.
  List your identities:  security find-identity -v -p codesigning
  Then set one:          export APPLE_SIGNING_IDENTITY=\"Apple Development: Your Name (TEAMID)\""
fi
if [[ "$identity" == "-" ]]; then
  die "APPLE_SIGNING_IDENTITY is '-' (ad hoc). Ad hoc signatures change every build and reset
  Accessibility grants (D-010). Use your Apple Development identity."
fi
if ! security find-identity -v -p codesigning | grep -qF -- "$identity"; then
  die "no valid codesigning identity matching \"$identity\" in the keychain.
  List your identities:  security find-identity -v -p codesigning"
fi

for tool in pnpm cargo codesign; do
  command -v "$tool" >/dev/null || die "$tool not found on PATH."
done

echo "build-dev: installing UI dependencies"
pnpm -C "$root/app" install --frozen-lockfile

echo "build-dev: building and signing Detent.app"
APPLE_SIGNING_IDENTITY="$identity" pnpm -C "$root/app" tauri build --debug --bundles app

[[ -d "$app" ]] || die "expected bundle not found at $app"
codesign --verify --deep --strict "$app" || die "signature check failed for $app"
# -vv: Authority lines only appear at verbosity 2.
details="$(codesign -dvv "$app" 2>&1)"
if grep -q "Signature=adhoc" <<<"$details"; then
  die "$app is ad hoc signed; the identity was not applied."
fi

echo "build-dev: signed with $(grep -m1 '^Authority=' <<<"$details" | cut -d= -f2-)"
echo "build-dev: $app"
