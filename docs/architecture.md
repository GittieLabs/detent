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

VID is Espressif's `0x303A`. The PID comes from Espressif's free usb-pids program (D-014); until that PR merges, firmware uses the placeholder in `protocol/src/ids.rs`. **Never use TinyUSB or IDF default PIDs** (for example `0x4001`); they collide with stock firmware. Each board build with its own IDs is a model in `ids::MODELS` (D-093); the app's `devices` treats any model as a Detent unit, and `docs/boards.md` lists them. A board's controls, including which button is each encoder's push switch, come from its HELLO (D-094); the model only picks the display name and starter template.

The USB serial number string is derived from the chip's MAC address, so two identical devices are distinguishable.

**Controls.** Each board's header in `firmware/include/boards/` (chosen through `include/board_pins.h`) is the one place that maps controls to pins: its buttons, and any number of encoders (none, one, several), each with its A and B pins and an optional push switch that is one of its buttons (D-096). A layout HELLO couldn't report fails to compile. `lib/layout` turns the tables into HELLO's controls and answers which encoder owns a push switch. Buttons are polled and debounced in the main loop; encoders are decoded from pin change interrupts (one handler for all) so turns survive a blocked loop, and detents within 20 ms go out as one ENCODER delta. HELLO names each encoder's push switch (protocol v2, D-094).

**Status LED** (the XIAO's user LED): off with no host, blinking once a second while mounted but not paired, solid once the app has sent HELLO_ACK paired.

**Encoder modes** (switched from the OLED menu, owned by firmware):

- **Volume.** Turns send Consumer Control volume steps; a press toggles mute. Runs entirely on the device.
- **Menu.** Turns scroll the OLED menu; a press selects.
- **Mapped.** Turns are sent to the app as ENCODER events and mapped like any control (for example zoom or scrub in Plasticity). The push switch goes to the app as a BUTTON.

A long press of the encoder's push switch opens the OLED menu from any mode, and another long press closes it, back to the mode it came from. In Volume mode, mute toggles on release, so a long press never mutes (D-048). The device boots in Volume mode.

**OLED menu.** A firmware owned part (Volume, Mapped, Setup, Device info) plus an app owned part (apps and dashboards) that the app sends as MENU messages (M6). Setup puts the device in pairing mode and shows its serial so the wizard can confirm the right unit. With no app connected, only the firmware owned part shows. Labels are at most 16 characters. Outside the menu the screen shows the model, the encoder mode, and USB state. `lib/menu` keeps the menu state and renders text lines (natively tested); `src/display.cpp` draws them with U8g2, skipping all drawing if no display answers on I2C.

## Identity

- **PID** identifies the device *model*.
- **VID + PID + serial** identifies the *unit*. Mappings attach to a unit.
- A model can ship a starter template, so a second unit of the same model pairs with sensible defaults.

Every event inside the app carries a `DeviceId` from day one, even though v1 supports one connected device. That keeps multi device a UI change, not a refactor.

## Firmware modules (`firmware/`)

Logic lives in pure C++11 libraries under `lib/` with caller supplied time and pins, tested natively (`just fw-test`, D-035); `src/` holds the thin Arduino glue.

- `lib/report`: pack device to app messages, strictly parse app to device ones; tested byte for byte against golden vectors generated from the Rust codec (D-039).
- `lib/link_layer`: HELLO, HEARTBEAT, REQUEST_HELLO, HELLO_ACK, seq, pairing (D-042).
- `lib/input`: button debounce and long press, quadrature decoding and detent coalescing (D-047).
- `lib/modes`: what the encoders and their push switches do in Volume, Mapped, and Menu modes (D-048); every encoder follows the one mode, and a long press of any push switch opens the menu (D-096).
- `lib/layout`: a board's buttons and encoders, which encoder owns a push switch, and what HELLO lists.
- `lib/menu`: OLED screens and the firmware owned menu, rendered as text lines (D-050).
- `lib/injector`: TEST_INJECT replay, enabled only in the debug env (D-043).
- `lib/usb_identity`: serial from the base MAC, product strings (D-040).
- `src/usb_device`: the composite USB device and its receive path (D-036, D-046); `src/display`: draws `lib/menu`'s lines with U8g2; `src/diag`: debug only boot, USB, and input diagnostics (D-049); `src/main.cpp`: wiring.
- `include/board_pins.h` selects the board by the env's `-DDETENT_BOARD_*` flag: one header per board in `include/boards/` (XIAO ESP32-S3, Adafruit ESP32-S3 Feather, ESP32-S3 DevKitC-1, Raspberry Pi Pico) holds its pins, status LED kind, OLED pins, and any switched I2C power pin. `src/platform.h` holds the per chip interrupt attribute and lock; the USB glue is per chip behind `src/usb_device.h` (`src/esp32/`, `src/pico/`, D-092). `include/detent_protocol.h` is generated from the `protocol` crate.

Builds per board: `xiao_esp32s3`, `feather_esp32s3` and `devkitc_esp32s3` (each with a `_debug` variant adding TEST_INJECT, diagnostics, and the input trace), `pico`, and `pico_spike` (USB only, no pins). `scripts/hil.sh` exercises the XIAO's on hardware.

## App modules (`app/src-tauri/src/`)

The engine modules are a library (`detent_lib`, `src/lib.rs`) that `main.rs`, the Tauri app, uses, so integration tests reach them without starting Tauri (D-052).

1. **`transport`** A `DeviceTransport` trait (read a report with a timeout, write a report). One `HidTransport` does the framing over any raw HID link: `UsbHid` over the `hidapi` crate, `MockTransport` over a scripted link, so tests run the real framing (D-061). A `Bus` trait lists attached vendor channels and opens them (`Hid`, `MockBus`). Bluetooth later is another implementation; nothing above this changes. Note: on macOS, open the vendor entry non exclusive (hidapi's default is exclusive, which swallows the volume keys), and every read and write carries the leading report ID `0x06`; drop reads with any other ID.
2. **`devices`** One synchronous `Devices::step(now)` (D-062): polls the device list once a second, checks the allowlist of paired units and the ignore list (ignore wins), opens paired units, sends REQUEST_HELLO and acks each HELLO, tracks silence (6 s without a valid report means disconnected; the next scan reopens), and tags events with a `DeviceId` (VID, PID, serial). Unknown devices raise a "new device found" tray badge and nothing else.
3. **`router`** Takes an event plus the frontmost app's bundle ID (NSWorkspace, no permission needed; AppKit refreshes it only while the process's main run loop runs, which the app's does) and resolves exactly one mapping: app scope first, then Everywhere, else nothing.
4. **`actions`** An `Executor` trait runs a config `Action` (D-058): `SystemExecutor` has one function per action type, `RecordingExecutor` runs nothing, for tests and Dry run. Arguments are checked before anything runs, by the rules the config loader uses. Each returns a result the Test and Debug views can show.
4a. **`engine`** Wires the above on background threads (D-063): the engine thread steps `devices`, resolves each input with `router`, and queues actions to one worker thread that runs them in order, so a slow script never stalls input or heartbeats. Every event, resolution, and result goes to a `Sink`: the app logs to `~/Library/Logs/Detent/detent.log` and drives the tray's device line. Crash recovery (D-102): a panic hook logs every panic with a backtrace; a panicking action fails only that action, and a panic in an engine turn is caught, every unit is reopened, and the loop carries on after a backoff (250 ms doubling to 30 s while panics repeat). The tray's device line then says it recovered.
4b. **`app_state`** What the UI shows (`AppState`: paired, candidate, unknown, and ignored units, the problem line), folded from engine records, plus the config edits behind the settings window's commands (D-070). Pure and tested; `src/commands.rs` in the app is a thin Tauri layer over it, allow listed for the settings window only (D-071).
4c. **`apps`** The app picker's list: running apps plus bundles in the usual app folders, merged by bundle ID (D-080). Pure apart from `list()`; running apps come from `macos`.
5. **`store`** Versioned JSON at `~/Library/Application Support/Detent/config.json`. Schema migrations from v1 onward. No database.
6. **`windows`** The settings window, the volume overlay (`hud.rs`: borderless, transparent, click through, not focusable, no IPC capabilities; shown about a second after a volume action, D-083), and the dashboard window (`dashboard.rs`: a web page sized to cover the chosen display, borderless, not focusable, no IPC capabilities, toggled by Open dashboard, refitted to the main display if its own is unplugged, D-088). A dashboard window loads its action's URL full screen (borderless maximized if native full screen fights Spaces) on the chosen display.
7. **`macos`** The only module allowed `unsafe`. Accessibility check (`AXIsProcessTrusted`), CGEvent keystroke posting, NSWorkspace frontmost app and app launch, CoreAudio volume, SMAppService login item.

## Background behavior

Menu bar app with no Dock icon (`LSUIElement`). Closing the settings window hides it; Quit lives only in the tray menu. Launch at login via SMAppService, called directly from `macos/` (D-067); after a wake from sleep the devices module reopens every unit at once (D-068). The devices module reopens paired units after sleep, wake, and replug without user action.

## Security

- Dashboard windows load remote content and get **no** Tauri IPC capabilities. A remote page can never call into Rust.
- Only paired units are opened. Unknown devices are listed, never read.
- TEST_INJECT is compiled only into debug firmware; release firmware ignores it.

## Permissions

| Permission | Needed for | When asked |
| --- | --- | --- |
| Accessibility | Send shortcut | First time a Send shortcut mapping is saved |
| Automation (per target app) | Run AppleScript | macOS prompts on first run against that app; that run's timeout is stretched while it might prompt (D-106) |
| Input Monitoring | Not needed | Vendor HID isn't a keyboard device |

The app checks permissions at startup and shows missing ones as a single banner with a Fix button, never a modal on launch.

## UX: three screens and a tray menu

**Tray menu.** Device status dot, Pause toggle, Open Detent, Quit.

**Devices.** Paired device with name, model, firmware version, live connected dot. An Ignored list. Pair new device starts the wizard:

1. Plug in your device. Lists newly seen candidates with VID `0x303A` that aren't paired or ignored.
2. Press any button on the one you want (or pick Setup on its OLED). Proves which physical device it is.
3. Name it. Prefilled from the model name.
4. Done. Offer the model's starter template with one checkbox.

**Mappings.** The device's controls from its HELLO, one row each, grouped by type, with a cell per trigger holding a chip per scope (Everywhere, each app) whose hover text says what fires (D-085). Click a row or chip to edit it in a side panel: action from the fixed list, one or two fields, scope (Everywhere or an app picked from running and recent apps). App scoped overrides show a badge. Pass through controls are greyed. Below the drawing, a **Device menu** list edits the OLED's app owned items: label plus one action, drag to reorder, firmware owned items shown greyed so the user sees the whole menu.

**Test.** The same drawing, live. Pressing a control highlights it and shows the mapping that would fire for the current frontmost app. **Dry run** is on by default and executes nothing; turn it off to execute for real.

**Debug** (a toggle inside Test, or launch with `--debug`). Scrolling event log with timestamps, decoded fields and raw bytes, the resolved mapping and action result per event, a permissions panel, the last HELLO, and Export logs (zip). User documentation lives in `site/` (mdBook) and is published to GitHub Pages; it's Open dashboard's default page (D-108). The raw bytes come from a tap in `devices` that's on only while Debug is (D-104); the log keeps the newest 500 entries in `app_state` and raw reports never go to `detent.log`.
