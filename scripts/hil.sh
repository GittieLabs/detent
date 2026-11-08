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

# Button 1 down, then up 120 ms later.
script="B1:down B1:up@120"
expected=$'BUTTON 1 down\nBUTTON 1 up'

# The encoder (control 5) and its push switch (control 6) go through the mode
# router: the device boots in Volume mode, where turns move the Mac's volume
# and a short press toggles mute, and a long press opens the menu (D-048).
vol() { osascript -e 'output volume of (get volume settings)'; }
muted() { osascript -e 'output muted of (get volume settings)'; }
VOL0="$(vol)"
MUTED0="$(muted)"
restore_audio() {
  osascript -e "set volume output volume $VOL0" -e "set volume output muted $MUTED0" >/dev/null
}
trap restore_audio EXIT

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
echo "hil: debug firmware replayed buttons in order; down to up ${gap_ms} ms"

# Volume mode: one detent moves the volume, one back restores it.
inject() { "$probe" --inject "$1" --exit-after "$2" >"$3" 2>&1 || fail "ctl-probe exited nonzero; see $3"; }
first=+1; second=-1
if (( VOL0 > 90 )); then first=-1; second=+1; fi
inject "E5:$first" 1 "$work/vol1.log"
v1="$(vol)"
inject "E5:$second" 1 "$work/vol2.log"
v2="$(vol)"
if [[ "$first" == "+1" ]]; then (( v1 > VOL0 )) || fail "volume didn't rise: $VOL0 -> $v1"; else (( v1 < VOL0 )) || fail "volume didn't fall: $VOL0 -> $v1"; fi
(( v2 == VOL0 )) || fail "volume didn't return: $VOL0 -> $v1 -> $v2"
grep -q "ENCODER" "$work/vol1.log" && fail "Volume mode sent ENCODER to the app"
echo "hil: Volume mode turns moved the volume $VOL0 -> $v1 -> $v2"

# Short press toggles mute; a second one toggles it back.
inject "B6:down B6:up@50" 1 "$work/mute1.log"
m1="$(muted)"
inject "B6:down B6:up@50" 1 "$work/mute2.log"
m2="$(muted)"
[[ "$m1" != "$MUTED0" && "$m2" == "$MUTED0" ]] || fail "mute didn't toggle: $MUTED0 -> $m1 -> $m2"
grep -q "BUTTON 6" "$work/mute1.log" && fail "Volume mode sent the push switch to the app"
echo "hil: short press toggled mute $MUTED0 -> $m1 -> $m2"

# Long press opens the menu (HEARTBEAT reports it), another closes it.
inject "B6:down B6:long@50 B6:up@50" 3 "$work/menu1.log"
grep -q "HEARTBEAT .*mode menu" "$work/menu1.log" || { cat "$work/menu1.log" >&2; fail "long press didn't open the menu"; }
inject "B6:down B6:long@50 B6:up@50" 3 "$work/menu2.log"
grep -q "HEARTBEAT .*mode volume" "$work/menu2.log" || { cat "$work/menu2.log" >&2; fail "long press didn't close the menu"; }
[[ "$(muted)" == "$MUTED0" ]] || fail "a long press toggled mute"
echo "hil: long press opened the menu and closed it, without muting"

# --- release firmware: TEST_INJECT ignored ---
flash xiao_esp32s3
run_probe "$work/release.log"
got="$(events_from "$work/release.log")"
if [[ -n "$got" ]]; then
  echo "$got" >&2
  fail "release firmware replayed TEST_INJECT"
fi
echo "hil: release firmware ignored TEST_INJECT; HELLO still arrives"

restore_audio
echo "hil: PASS (board left on release firmware)"
rm -rf "$work"
