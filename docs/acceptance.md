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
- Step 2, second unit: `PASS`, 2026-11-10. Keith plugged in a second XIAO alone (factory firmware, MAC `E0:72:A1:FC:84:9C`); with his OK Claude flashed release firmware from `15af3c1` (4 of 4 regions verified). `ioreg` shows vendor `GittieLabs`, product `Detent Dev`, VID 12346, PID 36830, serial `E072A1FC849C`, different from the first unit's `E072A1FC4EDC`. Detent logged it as `Unknown` (not paired) at once.
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
5. Run AppleScript. Map a button to `tell application "Finder"` / `reopen` / `activate` / `end tell` (the example config's button 4). Close every Finder window first and put another app in front. The first press shows macOS's Automation prompt naming Detent (`reopen` is an Apple Event to Finder; `activate` alone needs no permission); after allowing, each press opens a Finder window if there is none and brings Finder forward. `activate` alone brings Finder forward but opens no window, which looks like nothing happened when Finder has none (found 2026-11-09).
6. Exit criterion: map a button (Everywhere) to Open app `com.apple.mail`. Quit Mail, press the button: Mail opens.
7. App scope. Map the same button to Volume mute only when Safari is frontmost. With Safari in front the press mutes; anywhere else it opens Mail.
8. Heartbeat and replug. Unplug the board: within 6 s Detent shows it disconnected (tray). Replug: it reconnects and presses work again, with no relaunch.

Results:

- Run 2026-11-09, signed `Detent.app` from `just build-dev` at `4fae1bd` (Mac Developer identity, `com.gittielabs.detent`), board `E072A1FC4EDC` on release firmware `b7c213b`, config written by Claude from the example with button 3 as Mute. Steps 1 to 8: `PASS`. Keith did each step; Claude read Detent's log for every press.
- Step 1: `PASS`, Keith. Mail opened (Claude confirmed it running and frontmost). `frontmost` printed only Claude at first: the tool never ran a run loop, so AppKit never refreshed the value (Claude reproduced it by activating Finder mid run). Fixed in `4fae1bd`; Keith's rerun followed Claude, a Chrome app, Plasticity, Passwords. Volume 50 -> 62 -> 50%; mute false -> true -> false.
- Step 2: `PASS`, Keith. Paired: LED solid, OLED `USB: paired`, tray `Connected: Desk controller`. With an empty allowlist and a relaunch, the LED kept blinking, the OLED didn't show paired, and the tray showed `No device connected`; the log showed one `Unknown` for the serial and no open. Keith's config restored afterwards.
- Step 3: `PASS`, Keith. Button 2 stepped 50 -> 56 -> 63 -> 69 -> 75 -> 81%, button 3 muted, button 2 while muted unmuted and stepped to 88% (D-059). Dispatch 0.6 to 5.1 ms.
- Step 5: `PASS`, Keith, after a script fix. With `tell application "Finder" to activate`, presses seemed to do nothing. Ruled out by Claude with a probe run from launchd: Detent's exact code path, an accessory app, and the Chrome app in front all brought Finder forward. Root cause, by Keith's tests: with no Finder window open, `activate` makes Finder frontmost (the log read `com.apple.finder` on the next presses) but opens nothing; and `activate` needs no Automation permission, hence no prompt. `reopen` opens a window but is also exempt. With the script counting Finder windows, reopening if none, then activating, the prompt named Detent and Finder came forward. The first press took 4.6 s while the prompt was up (M7 follow-up in the roadmap).
- Step 6 (exit criterion): `PASS`, Keith. Quit Mail, pressed button 1 from another app: Mail opened.
- Step 7: `PASS`, Keith. In Safari, button 1 toggled mute 8 times and never opened Mail; in the Chrome app it opened Mail. Frontmost app tracking works in the real app.
- Step 8: `PASS`, Keith. Unplug was logged at once as a link error; the tray showed no device. Replug: connected and a HELLO 12 ms later, then button 2 stepped the volume with no relaunch.
- Engine HIL, run by Claude on 2026-11-09, build `9b595f9`: PASS. Debug firmware flashed (4 of 4 regions verified); the app ran `--dry-run` with the example config; `ctl-probe --inject` replayed presses of buttons 1 to 3 and a press and long press of button 4. The app logged each against the frontmost app (Claude) and resolved it: 1 to `open_app com.apple.mail`, 2 to `volume_up 6%`, 3 to `volume_down 6%`, 4 to the Finder AppleScript, 4 long to `volume_set 25%`; each recorded (dry run), dispatch 0.2 to 0.7 ms. Nothing ran. Three HELLOs were acked (the app's REQUEST_HELLO and the probe's, which the app also sees). Release firmware reflashed (4 of 4 verified); TEST_INJECT produced no BUTTON on it. Not an acceptance step; steps 1 to 8 still need Keith.

## M3

Needs a signed build (`just build-dev`), the board on release firmware, and Detent's config moved aside so it starts empty (`mv ~/Library/Application\ Support/Detent/config.json{,.bak}`). Steps 2 and 3 are the exit criteria.

1. Devices screen. Open Detent from the tray. The Devices screen shows no paired device and a Pair new device button. Close the window: Detent keeps running in the menu bar.
2. Exit criterion: pair in under 2 minutes. Start a timer, click Pair new device, and follow the wizard with no docs: the board appears as a candidate, pressing one of its buttons picks it, name it, finish. Stop the timer: under 2 minutes. The board's LED goes solid and the OLED shows `USB: paired`; the Devices screen shows it connected with its firmware version.
3. Exit criterion: reboot. Turn on launch at login in the tray, restart the Mac, and log in without touching Detent. Detent is in the menu bar, the board is connected, and a mapped button works.
4. Sleep and wake. With the board connected, sleep the Mac (Apple menu, Sleep), wait 30 s, wake it. Within a few seconds of waking the board is connected again and a mapped button works, with no relaunch.
5. Tray. The tray shows the connection state, and Pause stops presses running anything (the log still shows them) until it's turned off, including across a relaunch. Plug in a second, unpaired unit if one is available: the tray shows "new device found" and it opens the wizard. Unplug and replug: state updates without a relaunch.
6. Ignore. In the wizard, Ignore a candidate (or an unpaired unit). It moves to the Ignored list and never appears as a candidate again until un-ignored.
7. Closing mid wizard. Open Pair new device with an unpaired board plugged in, then close the window before pairing. The board stays unpaired (LED still blinking). Open Detent again from the tray: it shows the Devices screen, not the wizard, with the board under "Plugged in, not paired". Pair new device again finds it, and a press picks it.

Results:

- Early checks of task 1, 2026-11-09, signed build after `1eafc76` plus the NotFound fix (not the acceptance pass; steps 3 and 4 still to run on the final build):
  - Open at Login: the first build greyed the item out ("needs the built app"): a never registered bundle reports SMAppService `NotFound`, which was mapped to unavailable. Fixed to mean off; Keith then turned it on and Detent appeared in Login Items. Reboot not yet tried.
  - Sleep and wake: Keith slept the Mac for about 35 s (pmset: 22:12:33 to 22:13:08). It went into DarkWake with TCP keep alive, so the CPU kept running: no clock gap, no `Slept` reopen, and the board stayed connected with heartbeats. Button 2 then stepped the volume 38 to 81%, dispatch 0.3 to 1.2 ms. The wake reopen path (D-068) is still untested on hardware: it needs a full sleep that stops the CPU.
- Early checks of task 6, 2026-11-10, signed build of the wizard on top of `6a86806` (Mac Developer identity), a throwaway config with no devices, board `E072A1FC4EDC` on release firmware (not the acceptance pass, which task 8 runs on the final build):
  - Step 2: `PASS`, Keith. Paired through the wizard in under 2 minutes with no docs.
  - Step 7: `PASS`, Keith. Closing the window mid wizard left the board unpaired; reopening showed Devices, and the wizard found the board again.
- Early checks of task 7, 2026-11-10, signed build of the tray work on top of `2db975b`, run on a copy of Keith's config, board `E072A1FC4EDC` on release firmware (not the acceptance pass):
  - Step 5: `PASS`, Keith. The device line showed 🟢 connected. Pause stopped button 2 changing the volume, and turning it off restored it. Pause stayed on across a quit and relaunch (the copy's config read `"paused": true`). Unpairing showed ⚪ and "New device found: Pair…", which opened the wizard, and the board paired again. Unplug and replug updated the line with no relaunch.
- Acceptance pass, 2026-11-10, signed `Detent.app` from `just build-dev` at `ef3b7ce` (Mac Developer identity, `com.gittielabs.detent`), board `E072A1FC4EDC` on release firmware, Keith's config moved aside to `config.json.bak` so Detent started empty. Keith did each step; Claude read Detent's log.
  - First attempt at `57a1c15`: the LED never blinked. The firmware keeps "paired" until it's unmounted or acked unpaired, and Detent only ever acked `paired = 1`, so the board stayed solid after an unpair and after the previous Detent quit. Fixed in `ef3b7ce` (D-076): `HELLO_ACK 0` on unpair and on quit. Keith confirmed Quit then made the LED blink; the run restarted from empty (the first attempt's config is kept as `config.json.run1`).
  - Step 1: `PASS`, Keith.
  - Step 6: `PASS`, Keith.
  - Step 7: `PASS`, Keith. The LED kept blinking after closing mid wizard.
  - Step 2 (exit criterion): `PASS`, Keith. Paired in under 2 minutes with no docs, keeping the suggested name `Detent Dev`. LED solid, OLED `USB: paired`, Devices showed it connected with its firmware. Unpairing then made the LED blink (D-076), and it paired again.
  - The five mappings from `config.json.bak` were then copied into the new config by Claude (M3 has no mapping editor) for steps 5, 4 and 3.
  - Step 5: `PASS`, Keith. Presses logged `(paused, not run)` while paused; Pause stayed on across a quit and relaunch; with it off, button 2 stepped the volume 25 -> 31 -> ... -> 63%. Unplug logged a disconnect at once and replug reconnected with no relaunch.
  - Step 4: `PASS` as written, Keith: after Apple menu Sleep and a wake, button 2 stepped the volume 19 -> 38% with no relaunch. But `pmset` shows DarkWake again (08:21:21 to 08:22:04, TCP keep alive on AC), so the CPU kept running and the board stayed connected: the wake reopen path (D-068) is still unexercised on hardware.
  - Step 3 (exit criterion): `PASS`, Keith. Open at Login on; the Mac booted at 08:25:06 and, after login with no action on Detent, it started at 08:27:13 with Keith's config (1 device, 5 mappings), connected the board and acked its HELLO within 75 ms. Button 2 then stepped the volume 38 -> 44 -> 50% (dispatch 0.3 ms).
  - M3 exit criteria both pass on a signed build. Keith's original config stays at `config.json.bak` (named `Desk controller`); the one in use was made by the wizard (`Detent Dev`) plus those mappings.
- Step 4 rerun, 2026-11-10, same build, lid closed about 3 minutes: `PASS` as written, Keith (button 2 stepped the volume 19 -> 38% after waking, no relaunch). Timeline: the HID link dropped at 08:46:47 as the lid closed (`Link(Disconnected)`), sleep at 08:46:58, wake at 08:50:03, the board reconnected through the normal rescan at 08:50:23, and the press came at 08:50:52. `pmset` logged it as DarkWake on AC power again, so the clock gap path (D-068) still didn't run: macOS dropped the link before sleeping, and recovery came through the replug path. Why the reconnect came 20 s after the wake isn't known (the login screen, or USB taking that long to come back).

## M4

Needs a signed build (`just build-dev`) with at least one paired board on release firmware. Steps 1 and 2 are the exit criteria.

1. Exit criterion: three mappings without reading. Open Detent, go to Mappings, and with no docs make three mappings on the paired board: button 1 opens Calculator, a long press of button 3 sets the volume to 25%, and button 4 runs `display notification "Detent"` as AppleScript. Each works on a press, and the log shows it.
2. Exit criterion: Dry run. Go to Test. Dry run is on. Press each mapped button: its control lights up and the screen shows the action that would run for the frontmost app, marked as a dry run, but nothing happens (no app opens, the volume doesn't move, no notification) and the log shows each as recorded, not run. Turn Dry run off: a press runs for real. Switch to Devices and back: Dry run is on again; close the window: presses run normally.
3. Handled by the device. In the drawing the encoder and its push switch show as handled by the device (Volume mode) and can't be edited.
4. Starter template. Pair the second board with the starter template box ticked. Its mappings appear on Mappings (D-077), and its button 2 steps the volume.
5. Edits stick. Change one mapping, remove another, and relaunch Detent: Mappings shows the same set, and an app scoped mapping (from the config or the template) still shows its badge until removed.

Results:
- Early check of task 4, 2026-11-10, signed build at `26100e2` on Keith's config:
  - Step 4: `PASS`, Keith. The second board (`E072A1FC849C`) paired through the wizard as `Detent Dev 2` (suggested name, D-073) with the template box ticked. Detent logged `config reloaded: 2 device(s), 5 mapping(s)` at pairing, then `2 device(s), 10 mapping(s)` on Finish. Its button 2 stepped the volume 38 -> 44 -> 50% (dispatch 0.4 and 3.4 ms).
- Early check of task 6, 2026-11-10, signed builds from `0c2559a` to `10b4f0b` on Keith's config, board `Detent Dev 2` (`E072A1FC849C`):
  - Step 1 (exit criterion): `PASS` with notes, Keith's call. He made three working mappings himself: button 1 opens Plasticity (`open_app com.electron.plasticity`, dispatch about 1 ms), a long press of button 3 turns the volume up by 25% (13 -> 38%), and button 4 ran `display notification "Detent"` (finished in 128 ms). Notes: (1) the AppleScript text had to be given to him; (2) he picked "up by 25%" where the step said "set to 25%": the labels "Volume up / Step" read as setting a level, so they became "Turn volume up by…" / "Set volume to…"; and the drawing ran press and long press together ("hold:"), so each key now shows labelled "Press:" and "Long press:" lines. His first try saved nothing and left no trace in the log; refused edits are now logged. Volume actions show no macOS HUD, so a change can't be seen (M5 item; the Test screen will show results).
- Early check of task 7, 2026-11-10, signed build of the Test screen on Keith's config, board `Detent Dev 2`:
  - Step 2 (exit criterion): `PASS`, Keith; every part confirmed in the log by Claude. Opening Test logged `dry run on`. Button 1 (Plasticity), button 2 (volume up), a long press of button 3 (volume up 25%) and button 4 (Finder script) were each resolved and logged `recorded (dry run)`, and none ran. Unticking Dry run logged `dry run off`, and button 2 then ran for real (38 -> 44%). Returning to Test turned it on again each time and leaving turned it off; after closing the window, button 2 ran for real (69 -> 75%). Repeated `dry run off` lines (asking for the state it was already in) are no longer logged.
  - Step 3: the drawing on Test and Mappings shows the encoder and its push switch dashed and unclickable as handled by the device; Keith saw this in the task 5 check ("Looks right"), not yet recorded as a pass.
- Acceptance pass, 2026-11-10, signed `Detent.app` from `just build-dev` at `a37a4f2` (Mac Developer identity), both boards on release firmware, Keith's config (backed up first). Steps 1, 2 and 4 passed early on builds from `26100e2` to `a37a4f2` (above); this pass covered the rest. Keith did each step; Claude read the log and config.
  - Step 3: `PASS`, Keith: on Mappings the encoder and its push switch show dashed and can't be edited.
  - Step 5: `PASS`, Keith. Button 2 on `Detent Dev 2` changed to volume up by 10% was saved (`config reloaded`) and was there after a quit and relaunch (`volume_up` step 0.1 in the loaded config); button 4 was removed after the relaunch and stays removed in the saved config. With `Detent Dev` plugged in, its Safari mapping on button 1 showed its badge.
  - Dry run re-check: entering Test logged `dry run on` and button 2 was `recorded (dry run)`. Leaving Test wasn't repeated in this pass (it passed in step 2).
  - M4 exit criteria both pass on a signed build.

## M5

Needs a signed build (`just build-dev`), a paired board on release firmware, and Plasticity installed. Steps 2 and 3 are the exit criteria. Step 1 starts from a reset grant: `tccutil reset Accessibility com.gittielabs.detent`.

1. Accessibility. With no Send shortcut mapping, Mappings shows no banner. Save one: a banner says Detent needs Accessibility to send shortcuts, with a Fix button. Fix shows macOS's prompt and opens Privacy & Security > Accessibility. Turn Detent on there: the banner goes away within a few seconds, with no relaunch.
2. Exit criterion: app scope. On Mappings, button 3 does Mute or unmute everywhere. Add a second mapping to it, only in Plasticity, that sends Plasticity's Extrude shortcut (record it in the shortcut field). The drawing shows the Plasticity badge. With a face selected in Plasticity and Plasticity in front, button 3 starts Extrude; with any other app in front it mutes. The log shows which mapping resolved each time.
3. Exit criterion: Secure Input. Map a button to send a plain letter everywhere. In TextEdit the press types the letter. Click into a password field (for example Safari's or Passwords' sign in field) and press it: nothing is typed, the Test screen shows the press as failed because Secure Input is on, and the log says the same. Where Send shortcut is picked, the editor says in one line that shortcuts go to the front app and that password fields block them.
4. Volume feedback. Button 2 (volume up) shows Detent's level overlay for about a second near the bottom of the screen with the pointer, with the new level; a long press set to 25% shows 25% and the volume is exactly 25% (Test shows "was …, now 25%"). Clicks pass through the overlay, and it needs no permission.
5. Dry run never sends keys. On Test with Dry run on, press the shortcut button with TextEdit in front: nothing is typed and Test shows it as not run.
6. Overview. Mappings lists the board's controls one row each, grouped (Buttons, then Encoder), with a cell per trigger. Mapped triggers show an Everywhere chip and one chip per app; hovering a chip says what it does, and clicking it opens that mapping in the panel. The encoder is one grey row (handled by the device). The list reads the same at the window's default size.

Results:
- Early checks of task 4, 2026-11-10, signed build of the shortcut field on Keith's config, board `Detent Dev` (`E072A1FC4EDC`), after `tccutil reset Accessibility com.gittielabs.detent`:
  - Step 1: `PASS`, Keith. No banner with no shortcut mapping; after recording Shift+H on button 4 and saving, the Accessibility banner showed; Fix brought up macOS's prompt and the Settings pane, and the banner cleared after the grant with no relaunch. No shortcut was refused for the grant afterwards.
  - Step 3 (exit criterion): `PASS`, Keith; confirmed in the log by Claude. In TextEdit button 4 logged `sent shift+h` (dispatch 115 ms the first time, 1.5 ms after). In the Passwords app it logged `FAILED Secure Input is on (a password field has focus), so macOS blocks sent keys` three times, nothing typed; the Test screen showed the failure with Dry run off. The editor's one line note is under the shortcut field.
  - Step 5, partly: with Dry run on, button 4 logged `recorded (dry run)` (in Passwords, not yet in TextEdit).
- Early check of task 5, 2026-11-10, signed build of app scope in the editor on Keith's config, board `Detent Dev`:
  - Step 2 (exit criterion): `PASS`, Keith; confirmed in the log and config by Claude. Button 3 press holds two mappings: Everywhere `mute_toggle`, and only in `com.electron.plasticity` `send_shortcut e`. With Plasticity in front the press resolved to `send_shortcut e` (sent, dispatch 2.6 ms) and started Extrude; in Notes and Chrome it resolved to `mute_toggle` (muted, unmuted).
- Early checks of tasks 6 and 7, 2026-11-10, signed builds of the overview list and the volume overlay on Keith's config, board `Detent Dev`:
  - Step 4: `PASS`, Keith. Button 2 in TextEdit showed Detent's level overlay near the bottom of the screen and it faded after about a second; five presses stepped 69 -> 75 -> 81 -> 88 -> 94 -> 100% (log); typing stayed in TextEdit, so the overlay doesn't take focus. The mute variant wasn't exercised (no `mute_toggle` press logged on these builds).
  - Step 6: `PASS`, Keith, on the list layout (D-085) after two rounds of his feedback: fixed tiles with a Preview as picker (D-084) were replaced, then the row dividers squared off and the window made a fixed frame. Claude also checked a fake 24 button device in the browser pane: tabs and headers stay, the Buttons group scrolls in its box while Encoder stays visible, the panel stays in view, and scroll positions survive re-renders (a bug found and fixed there).
- Acceptance pass, 2026-11-10, signed `Detent.app` from `just build-dev` at `b5fdfc7` (Mac Developer identity), board `Detent Dev` on release firmware, Keith's config. Steps 1 to 4 and 6 passed early on builds from `ee132d4` to `b5fdfc7` (above); this pass finished step 5 and the overlay's mute case, and re-checked step 2. Keith did each step; Claude read the log.
  - Step 5: `PASS`, Keith. On Test with Dry run on and TextEdit in front, button 4 twice logged `send_shortcut shift+h: recorded (dry run)` and nothing was typed; with Dry run off the next press logged `sent shift+h`.
  - Step 4, mute: `PASS`, Keith. Button 3 in TextEdit logged `muted` then `unmuted`; the overlay showed Muted, then the level.
  - Step 2 re-check: `PASS`. Button 3 in Plasticity logged `send_shortcut e: sent e`.
  - M5 exit criteria both pass on a signed build.

## M6

Needs a signed build (`just build-dev`), a paired board on release firmware with M6's changes (flashed with Keith's OK), and a second display for steps 4 and 5, which are the exit criteria.

1. Device menu. On Mappings, add two Device menu items: "Mail" opens Mail, "Mute" toggles mute. Long press the encoder: the OLED menu lists Volume, Mapped, Setup, Device info, then Mail and Mute. Select Mail: Mail opens. Move Mute above Mail in Detent: within a second the OLED order follows. Unplug and replug: the items come back.
2. Status line. Press a mapped button: the OLED's status screen shows a short result (for example `Vol 38%` or `Muted`) for a few seconds, then clears.
3. Mapped mode. Switch the board to Mapped from its menu. Within a few seconds, without replugging, Mappings shows the encoder's Turn left, Turn right and Push as editable. Map Turn right to volume up by 2%: each detent raises the volume 2%. Switch back to Volume: the encoder shows as handled by the device again and turns change the volume as before.
4. Exit criterion: dashboard. Map a button to Open dashboard on the secondary display. A press shows the dashboard full screen on that display, without taking focus from the app in front; a second press hides it. Moving the mouse over it shows a close button that hides it. Unplugging the paired device hides it (Close when no device is connected, on by default). Set the mode to Window: the page opens in a titled, resizable window centred on that display, and its close button hides it.
5. Exit criterion: display unplugged. With the dashboard showing on the secondary display, unplug that display. Detent doesn't crash, and the dashboard moves to the main display (or hides) rather than being stranded off screen. Plug it back in: the next press shows it there again.

Results:
- Early checks of tasks 2 and 3, 2026-11-10, signed builds on Keith's config, `Detent Dev` on the M6 firmware:
  - Step 3: `PASS`, Keith. Switching the board to Mapped and back logged `device Mode` Menu, Mapped, Menu, Volume with no replug, and Mappings followed. In Mapped mode he mapped Turn left and Turn right to volume down and up by 10% (one action per detent in the log) and Push to mute (`button 6 press -> mute_toggle`); the push's long press still opened the device menu. His first try couldn't reach Push: empty cells weren't clickable, now fixed.
- Early check of tasks 4 and 5, 2026-11-10, signed builds on Keith's config, `Detent Dev` on the M6 firmware:
  - Step 1: `PASS`, Keith. Two items put in the config by hand showed after the device's own, and picking them ran them (`menu "Louder" -> volume_up 6%`, 65 -> 71%; `menu "Mail" -> open_app com.apple.mail`). In the editor he added Notes, picked it on the device (`menu "Notes" -> open_app com.apple.Notes`), and removed it again; each edit reached the device. His first reorder exposed a UI bug: after Move up the panel reloaded its form from the item's old position before following it, so the next Save wrote Mail over Louder (the menu became Mail, Mail; restored by hand). Fixed so the selection moves before the redraw and the form reloads whenever its item changes; on the rebuilt app Move up then Save left Louder, Mail (two saves, both items intact), and the device's menu followed. The probe's leftover Mute was gone (the clear on connect).
- Early check of task 6, 2026-11-10, signed build on Keith's config, `Detent Dev`:
  - Step 2: `PASS`, Keith. After presses the OLED's status screen showed the result (volume level, Muted and Unmuted, Opened Mail from the device menu) and cleared after a few seconds.
- Early checks of task 7, 2026-11-10, signed builds on Keith's config, `Detent Dev`, Studio Display and the MacBook's screen:
  - Steps 4 and 5 (exit criteria): `PASS`, Keith. Button 4 showed the repo page full screen on the chosen display without taking focus (`open_dashboard ...: dashboard shown`), a second press hid it, and unplugging the Studio Display moved it to the main screen without a crash. After his review the close control became a large bottom right "Close" pill shown while the mouse moves, which he saw and used, and the page now sits below the menu bar. Window mode and Close when no device is connected were built after that round and haven't been tried on screen yet.
- Acceptance pass, 2026-11-10, signed `Detent.app` from `just build-dev` at `741c01d` plus the mode switch fix below, `Detent Dev` on the M6 firmware, Keith's config, Studio Display and the MacBook's screen. Keith did each step; Claude read the log.
  - Step 1: `PASS`. `menu "Louder" -> volume_up 6%` from the device's menu.
  - Step 2: `PASS`. Button 2 stepped 50 -> 56% and the OLED showed the result.
  - Step 3: `PASS`. Mode Menu, Mapped (turns stepped 10% each way: 56 -> 66 -> 56 -> 66 -> 56 -> 46%), Menu, Volume, with no replug.
  - Step 4 (exit criterion): `PASS`. Full screen below the menu bar, closed with the Close pill, shown again. Window mode first failed on the first press after switching ("this action runs only inside the Detent app", a misleading message): switching rebuilt one window label while macOS was still tearing the old one down. Fixed with a window per mode and a truthful error; then Window, Full screen and Window again each showed on the first press after the switch, and hid on the next.
  - Step 5 (exit criterion): `PASS` earlier (Studio Display unplugged, the dashboard moved to the main screen). Unplugging Detent Dev hid the dashboard (Close when no device is connected).
  - M6 exit criteria both pass on a signed build.

## M7

Needs a signed build (`just build-dev`) and each board flashed with its release firmware (Keith's OK for each flash), wired with buttons, an encoder and the OLED per its pin map. Steps 1 to 3 are the exit criteria.

1. DevKitC. Plug it in alone: the tray offers "New device found", and the wizard pairs it with its model's name. Mappings shows its controls from HELLO. A mapped button runs, the device menu shows Detent's items after its own, a result shows on the status line, and Mapped mode follows from the board's menu. Its RGB LED blinks unpaired and turns solid when paired.
2. Pico. The same as step 1, on the Pico. Flashing it again from `just fw-upload` (or its own target) works without pressing BOOTSEL.
3. The XIAO, unchanged. Plug the XIAO back in: it connects as before with its mappings, device menu and status line, and `scripts/hil.sh` passes on it.
4. Protocol v2 (D-094). Before reflashing, plug in a XIAO still on v1 firmware (0.1.0): Devices says its firmware is too old and it stays connected (no reconnect loop in the log). After reflashing it with 0.2.0, it shows firmware 0.2.0, Mappings shows the knob with its Push cell on the same row, and in Volume mode both are marked as handled on the device.

Results:
- Early check of task 1, 2026-11-11, the Pico USB spike (`pico_spike`, flashed with Keith's OK from BOOTSEL), Keith's Raspberry Pi Pico with no headers:
  - `ioreg`: VID `0x1209`, PID `0x0001`, manufacturer GittieLabs, product Detent Dev, serial `E6654854571A6E2F`. `ctl-probe list` shows the vendor collection (0xFF00/0x01) and Consumer Control (0x000C) on interface 2, as on the XIAO.
  - `ctl-probe` with `DETENT_PROBE_IDS=1209:0001`: HELLO (fw 0.1.0, one button) right after connecting, then 45 heartbeats in 90 s; six BOOTSEL presses as `BUTTON 1 down` and `up` in order. Keith saw the Mac's volume rise with each press (Consumer Control).
  - Step 2 (the full Pico) waits on soldered headers.
- Step 4, 2026-11-11, signed build (`just build-dev`), XIAO `E072A1FC4EDC`:
  - On v1 firmware 0.1.0: the log shows `Outdated { version: 1 }` once and no disconnect over 12 s; Keith saw Devices say its firmware is too old.
  - Flashed to 0.2.0 with Keith's OK: it reconnected with HELLO naming button 6 as encoder 5's push; Keith checked firmware 0.2.0, the knob and its Push on one row, and both handled on the device in Volume mode. PASS.
  - `scripts/hil.sh` passed on it (debug and release; board left on release 0.2.0).
  - The second XIAO (`E072A1FC849C`) flashed to release 0.2.0 with Keith's OK; it reconnected with HELLO naming button 6 as encoder 5's push.

