# Acceptance

Manual checks that need Keith's hands: physical buttons, macOS permission dialogs, sleep and wake, clean accounts. Claude Code writes the steps; Keith runs them and records the result. Never mark a step passed without Keith.

Record format: `PASS` or `FAIL`, date, build (git short SHA), one line of notes.

## Test layers (for context)

1. **Unit** (`cargo test`): codec round trips and proptest fuzzing, router resolution, config migrations, action argument validation.
2. **Integration** with `MockTransport` and `RecordingExecutor`: scripted attach, HELLO, presses, detach, garbage; assert dispatched actions.
3. **Hardware in the loop**: with the device on the build machine, send TEST_INJECT and verify decoding through the real USB path. Claude Code can run these without anyone pressing buttons.
4. **Manual**: this file.

## Scenarios required before v0.1

| Scenario | Layer | Milestone |
| --- | --- | --- |
| Built in keyboard never triggers an action | Manual + HIL | M1 |
| Malformed or wrong version report rejected and logged | Unit | M1 |
| Unknown device ignored; ignored device stays ignored | Integration | M3 |
| Unplug mid press, replug, state recovers | Integration + HIL | M3 |
| Sleep and wake with device attached | Manual | M3 |
| Missing heartbeat for 6 s marks device disconnected | Integration | M3 |
| App scope beats Everywhere; Everywhere used with no app match | Unit | M5 |
| Shortcut into a Secure Input field is skipped and logged | Manual | M5 |
| Accessibility revoked while running shows banner, no crash | Manual | M5 |
| Dashboard display disconnected while open | Manual | M6 |
| Older config schema migrates without loss | Unit | M7 |
| App crash relaunches and reopens the device | Manual | M7 |

## M0

Steps 1 to 6 prove the exit criterion "two consecutive signed rebuilds keep the Accessibility grant". Steps 7 to 9 check the firmware; they aren't an M0 exit criterion but are worth doing while the board is handy. The app is `target/debug/bundle/macos/Detent.app` throughout.

Setup, once:

1. Make sure you have a valid development identity: `security find-identity -v -p codesigning` lists one starting "Apple Development:" or the older "Mac Developer:". If not, create one: Xcode, Settings, Accounts, select your team, Manage Certificates, +, Apple Development. Then `export APPLE_SIGNING_IDENTITY="<that identity, quoted exactly as listed>"`, ideally in your shell profile. Never commit it.
2. Start from a clean grant state: `tccutil reset Accessibility com.gittielabs.detent`.

Signed build and tray:

3. Run `just build-dev`. It ends with `build-dev: signed with <your identity>`; an ad hoc result is a failure. Note the designated requirement: `codesign -dr - target/debug/bundle/macos/Detent.app`.
4. `open target/debug/bundle/macos/Detent.app`. A tray icon (knob outline) appears; no Dock icon, no window. Click the tray icon: the menu shows "Accessibility: not granted", Open Detent, Quit Detent. Choose Open Detent, close the window that opens; the tray icon stays. Leave the app running.

Grant, then rebuild twice:

5. Detent never prompts, so add it yourself: System Settings, Privacy & Security, Accessibility, +, pick `target/debug/bundle/macos/Detent.app`, make sure the toggle is on. Click the tray icon again; the line now says "Accessibility: granted" without a relaunch.
6. Twice in a row: Quit Detent from the tray, change the tooltip text in `app/src-tauri/src/tray.rs` (for example `"Detent"` to `"Detent 1"`, then `"Detent 2"`), run `just build-dev`, open the app. Each time the tray shows "Accessibility: granted" and macOS shows no prompt. `codesign -dr -` prints the same designated requirement as step 3. Revert the tooltip change afterwards.

If step 6 shows "not granted", compare the `codesign -dr -` output with step 3: a different requirement means the signing identity changed between builds.

Firmware (needs the XIAO ESP32-S3 on USB):

7. Run `just fw-upload`. The first flash over stock firmware may need BOOT held while plugging in. The orange user LED blinks about once a second.
8. Run `ioreg -p IOUSB -l -w0 | grep -iE '"(idVendor|idProduct|USB Product Name)"'` (or check System Information, USB). The device shows vendor `12346` (0x303A), product `36830` (0x8FDE), name `Detent Dev`. No Seeed `0x2886` entry.
9. Run `just fw-upload` again without touching BOOT. It resets and flashes on its own.

Results:

- Steps 1 to 6: `PASS`, 2026-11-04, build `147ecce` (plus the step 6 tooltip edits, since reverted). Keith confirmed "Accessibility: granted" after the manual grant and after both signed rebuilds, with no prompt. Both rebuilds had new code hashes and the same designated requirement as the granted build.
- Steps 7 to 9 (firmware): not run yet. Not an M0 exit criterion.

## M1

Needs the XIAO ESP32-S3 on USB, wired with the buttons, encoder, and OLED. Steps 3 to 6 are the M1 exit criteria; the rest check pieces as their tasks land. Exact `ctl-probe` flags are filled in by the task that adds them.

1. HID layout spike (task 1). With the spike firmware flashed, run the raw `ctl-probe` mode the task names. Record whether macOS shows any permission prompt (Input Monitoring especially) when the probe opens the device, and whether the volume keys still change the volume while the probe runs.
2. USB identity (task 5). `ioreg -p IOUSB -l -w0 | grep -iE '"(idVendor|idProduct|USB Serial Number|USB Product Name)"'` shows vendor `12346` (0x303A), product `36830` (0x8FDE), a 12 hex char serial, and `Detent <model>`. A second unit, if you have one, shows a different serial.
3. Replug (exit criterion). With the device plugged in, run `just probe --count-hellos 21` (the connection already there counts as the first), then unplug and replug the device 20 times, waiting for each HELLO line. The probe exits 0 after the 21st HELLO, which is the 20th replug.
4. Keyboard isolation (exit criterion). With `ctl-probe` running, type a sentence on the built in keyboard and on any external keyboard. The probe prints nothing.
5. Controls (task 9). With `ctl-probe` running, press, hold (over 600 ms), and release each button, and turn the encoder in Mapped mode both ways. Each shows the matching BUTTON down, long press, up, and ENCODER lines with the right sign.
6. Volume pass through (exit criterion). Quit `ctl-probe`. The encoder boots in Volume mode (or pick Volume from the OLED menu). Turning it moves the macOS volume with the native HUD; a short press toggles mute on release. A long press (over 600 ms) opens the menu and does not mute; another long press closes it.
7. OLED (task 11). Wire the display: SDA to D4, SCL to D5, VCC to 3V3, GND to GND. The status screen shows `Detent Dev`, `Mode: Volume`, `USB: connected` (or `paired`), and `Hold knob: menu`. A long press opens the menu (Volume, Mapped, Setup, Device info) with a `>` cursor and `*` on the active mode; turning moves the cursor and wraps. Selecting Mapped returns to the status screen showing `Mode: Mapped`, and turns then reach `just probe` as ENCODER. Setup shows the same serial as step 2; leaving it, by a press or a long press, makes `just probe` print a HELLO. Device info shows the firmware version, serial, protocol version, and pairing. If the picture is shifted or garbled, the module is an SH1106: build with `-DDETENT_OLED_SH1106=1`.

Results:

- Step 5: `PASS`, 2026-11-09, release firmware `b7c213b`, four tactile buttons on D0 to D3 as `board_pins.h` maps them. Keith tapped each button: one clean down and up each. Holds of 526 to 575 ms correctly sent no long press; 2 s holds sent down, long press at 599 to 601 ms, then up, for all four. No rejects or disconnects. (Deferred 2026-11-07 until the buttons arrived.)
- Step 2, second unit: DEFERRED, needs a second board. The single unit's identity checks passed. Carried into M2.
- Step 4: `PASS`, 2026-11-07, release firmware `b7c213b`. Keith typed on the built in keyboard for 40 s while `ctl-probe --raw` ran: it showed only HELLO and 20 heartbeats, with no BUTTON, ENCODER, rejected, or foreign collection reports.
- Step 3: `PASS`, 2026-11-07, release firmware `b7c213b`. Keith replugged 20 times with `ctl-probe` counting: 19 replugs in a `--count-hellos 20` run (the first count was the existing connection; 19 disconnects logged) plus 1 in a `--count-hellos 2` run. Every connection produced a HELLO; both runs exited 0.
- HIL (`scripts/hil.sh`), run by Claude on 2026-11-07, build `b7c213b`: PASS. Debug firmware replayed buttons in order (down to up 118 ms); a Volume mode detent moved the Mac's volume 44 -> 50 -> 44; a short press toggled mute false -> true -> false; a long press opened and closed the menu without muting; release firmware ignored TEST_INJECT and still sent HELLO. Board left on release firmware; audio restored.
- Step 7: `PASS`, 2026-11-07, firmware `7ca8d97` (debug build), OLED on D4/D5 at 0x3C. Keith ran a full mode sweep by the screen: in Mapped, turns and a press reached the app as ENCODER and BUTTON 6 with no volume or mute change; a long press opened the menu (the app still got the press's Up); two clicks reached Setup, which showed the serial, and leaving it sent a fresh HELLO; selecting Volume returned to Volume mode, where turns moved the volume and presses toggled mute. Device and app logs agree. Along the way the debug pin finder was found taking the OLED's I2C pins and fixed.
- Step 6: `PASS`, 2026-11-07, firmware `6be3447` (debug build, same behavior as release). Keith turned the encoder with no probe running and the macOS volume moved (after rewiring A/B from D4/D5 to D8/D9; the debug pin finder showed the mistake). In a traced run of his presses: six short presses each toggled mute on release; a long press opened the menu at 600 ms without muting; a short press in the menu selected instead of muting; a second long press returned to Volume.
- Step 1: `PASS`, 2026-11-07, build `da68b77` (spike firmware). Keith confirmed no macOS permission dialog appeared. Evidence, run by Claude (spike firmware, build after `31a5fc6`): `ctl-probe` launched through Terminal.app, whose Input Monitoring status was `unknown` (never granted), opened the vendor channel non exclusive and read reports; the status was still `unknown` afterwards, so no prompt was raised. Volume keys (Consumer Control) changed the macOS volume while the probe held the device open shared, and didn't when it was open exclusive.
- Step 2, evidence run by Claude on 2026-11-07 (build after `dc896ca`): `ioreg` shows vendor `12346`, product `36830`, serial `E072A1FC4EDC` (matches the ROM's MAC `E0:72:A1:FC:4E:DC`), product `Detent Dev`, vendor name `GittieLabs`. Not marked PASS: the second unit check needs Keith and a second board.

## M2

Needs a signed build (`just build-dev`), the board on release firmware with its buttons and encoder, and a hand written config at `~/Library/Application Support/Detent/config.json` (start from `app/src-tauri/examples/`). Step 6 is the exit criterion. Detent logs every event, resolved mapping, and action result (with dispatch time) to `~/Library/Logs/Detent/detent.log`; the tray's first line shows the connected device.

1. macOS bindings (task 1). Run `just macos-check open com.apple.mail` (Mail opens or comes forward), `just macos-check frontmost` (switch apps; each one's bundle ID prints), `just macos-check volume` (the menu bar volume jumps up for 2 s, then returns), and `just macos-check mute` (mutes for 2 s, then unmutes).
2. Device opens (Devices and Engine tasks). Launch Detent with the board allowlisted in the config. The board's LED goes solid (HELLO_ACK paired) and the OLED shows `USB: paired`. Remove the board from the allowlist and relaunch: the LED keeps blinking and Detent never reads it.
3. Volume action. Map a button to Volume up and another to Mute; each press changes the volume or mute, as the menu bar volume shows (setting it through CoreAudio doesn't show the on screen volume HUD that the keys do). Mute, then press Volume up: it unmutes (D-059).
4. (Open dashboard is deferred to M6 with its content still to be decided, D-055. No step here.)
5. Run AppleScript. Map a button to `tell application "Finder" to activate`. The first press shows macOS's Automation prompt naming Detent; after allowing, presses bring Finder forward.
6. Exit criterion: map a button (Everywhere) to Open app `com.apple.mail`. Quit Mail, press the button: Mail opens.
7. App scope. Map the same button to Volume mute only when Safari is frontmost. With Safari in front the press mutes; anywhere else it opens Mail.
8. Heartbeat and replug. Unplug the board: within 6 s Detent shows it disconnected (tray). Replug: it reconnects and presses work again, with no relaunch.

Results:

- (none yet)
