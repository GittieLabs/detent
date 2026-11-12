#!/usr/bin/env bash
# Tag and publish a Detent release (D-114, D-120).
#
# Prepare (local only, nothing leaves the Mac):
#   scripts/release.sh            (or: just release)
#   Checks the tree is clean on main and pushed, the app's versions agree and
#   have no tag yet, and the notarized, stapled DMG is there; builds each
#   board's release firmware; writes the release notes; creates the annotated
#   tag v<version> on HEAD (not pushed).
#
# Publish (pushes the tag and uploads; Keith's OK each time):
#   scripts/release.sh --publish  (or: just release-publish)
#   Pushes the tag and creates a draft GitHub Release with the DMG, the
#   firmware images and the notes. Nothing is public until Publish is pressed
#   on GitHub.
#
# Output: target/release/dist/ (the DMG from just build-release, firmware/,
# notes-v<version>.md)
set -euo pipefail

die() {
  echo "release: $*" >&2
  exit 1
}
say() { echo "release: $*"; }

root="${DETENT_RELEASE_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"

# Release firmware for every board, by PlatformIO env. Debug envs never ship.
envs=(xiao_esp32s3 feather_esp32s3 devkitc_esp32s3 pico)

publish=false
while (($#)); do
  case "$1" in
    --publish) publish=true ;;
    *) die "unknown argument: $1 (use --publish, or nothing to prepare)" ;;
  esac
  shift
done

[[ "$(uname -s)" == "Darwin" ]] || die "macOS only."
cd "$root"

# Versions: the app's (workspace and UI must agree), the firmware's and the
# protocol's, for the tag and the notes.
version="$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)"
[[ -n "$version" ]] || die "no [workspace.package] version in Cargo.toml"
ui="$(sed -n 's/^  "version": "\(.*\)",$/\1/p' app/package.json | head -1)"
[[ "$ui" == "$version" ]] ||
  die "Cargo.toml says $version but app/package.json says ${ui:-nothing}; make them agree first."
fw_part() { sed -n "s/^#define DETENT_FW_$1 \([0-9]*\)/\1/p" firmware/src/version.h; }
firmware="$(fw_part MAJOR).$(fw_part MINOR).$(fw_part PATCH)"
[[ "$firmware" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "can't read the firmware version from firmware/src/version.h"
protocol="$(sed -n 's/^pub const PROTOCOL_VERSION: u8 = \([0-9]*\);/\1/p' protocol/src/lib.rs)"
[[ -n "$protocol" ]] || die "can't read PROTOCOL_VERSION from protocol/src/lib.rs"
tag="v$version"

dist="$root/target/release/dist"
dmg="$dist/Detent-$version.dmg"
fwdir="$dist/firmware"
notes="$dist/notes-$tag.md"

# The git state every release needs: main, clean, and pushed, so the tag
# names a commit that's on GitHub.
branch="$(git rev-parse --abbrev-ref HEAD)"
[[ "$branch" == "main" ]] || die "on branch $branch; releases are tagged on main."
[[ -z "$(git status --porcelain)" ]] || die "the working tree isn't clean; commit or stash first:
$(git status --short)"
git rev-parse --verify --quiet refs/remotes/origin/main >/dev/null ||
  die "no origin/main; push main first."
git merge-base --is-ancestor HEAD refs/remotes/origin/main ||
  die "HEAD isn't on origin/main; push main first."

check_dmg() {
  [[ -f "$dmg" ]] || die "no DMG at $dmg; run just build-release, then just notarize-release."
  xcrun stapler validate "$dmg" >/dev/null 2>&1 ||
    die "$dmg has no valid stapled ticket; run just notarize-release (Keith's OK)."
  spctl -a -t open --context context:primary-signature "$dmg" >/dev/null 2>&1 ||
    die "Gatekeeper rejects $dmg"
}

if $publish; then
  [[ "$(git rev-parse --verify --quiet "refs/tags/$tag^{commit}")" == "$(git rev-parse HEAD)" ]] ||
    die "no tag $tag on HEAD; run just release first."
  check_dmg
  [[ -f "$notes" ]] || die "no release notes at $notes; run just release first."
  assets=("$dmg")
  for env in "${envs[@]}"; do
    f="$(ls "$fwdir"/detent-fw-"$env"-"$firmware".* 2>/dev/null | head -1)"
    [[ -n "$f" ]] || die "no firmware image for $env in $fwdir; run just release first."
    assets+=("$f")
  done
  command -v gh >/dev/null || die "gh not found on PATH."
  ! gh release view "$tag" >/dev/null 2>&1 || die "GitHub already has a release $tag."

  say "pushing tag $tag"
  git push origin "refs/tags/$tag"
  say "creating the draft release $tag (uploads ${#assets[@]} files)"
  gh release create "$tag" --draft --verify-tag --title "Detent $version" \
    --notes-file "$notes" "${assets[@]}"
  say "draft created; review it on GitHub and press Publish."
  exit 0
fi

git rev-parse --verify --quiet "refs/tags/$tag" >/dev/null &&
  die "tag $tag already exists; bump the version in Cargo.toml and app/package.json for a new release."
check_dmg
command -v pio >/dev/null || die "pio not found on PATH."

say "building release firmware ${firmware} for ${envs[*]}"
args=()
for env in "${envs[@]}"; do args+=(-e "$env"); done
pio run -d firmware "${args[@]}"

rm -rf "$fwdir"
mkdir -p "$fwdir"
for env in "${envs[@]}"; do
  build="firmware/.pio/build/$env"
  if [[ -f "$build/firmware.uf2" ]]; then
    # RP2040: drag onto the RPI-RP2 drive in BOOTSEL mode.
    cp "$build/firmware.uf2" "$fwdir/detent-fw-$env-$firmware.uf2"
  elif [[ -f "$build/firmware.factory.bin" ]]; then
    # ESP32-S3: bootloader, partitions and app in one image, written at 0x0.
    cp "$build/firmware.factory.bin" "$fwdir/detent-fw-$env-$firmware.bin"
  else
    die "no flashable image in $build"
  fi
done

say "writing $notes"
{
  echo "Detent $version: app $version, firmware $firmware, protocol v$protocol."
  echo
  echo "The app works with units on protocol v$protocol firmware. A unit on older"
  echo "firmware shows \"firmware too old\" on Devices until it's flashed with one of"
  echo "the images below."
  echo
  echo "## Files"
  echo
  echo "- \`Detent-$version.dmg\`: the app, notarized. Open it and drag Detent to Applications."
  for env in "${envs[@]}"; do
    f="$(cd "$fwdir" && ls detent-fw-"$env"-"$firmware".*)"
    case "$f" in
      *.uf2) how="hold BOOTSEL while plugging in, then copy it to the RPI-RP2 drive" ;;
      *) how="\`esptool.py write_flash 0x0 $f\`, or \`just fw-upload $env\` from the repo" ;;
    esac
    echo "- \`$f\`: firmware for \`$env\` ($how)."
  done
  echo
  echo "Boards and USB IDs: docs/boards.md. Changes since the last release: the"
  echo "commits on main, and the rows added to docs/decisions.md."
} >"$notes"

say "tagging HEAD as $tag"
git tag -a "$tag" -m "Detent $version (firmware $firmware, protocol v$protocol)"

say "ready: $tag on $(git rev-parse --short HEAD), $(ls "$fwdir" | wc -l | tr -d ' ') firmware images, notes in $notes"
say "next, with Keith's OK (pushes the tag and uploads a draft release):  just release-publish"
