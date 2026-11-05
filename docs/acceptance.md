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
3. Replug (exit criterion). Run `ctl-probe --count-hellos 20`, then unplug and replug the device 20 times, waiting for each HELLO line. The probe exits 0 after the 20th.
4. Keyboard isolation (exit criterion). With `ctl-probe` running, type a sentence on the built in keyboard and on any external keyboard. The probe prints nothing.
5. Controls (task 9). With `ctl-probe` running, press, hold (over 600 ms), and release each button, and turn the encoder in Mapped mode both ways. Each shows the matching BUTTON down, long press, up, and ENCODER lines with the right sign.
6. Volume pass through (exit criterion). Quit `ctl-probe`. Put the encoder in Volume mode from the OLED menu. Turning it moves the macOS volume with the native HUD; pressing it toggles mute.
7. OLED (task 11). The screen shows connection, paired state, and mode. The menu lists Volume, Mapped, Setup, Device info; Setup shows the same serial as step 2; leaving Setup makes `ctl-probe` print a HELLO.

Results:

- (none yet)
