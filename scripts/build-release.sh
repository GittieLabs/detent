#!/usr/bin/env bash
# Build, sign and notarize a Detent release (D-010, D-099).
#
# Build and sign (local only, nothing leaves the Mac):
#   APPLE_SIGNING_IDENTITY="Developer ID Application: Your Org (TEAMID)" scripts/build-release.sh
#
# Notarize and staple the DMG that build made (uploads it to Apple):
#   APPLE_NOTARY_PROFILE=<profile> scripts/build-release.sh --notarize [--dmg PATH]
#
# The notary profile is a keychain item made once with
#   xcrun notarytool store-credentials <profile>
# so no password passes through the shell, the repo or the logs.
#
# Output: target/release/dist/Detent-<version>.dmg
set -euo pipefail

die() {
  echo "build-release: $*" >&2
  exit 1
}
say() { echo "build-release: $*"; }

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' "$root/Cargo.toml")"
app="$root/target/release/bundle/macos/Detent.app"
dist="$root/target/release/dist"
dmg="$dist/Detent-$version.dmg"

notarize=false
while (($#)); do
  case "$1" in
    --notarize) notarize=true ;;
    --dmg)
      [[ $# -ge 2 ]] || die "--dmg needs a path"
      dmg="$2"
      shift
      ;;
    *) die "unknown argument: $1 (use --notarize [--dmg PATH], or nothing to build)" ;;
  esac
  shift
done

[[ "$(uname -s)" == "Darwin" ]] || die "macOS only."

notarize_dmg() {
  local profile="${APPLE_NOTARY_PROFILE:-}"
  [[ -n "$profile" ]] || die "APPLE_NOTARY_PROFILE is not set.
  Store your notary credentials in the keychain once:  xcrun notarytool store-credentials <profile>
  Then:  export APPLE_NOTARY_PROFILE=<profile>"
  [[ -f "$dmg" ]] || die "no DMG at $dmg; run just build-release first."

  say "submitting $dmg to Apple's notary service (this uploads it)"
  local out id status
  out="$(xcrun notarytool submit "$dmg" --keychain-profile "$profile" --wait 2>&1)" || true
  echo "$out"
  id="$(awk '/^  id: /{print $2; exit}' <<<"$out")"
  status="$(awk '/^  status: /{s=$2} END{print s}' <<<"$out")"
  if [[ "$status" != "Accepted" ]]; then
    [[ -n "$id" ]] && xcrun notarytool log "$id" --keychain-profile "$profile" || true
    die "notarization ended with status '${status:-none}'; the log is above."
  fi

  say "stapling the ticket"
  xcrun stapler staple "$dmg"
  xcrun stapler validate "$dmg" || die "stapled ticket doesn't validate"
  spctl -a -t open --context context:primary-signature -vv "$dmg" ||
    die "Gatekeeper rejects $dmg"

  local mnt
  mnt="$(mktemp -d)"
  hdiutil attach -nobrowse -readonly -mountpoint "$mnt" "$dmg" >/dev/null
  local ok=true
  spctl -a -t exec -vv "$mnt/Detent.app" || ok=false
  hdiutil detach "$mnt" >/dev/null
  rmdir "$mnt" 2>/dev/null || true
  $ok || die "Gatekeeper rejects the Detent.app inside $dmg"

  say "notarized and stapled: $dmg"
}

if $notarize; then
  notarize_dmg
  exit 0
fi

identity="${APPLE_SIGNING_IDENTITY:-}"
[[ -n "$identity" ]] || die "APPLE_SIGNING_IDENTITY is not set.
  A release needs a Developer ID Application identity. List yours:
    security find-identity -v -p codesigning
  Then pass it for this command only:
    APPLE_SIGNING_IDENTITY=\"Developer ID Application: Your Org (TEAMID)\" just build-release"
# The identity is its full name or its certificate's SHA-1 hash (40 hex
# characters), which picks one certificate when two share a name.
if [[ "$identity" =~ ^[0-9A-Fa-f]{40}$ ]]; then
  hash="$(tr '[:lower:]' '[:upper:]' <<<"$identity")"
  matches="$(security find-identity -v -p codesigning | grep -F -- " $hash " || true)"
  [[ -n "$matches" ]] || die "no valid codesigning identity with hash $hash in the keychain.
  List your identities:  security find-identity -v -p codesigning"
  grep -qF '"Developer ID Application:' <<<"$matches" ||
    die "a release needs a Developer ID Application identity; $hash is $(cut -d'"' -f2 <<<"$matches")."
  identity="$hash"
else
  [[ "$identity" == "Developer ID Application:"* ]] ||
    die "a release needs a Developer ID Application identity, not \"$identity\".
  Apple Development and ad hoc signatures can't be notarized (D-010, D-099)."
  count="$(security find-identity -v -p codesigning | grep -cF -- "\"$identity\"" || true)"
  [[ "$count" -gt 0 ]] || die "no valid codesigning identity matching \"$identity\" in the keychain.
  List your identities:  security find-identity -v -p codesigning"
  [[ "$count" -eq 1 ]] || die "$count valid certificates are named \"$identity\", so codesign can't choose.
  Pass the SHA-1 hash of the one to use instead (the first column of
  security find-identity -v -p codesigning), usually the one that expires last."
fi

for tool in pnpm cargo codesign hdiutil xcrun; do
  command -v "$tool" >/dev/null || die "$tool not found on PATH."
done

# Tauri notarizes on its own when these are set; this script does it instead,
# with a keychain profile, in its own step.
unset APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID APPLE_API_KEY APPLE_API_ISSUER APPLE_API_KEY_PATH

say "installing UI dependencies"
pnpm -C "$root/app" install --frozen-lockfile

say "building and signing Detent $version (release)"
APPLE_SIGNING_IDENTITY="$identity" pnpm -C "$root/app" tauri build --bundles app

[[ -d "$app" ]] || die "expected bundle not found at $app"
codesign --verify --deep --strict "$app" || die "signature check failed for $app"
details="$(codesign -dvv "$app" 2>&1)"
grep -q "flags=0x10000(runtime)" <<<"$details" || die "$app lacks the hardened runtime"
grep -q "^Authority=Developer ID Application:" <<<"$details" ||
  die "$app isn't signed with a Developer ID Application identity"
grep -q "^Timestamp=" <<<"$details" || die "$app has no secure timestamp; notarization needs one"
ents="$(codesign -d --entitlements - --xml "$app" 2>/dev/null)"
grep -q "com.apple.security.automation.apple-events" <<<"$ents" ||
  die "$app lacks the Apple Events entitlement"
! grep -q "get-task-allow" <<<"$ents" || die "$app allows debugging (get-task-allow)"

say "making the DMG"
mkdir -p "$dist"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
cp -R "$app" "$stage/"
ln -s /Applications "$stage/Applications"
rm -f "$dmg"
hdiutil create -quiet -volname "Detent $version" -srcfolder "$stage" -ov -format UDZO "$dmg"
codesign --sign "$identity" --timestamp "$dmg"
codesign --verify --strict "$dmg" || die "signature check failed for $dmg"

say "signed with $(grep -m1 '^Authority=' <<<"$details" | cut -d= -f2-)"
say "$dmg"
say "next: notarize it with your OK:  APPLE_NOTARY_PROFILE=<profile> just notarize-release"
