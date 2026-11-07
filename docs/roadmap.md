# Roadmap

Changes every session. This is the only place progress is tracked.

**Current milestone: M1 Device speaks**

Milestones are ordered by risk: macOS signing and permissions, then the USB protocol, are proven before any UI exists. The engine runs headless before it gets a window. The next milestone starts only when every exit criterion of the current one passes on a signed build.

## M0 Foundations (done 2026-11-04)

Goal: a signed, tray only Tauri app in a monorepo with CI, and proof that TCC grants survive rebuilds.

Tasks:

- [x] Create the Cargo workspace with members `protocol`, `app/src-tauri`, `tools/ctl-probe`. Add `rust-toolchain.toml` (stable), `.gitignore`, `rustfmt.toml`, and a `justfile` with `fmt`, `lint`, `test`, `build-dev`, `fw` targets.
- [x] Scaffold the Tauri 2 app in `app/` with a TypeScript UI (Vite, no UI framework decision yet; plain TS is fine for M0). Bundle identifier `com.gittielabs.detent` (placeholder until Keith confirms). Tray only: `LSUIElement` true, no window on launch, tray menu with "Accessibility: granted / not granted", "Open Detent" (stub window), "Quit".
- [x] Add `app/src-tauri/src/macos/` with an `accessibility_trusted()` wrapper around `AXIsProcessTrusted`, shown in the tray menu. This is the probe for the TCC exit criterion.
- [x] `scripts/build-dev.sh`: builds and signs the app with `APPLE_SIGNING_IDENTITY` from the environment, fails clearly if it's unset. Document usage in `README.md`.
- [x] Create the `protocol` crate with `PROTOCOL_VERSION`, message type constants, VID, and a placeholder PID constant in `src/ids.rs` (pick an unused value in `0x8000..0x8FFF`, mark it `// PLACEHOLDER: replace with Espressif usb-pids allocation`). No codec yet.
- [x] Create the PlatformIO project in `firmware/` targeting ESP32-S3 with Arduino-ESP32, TinyUSB enabled. Board is `seeed_xiao_esp32s3` unless Keith says otherwise. It only needs to build and blink in M0.
- [x] `scripts/gen-header.sh` stub plus a test that will compare generated header and crate constants (can assert trivially for now). Replace the temporary VID/PID copy in `firmware/src/main.cpp` with the generated header.
- [x] GitHub Actions on `macos-latest`: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, `pnpm -C app tsc --noEmit`, `pio run -d firmware`.
- [x] Write the M0 steps in `acceptance.md` for Keith.

Exit criteria:

- [x] Two consecutive signed rebuilds keep the Accessibility grant without re prompting (manual, `acceptance.md`). Keith passed it on 2026-11-04 with build `147ecce`.
- [x] CI green on main. First green run 41943577075 on 2026-11-05 (private repo `gittielabs/detent`).

## M1 Device speaks (current)

Goal: firmware enumerates with VID, PID, serial, the vendor HID channel, and Consumer Control. It sends HELLO, BUTTON, ENCODER, HEARTBEAT and handles REQUEST_HELLO and HELLO_ACK. Encoder Volume mode sends Consumer Control; the OLED shows status and the firmware owned menu. Protocol codec in the `protocol` crate with property tests. `ctl-probe` opens the device by VID/PID and prints decoded events. `protocol.md` frozen at v1.

Riskiest unknown: the precompiled TinyUSB in Arduino-ESP32 2.0.17 (D-028) allows one HID interface (`CFG_TUD_HID = 1`), and the core's `USBHID` shares it between devices by report ID. The two interface, 64 byte, no report ID layout in `architecture.md` and `protocol.md` can't be built on it unchanged. Task 1 settles this on hardware before any codec work.

Tasks:

- [x] Spike: settle the USB HID layout on the XIAO. Option A, one HID interface with a vendor collection (usage page `0xFF00`, report ID 1, 63 byte reports) plus Consumer Control (report ID 2), using the core's `USBHIDVendor` and `USBHIDConsumerControl`. Throwaway firmware in `firmware/src/main.cpp` and a raw mode in `tools/ctl-probe` (hidapi: list interfaces with usage pages, dump raw reports). Answer: does hidapi on macOS open the vendor collection without an Input Monitoring prompt, do media keys still drive the volume while the probe holds the device open, does a report ID prefix show up on read and write. If A needs Input Monitoring, evaluate option B (two HID interfaces through ESP-IDF `esp_tinyusb`, or a TinyUSB app class driver) far enough to estimate it. Output: a decisions row; if the layout changes, one atomic commit of `protocol.md`, the `protocol` crate, and the header. Tests: HIL with the board on the build Mac, plus `acceptance.md` M1 step 1 for Keith (permission prompts).
- [x] Codec, device to app: `protocol/src/codec.rs` with a header parser (version, type, seq, len checks) and a `DeviceMessage` enum that decodes and encodes HELLO, BUTTON, ENCODER, MENU_SELECT, HEARTBEAT, with a typed error. Tests: unit test per message and per rejection rule (wrong version, `len` over max, unknown type, short payload), proptest that any input never panics and that encode then decode round trips.
- [x] Codec, app to device: `AppMessage` for HELLO_ACK, MENU, STATUS, REQUEST_HELLO, TEST_INJECT, with label and text validation (ASCII, max length, event count). Tests: unit tests per message and limit, proptest round trips and no panic.
- [ ] Golden vectors and firmware report packing: extend `gen-header` to emit `firmware/test/vectors.h` (encoded bytes for each message from the Rust codec); C++ pack and parse in `firmware/lib/report/`; a PlatformIO `native` env running Unity tests that the C++ output matches the vectors byte for byte. Add `pio test -d firmware -e native` and the vectors `--check` to CI and `just ci`. Tests: native unit, the existing header drift test.
- [ ] Firmware USB identity: the HID layout from the spike, serial number string from the chip MAC (12 hex chars), product string `Detent <model>` (placeholder model name), manufacturer GittieLabs, CDC kept for uploads. `firmware/lib/usb/` plus `main.cpp`. Tests: native test for the MAC to serial formatting; HIL with `ctl-probe --list` showing VID, PID, serial, and usage pages; acceptance step for `ioreg`.
- [ ] `ctl-probe` CLI: open the paired channel by VID/PID and usage page only (never a keyboard), send REQUEST_HELLO on open, print each decoded event with timestamp and `--raw` hex, reopen after unplug, and a `--count-hellos N` mode that exits 0 after N replugs each produced a HELLO. `tools/ctl-probe/`. Uses hidapi directly; the app's `DeviceTransport` arrives in M2 (D-034). Tests: unit tests for argument parsing and event formatting against fixture reports; HIL.
- [ ] Firmware link layer: `firmware/lib/link/` sends HELLO on host mount, on REQUEST_HELLO, and after leaving Setup; HEARTBEAT every 2 s with uptime and encoder mode; per direction seq counter; paired flag from HELLO_ACK. Tests: native unit tests against a fake clock and fake transport (HELLO triggers, heartbeat timing, seq wrap at 255); HIL with `ctl-probe`.
- [ ] TEST_INJECT and a HIL script (note: `pio run -t upload` from a running app sometimes exits 1 with "could not open port" after a verified flash, a race in the final reset; the script must check the device re-enumerates rather than trust the exit code): a debug env in `platformio.ini` that compiles TEST_INJECT handling; release ignores the type. `ctl-probe --inject <script>` sends events; `scripts/hil.sh` flashes debug firmware, injects a press sequence, and asserts the decoded output. Tests: native test that the release build drops TEST_INJECT; `scripts/hil.sh` run by Claude with the board attached.
- [ ] Firmware buttons and encoder input: `firmware/lib/input/` debounce, down, up, long press (fires once at 600 ms held, before up), quadrature decode, and 20 ms detent coalescing. Pins in `firmware/include/board_pins.h` marked PLACEHOLDER until Keith's wiring is known. Tests: native unit tests feeding simulated pin sequences (bounce, long press, fast turns); acceptance step pressing each control with `ctl-probe` running.
- [ ] Encoder modes and Consumer Control: `firmware/lib/modes/` routes the encoder by mode. Volume sends Consumer Control volume steps and press toggles mute; Mapped sends ENCODER events; Menu drives the OLED menu. Mode reported in HELLO and HEARTBEAT. Tests: native unit tests for routing per mode; HIL for ENCODER in Mapped mode via TEST_INJECT; acceptance step for volume with `ctl-probe` closed (exit criterion).
- [ ] OLED status and firmware owned menu: `firmware/lib/display/` for the SSD1306 over I2C; status line (connected, paired, mode), firmware menu (Volume, Mapped, Setup, Device info), Setup shows the serial and sends HELLO on exit, labels at most 16 chars. Pick the display library then and log it. Tests: native unit tests for the menu model (navigation, selection, truncation); acceptance step for what the screen shows.
- [ ] Freeze protocol v1: reconcile `protocol.md` with the codec as built, drop "draft", regenerate the header and vectors, and run the M1 acceptance pass. Tests: full `just ci`; `acceptance.md` M1 steps.

Exit criteria:

- [ ] 20 unplug and replug cycles, a HELLO every time (`ctl-probe --count-hellos 20`, `acceptance.md`).
- [ ] Typing on the built in keyboard produces nothing in `ctl-probe` (manual, `acceptance.md`).
- [ ] The encoder in Volume mode moves the macOS volume with `ctl-probe` closed (manual, `acceptance.md`).
- [x] Codec rejects malformed reports without panicking (proptest in `cargo test`). Both directions, 2048 cases per property, 2026-11-07.

## M2 Headless engine

Devices module with allowlist and heartbeat, router, and four actions (Open app, Open URL, Volume, AppleScript), driven by a hand written JSON config. MockTransport and RecordingExecutor.

Exit criteria:

- Router and codec unit tests pass; integration tests pass against MockTransport.
- A real button opens Mail.

## M3 App shell and pairing

Tray app, launch at login, sleep and wake recovery, Devices screen, pairing wizard, ignore list, persistence with schema version.

Exit criteria:

- A fresh device pairs in under 2 minutes.
- After reboot the device works with no user action.

## M4 Mappings and Test

Device drawing from HELLO, mapping editor, Device menu editor (MENU messages), Test screen with Dry run.

Exit criteria:

- Keith creates three mappings without reading anything.
- Dry run never executes an action.

## M5 App scope and shortcuts

Only in app scope, Send shortcut action via CGEvent, permission banner with Fix button.

Exit criteria:

- One button is Extrude in Plasticity and Mute everywhere else.
- Secure Input behavior verified and stated in the UI.

## M6 Dashboard and encoder menu

Full screen dashboard window on a chosen display, app owned OLED menu items via MENU and MENU_SELECT, Mapped encoder mode, STATUS messages.

Exit criteria:

- One button shows Homepage full screen on the secondary display.
- Unplugging that display doesn't crash or strand the window.

## M7 Hardening and v0.1

Debug mode and log export, crash recovery, config migration tests, notarized release build.

Exit criteria:

- Full manual acceptance pass with zero blockers.
- Notarized build installs and runs on a clean user account.

## After v0.1

Multiple connected devices, Bluetooth transport, then Windows.

## Open questions (Keith's call)

- [x] GitHub and trademark check on "Detent" before the repo goes public. Keith made the repo public on 2026-11-04 without the check; rename the app and repo later if the name doesn't hold up (D-032).
- [ ] Bundle identifier: `com.gittielabs.detent` or another domain.
- [x] License for app and firmware: MIT or Apache 2.0. Apache-2.0, matching GittieLabs' copperplane (D-033).
- [ ] Button count on the first device. Doesn't block app work; HELLO reports it.
- [ ] Exact ESP32-S3 board (assumed XIAO ESP32-S3).
- [ ] App and tray icons are placeholders (a knob outline in `app/src-tauri/icons/`). Final artwork before v0.1.
- [ ] Homepage container URL, once it's running.
- [ ] Pin wiring on the first device: which XIAO pins carry each button, the encoder A, B, and switch, and the OLED's I2C. `firmware/include/board_pins.h` uses a PLACEHOLDER map until then.
- [ ] Model name for the USB product string `Detent <model name>`. Placeholder `Detent Dev`.
- [ ] `protocol.md` says changes need a version bump, but v1 is a draft until the end of M1. May the draft change at version 1 (still one atomic commit of doc, crate, and header) until the freeze? Placeholder: yes, version stays 1 until the freeze.
- [ ] File the Espressif usb-pids PR once the repo is public. Unblocked: the repo is public and Apache-2.0. Keith files it (it's a PR from his GitHub account to `espressif/usb-pids`).

## Session log

Newest first. One or two lines per session: what landed, what's next.

- 2026-11-07: M1 spike done. Option A (one HID interface, vendor report ID 6 with 63 byte reports, Consumer Control report ID 4) works on the XIAO with no Input Monitoring; protocol, crate, and header updated together (D-036). Next: codec, device to app.
- 2026-11-04 (later): M0 exit criteria both pass. Keith installed a Mac Developer cert and passed acceptance steps 1 to 6; firmware steps 7 to 9 are still to run. Next: `/plan-milestone` for M1.
- 2026-11-04: All nine M0 tasks landed; CI green and repo public. M0 closes when Keith passes `acceptance.md` M0 steps 1 to 6 (needs an Apple Development certificate first). Then `/plan-milestone` for M1. License is now urgent (repo public, no LICENSE).
- 2026-11-04: Plan written in claude.ai and split into these docs. Next: `/start-task` on the first M0 task.
