# Install

1. Download `Detent-<version>.dmg` from the project's [Releases page](https://github.com/gittielabs/detent/releases).
2. Open it and drag **Detent** to **Applications**.
3. Open Detent from Applications. It's signed and notarized by Apple, so macOS opens it without a warning.

Each release also carries firmware for every supported board (`detent-fw-<board>-<version>.bin`, or `.uf2` for the Pico), and its notes say which firmware the app expects. See [Boards and firmware](boards.md) for flashing.

Detent has no Dock icon. Look for its knob in the menu bar; its menu shows:

- **The device line**: a green dot and the device's name when it's connected, white when none is, red when something's wrong (with the reason in the settings window).
- **New device found: Pair…** when an unpaired Detent device is plugged in.
- **Pause**: presses are logged but run nothing until you turn it off.
- **Open Detent**: the settings window.
- **Detent 0.1.0**: which version is running (*dev build* on one built from source with `just build-dev`). The settings window's title says the same.
- **Open at login**: starts Detent when you log in.
- **Quit**.

Closing the settings window keeps Detent running; only Quit stops it.

## Permissions

Detent asks for nothing up front. Two actions need a permission, and the app tells you when:

| Permission | Needed for | When |
| --- | --- | --- |
| Accessibility | Send shortcut | The first time you save a Send shortcut mapping, a banner offers **Fix**, which opens System Settings. |
| Automation | Run AppleScript | macOS asks the first time a script tells a particular app to do something. |

Detent doesn't need Input Monitoring: the device isn't a keyboard.

## Building it yourself

The app and firmware build from source on macOS with Rust, Node with pnpm, PlatformIO and the Xcode command line tools. The [README](https://github.com/gittielabs/detent#readme) has the steps.
