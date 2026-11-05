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

1. Set `APPLE_SIGNING_IDENTITY` to your Apple Development identity (`security find-identity -v -p codesigning`).
   If that lists no "Apple Development" identity (true on the build Mac as of 2026-11-04), create one first: Xcode, Settings, Accounts, select your Apple ID, Manage Certificates, +, Apple Development.
2. Run `scripts/build-dev.sh` and launch the app. Confirm a tray icon appears and no Dock icon.
   Click the tray icon, choose Open Detent, and close the window that opens. The tray icon stays and the app keeps running. Choose Quit Detent; the tray icon disappears.
3. Tray menu shows "Accessibility: not granted". Grant it in System Settings, Privacy & Security, Accessibility. Relaunch; tray shows "granted".
   Also check without relaunching: after granting or revoking, click the tray icon again; the line should update.
4. Make a trivial code change, rebuild with `scripts/build-dev.sh`, relaunch. Tray still shows "granted" with no new prompt.
5. Repeat step 4 once more.

Results:

- (none yet)
