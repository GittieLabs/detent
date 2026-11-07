#!/usr/bin/env bash
# Hardware in the loop test. Needs a Detent board on USB; no button presses.
#
#   scripts/hil.sh     (or: just hil)
#
# 1. Flashes debug firmware, sends a TEST_INJECT script through ctl-probe, and
#    checks the decoded events come back in order with the right spacing.
# 2. Flashes release firmware, sends the same script, and checks nothing comes
#    back (release ignores TEST_INJECT) while HELLO still does.
# Leaves the board on release firmware.
#
# Uploads sometimes exit 1 after a verified flash (a race in the final reset),
# so a flash counts as good when the upload log shows all four regions verified
# and the vendor channel re-enumerates, whatever the exit code.
#
# It writes to the device the moment it re-enumerates, on purpose: on
# Arduino-ESP32 2.0.17 that tripped a TinyUSB stall (D-045), and this script is
# the regression test for it.
#
# Logs are kept on failure; the script prints where.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
probe="$root/target/debug/ctl-probe"
work="$(mktemp -d)"

# Button 1 down, up 120 ms later, then the encoder (control 5) +2 and -1.
script="B1:down B1:up@120 E5:+2@50 E5:-1@50"
expected=$'BUTTON 1 down\nBUTTON 1 up\nENCODER 5 +2\nENCODER 5 -1'

fail() {
  echo "hil: FAIL: $*" >&2
  echo "hil: logs kept in $work" >&2
  exit 1
}

# Capture first: `probe | grep -q` can fail under pipefail when grep exits early.
channel_up() {
  local out
  out="$("$probe" list 2>/dev/null || true)"
  [[ "$out" == *"usage_page=0xFF00"* ]]
}

flash() {
  local env="$1" log="$work/upload-$1.log"
  echo "hil: flashing $env"
  pio run -d "$root/firmware" -e "$env" -t upload >"$log" 2>&1 || true
  local verified
  verified="$(grep -c "Hash of data verified" "$log" || true)"
  if (( verified < 4 )); then
    tail -20 "$log" >&2
    fail "upload of $env verified $verified of 4 regions"
  fi
  for _ in $(seq 1 40); do
    if channel_up; then
      return 0
    fi
    sleep 0.5
  done
  fail "$env flashed but the vendor channel didn't come back within 20 s"
}

# Prints the decoded BUTTON and ENCODER lines from a probe run.
events_from() {
  sed -nE 's/^ *[0-9.]+s +seq +[0-9]+ +((BUTTON|ENCODER) .*)$/\1/p' "$1"
}

run_probe() {
  local out="$1"
  "$probe" --inject "$script" --exit-after 3 >"$out" 2>&1 || fail "ctl-probe exited nonzero; see $out"
  grep -q " HELLO fw " "$out" || { cat "$out" >&2; fail "no HELLO from the device"; }
}

echo "hil: building ctl-probe"
cargo build -q --manifest-path "$root/Cargo.toml" -p ctl-probe
board="$("$probe" list 2>/dev/null || true)"
[[ "$board" == *"vid=0x303A"* ]] || compgen -G "/dev/cu.usbmodem*" >/dev/null ||
  fail "no board found on USB"

# --- debug firmware: injected events come back ---
flash xiao_esp32s3_debug
run_probe "$work/debug.log"
got="$(events_from "$work/debug.log")"
if [[ "$got" != "$expected" ]]; then
  echo "expected:"; echo "$expected"; echo "got:"; echo "${got:-<nothing>}"
  cat "$work/debug.log" >&2
  fail "debug firmware didn't replay the script"
fi
# The up event should land about 120 ms after the down event.
gap_ms="$(awk '/BUTTON 1 down/ {d=$1} /BUTTON 1 up/ {u=$1} END {sub("s","",d); sub("s","",u); printf "%d", (u-d)*1000}' "$work/debug.log")"
if (( gap_ms < 100 || gap_ms > 300 )); then
  fail "down to up took ${gap_ms} ms, expected about 120"
fi
echo "hil: debug firmware replayed 4 events in order; down to up ${gap_ms} ms"

# --- release firmware: TEST_INJECT ignored ---
flash xiao_esp32s3
run_probe "$work/release.log"
got="$(events_from "$work/release.log")"
if [[ -n "$got" ]]; then
  echo "$got" >&2
  fail "release firmware replayed TEST_INJECT"
fi
echo "hil: release firmware ignored TEST_INJECT; HELLO still arrives"

echo "hil: PASS (board left on release firmware)"
rm -rf "$work"
