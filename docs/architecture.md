# Architecture

Changes at milestones. Update this when a module's job or boundary changes.

## The principle

The device is dumb and the app owns meaning. The device reports what was pressed; the app decides what that does. Firmware flashes stay rare, and one app can support many different controllers.

```
ESP32-S3 device                         Detent app (Tauri 2, Rust)
+--------------------+                  +-----------------------------------------------+
| Vendor HID  -------+---------------->| Transport -> Devices -> Router -> Actions -----+--> macOS
|  (app traffic)     |                  |                          ^         |           |   (apps, AppleScript,
|                    |                  |   UI (3 screens) -> Store+         v           |    keystrokes, CoreAudio,
| Consumer Control --+--- media keys ---+------------------------------>  Dashboard win  |    frontmost app)
|  (volume, mute)    |   (no app needed)+-----------------------------------------------+
+--------------------+
```

## Device

**Hardware (first device).** ESP32-S3, a 0.96 inch 128x64 OLED (SSD1306 class, I2C), one push encoder, and some number of buttons. The count isn't fixed; HELLO reports whatever exists and the app adapts. Firmware is PlatformIO (Core 6.2+) with Arduino-ESP32 3.3 (pioarduino platform, D-045) and TinyUSB.

**USB.** Composite device: one HID interface with two top level collections, plus a CDC serial interface for uploads and logs (D-036). The Arduino core's HID class puts every HID device on one interface (established on 2.0.17, D-036, and unchanged in our use of 3.3), so the collections share it and are told apart by report ID:

1. Vendor defined HID (usage page `0xFF00`, report ID 6), 63 byte reports, carrying all app traffic. See `protocol.md`.
2. Standard Consumer Control (report ID 4) for volume up, down, and mute.

VID is Espressif's `0x303A`. The PID comes from Espressif's free usb-pids program (D-014); until that PR merges, firmware uses the placeholder in `protocol/src/ids.rs`. **Never use TinyUSB or IDF default PIDs** (for example `0x4001`); they collide with stock firmware.

The USB serial number string is derived from the chip's MAC address, so two identical devices are distinguishable.

**Controls.** `firmware/include/board_pins.h` is the one place that maps controls to pins, and HELLO reports exactly its table (a PLACEHOLDER until the wiring is settled). Buttons are polled and debounced in the main loop; the encoder is decoded from pin change interrupts so turns survive a blocked loop, and detents within 20 ms go out as one ENCODER delta. The encoder's push switch is its own button control.

**Status LED** (the XIAO's user LED): off with no host, blinking once a second while mounted but not paired, solid once the app has sent HELLO_ACK paired.

**Encoder modes** (switched from the OLED menu, owned by firmware):

- **Volume.** Turns send Consumer Control volume steps; a press toggles mute. Runs entirely on the device.
- **Menu.** Turns scroll the OLED menu; a press selects.
- **Mapped.** Turns are sent to the app as ENCODER events and mapped like any control (for example zoom or scrub in Plasticity). The push switch goes to the app as a BUTTON.

A long press of the encoder's push switch opens the OLED menu from any mode, and another long press closes it, back to the mode it came from. In Volume mode, mute toggles on release, so a long press never mutes (D-048). The device boots in Volume mode.

**OLED menu.** A firmware owned part (Volume, Mapped, Setup, Device info) plus an app owned part (apps and dashboards) that the app sends as MENU messages. Setup puts the device in pairing mode and shows its serial so the wizard can confirm the right unit. With no app connected, only the firmware owned part shows. Labels are at most 16 characters.

## Identity

- **PID** identifies the device *model*.
- **VID + PID + serial** identifies the *unit*. Mappings attach to a unit.
- A model can ship a starter template, so a second unit of the same model pairs with sensible defaults.

Every event inside the app carries a `DeviceId` from day one, even though v1 supports one connected device. That keeps multi device a UI change, not a refactor.

## App modules (`app/src-tauri/src/`)

1. **`transport`** A `DeviceTransport` trait. `UsbHid` implementation over the `hidapi` crate. A `MockTransport` for tests. Bluetooth later is another implementation; nothing above this changes. Note: on macOS, open the vendor entry non exclusive (hidapi's default is exclusive, which swallows the volume keys), and every read and write carries the leading report ID `0x06`; drop reads with any other ID.
2. **`devices`** Watches attach and detach (poll the device list once a second unless an IOKit notification path proves reliable), checks the allowlist of paired units and the ignore list, opens paired units, tracks HEARTBEAT (6 s without one means disconnected), and tags events with a `DeviceId`. Unknown devices raise a "new device found" tray badge and nothing else.
3. **`router`** Takes an event plus the frontmost app's bundle ID (NSWorkspace, no permission needed) and resolves exactly one mapping: app scope first, then Everywhere, else nothing.
4. **`actions`** One executor per action type behind an `Action` trait. Each returns a result the Test and Debug views can show. A `RecordingExecutor` for tests and Dry run.
5. **`store`** Versioned JSON at `~/Library/Application Support/Detent/config.json`. Schema migrations from v1 onward. No database.
6. **`windows`** The settings window and dashboard windows. A dashboard window loads its action's URL full screen (borderless maximized if native full screen fights Spaces) on the chosen display.
7. **`macos`** The only module allowed `unsafe`. Accessibility check (`AXIsProcessTrusted`), CGEvent keystroke posting, NSWorkspace frontmost app and app launch, CoreAudio volume.

## Background behavior

Menu bar app with no Dock icon (`LSUIElement`). Closing the settings window hides it; Quit lives only in the tray menu. Launch at login via SMAppService (`tauri-plugin-autostart`). The devices module reopens paired units after sleep, wake, and replug without user action.

## Security

- Dashboard windows load remote content and get **no** Tauri IPC capabilities. A remote page can never call into Rust.
- Only paired units are opened. Unknown devices are listed, never read.
- TEST_INJECT is compiled only into debug firmware; release firmware ignores it.

## Permissions

| Permission | Needed for | When asked |
| --- | --- | --- |
| Accessibility | Send shortcut | First time a Send shortcut mapping is saved |
| Automation (per target app) | Run AppleScript | macOS prompts on first run against that app |
| Input Monitoring | Not needed | Vendor HID isn't a keyboard device |

The app checks permissions at startup and shows missing ones as a single banner with a Fix button, never a modal on launch.

## UX: three screens and a tray menu

**Tray menu.** Device status dot, Pause toggle, Open Detent, Quit.

**Devices.** Paired device with name, model, firmware version, live connected dot. An Ignored list. Pair new device starts the wizard:

1. Plug in your device. Lists newly seen candidates with VID `0x303A` that aren't paired or ignored.
2. Press any button on the one you want (or pick Setup on its OLED). Proves which physical device it is.
3. Name it. Prefilled from the model name.
4. Done. Offer the model's starter template with one checkbox.

**Mappings.** A drawing of the device built from its HELLO control list. Click a control to edit it in a side panel: action from the fixed list, one or two fields, scope (Everywhere or an app picked from running and recent apps). App scoped overrides show a badge. Pass through controls are greyed. Below the drawing, a **Device menu** list edits the OLED's app owned items: label plus one action, drag to reorder, firmware owned items shown greyed so the user sees the whole menu.

**Test.** The same drawing, live. Pressing a control highlights it and shows the mapping that would fire for the current frontmost app. **Dry run** is on by default and executes nothing; turn it off to execute for real.

**Debug** (a toggle inside Test, or launch with `--debug`). Scrolling event log with timestamps, decoded fields and raw bytes, the resolved mapping and action result per event, a permissions panel, the last HELLO, and Export logs (zip).
