# Roadmap

Changes every session. This is the only place progress is tracked.

**Current milestone: M2 Headless engine**

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

## M1 Device speaks (done 2026-11-07)

Goal: firmware enumerates with VID, PID, serial, the vendor HID channel, and Consumer Control. It sends HELLO, BUTTON, ENCODER, HEARTBEAT and handles REQUEST_HELLO and HELLO_ACK. Encoder Volume mode sends Consumer Control; the OLED shows status and the firmware owned menu. Protocol codec in the `protocol` crate with property tests. `ctl-probe` opens the device by VID/PID and prints decoded events. `protocol.md` frozen at v1.

Riskiest unknown: the precompiled TinyUSB in Arduino-ESP32 2.0.17 (D-028) allows one HID interface (`CFG_TUD_HID = 1`), and the core's `USBHID` shares it between devices by report ID. The two interface, 64 byte, no report ID layout in `architecture.md` and `protocol.md` can't be built on it unchanged. Task 1 settles this on hardware before any codec work.

Tasks:

- [x] Spike: settle the USB HID layout on the XIAO. Option A, one HID interface with a vendor collection (usage page `0xFF00`, report ID 1, 63 byte reports) plus Consumer Control (report ID 2), using the core's `USBHIDVendor` and `USBHIDConsumerControl`. Throwaway firmware in `firmware/src/main.cpp` and a raw mode in `tools/ctl-probe` (hidapi: list interfaces with usage pages, dump raw reports). Answer: does hidapi on macOS open the vendor collection without an Input Monitoring prompt, do media keys still drive the volume while the probe holds the device open, does a report ID prefix show up on read and write. If A needs Input Monitoring, evaluate option B (two HID interfaces through ESP-IDF `esp_tinyusb`, or a TinyUSB app class driver) far enough to estimate it. Output: a decisions row; if the layout changes, one atomic commit of `protocol.md`, the `protocol` crate, and the header. Tests: HIL with the board on the build Mac, plus `acceptance.md` M1 step 1 for Keith (permission prompts).
- [x] Codec, device to app: `protocol/src/codec.rs` with a header parser (version, type, seq, len checks) and a `DeviceMessage` enum that decodes and encodes HELLO, BUTTON, ENCODER, MENU_SELECT, HEARTBEAT, with a typed error. Tests: unit test per message and per rejection rule (wrong version, `len` over max, unknown type, short payload), proptest that any input never panics and that encode then decode round trips.
- [x] Codec, app to device: `AppMessage` for HELLO_ACK, MENU, STATUS, REQUEST_HELLO, TEST_INJECT, with label and text validation (ASCII, max length, event count). Tests: unit tests per message and limit, proptest round trips and no panic.
- [x] Golden vectors and firmware report packing: extend `gen-header` to emit `firmware/test/vectors.h` (encoded bytes for each message from the Rust codec); C++ pack and parse in `firmware/lib/report/`; a PlatformIO `native` env running Unity tests that the C++ output matches the vectors byte for byte. Add `pio test -d firmware -e native` and the vectors `--check` to CI and `just ci`. Tests: native unit, the existing header drift test.
- [x] Firmware USB identity: the HID layout from the spike, serial number string from the chip MAC (12 hex chars), product string `Detent <model>` (placeholder model name), manufacturer GittieLabs, CDC kept for uploads. `firmware/lib/usb/` plus `main.cpp`. Tests: native test for the MAC to serial formatting; HIL with `ctl-probe --list` showing VID, PID, serial, and usage pages; acceptance step for `ioreg`.
- [x] `ctl-probe` CLI: open the paired channel by VID/PID and usage page only (never a keyboard), send REQUEST_HELLO on open, print each decoded event with timestamp and `--raw` hex, reopen after unplug, and a `--count-hellos N` mode that exits 0 after N replugs each produced a HELLO. `tools/ctl-probe/`. Uses hidapi directly; the app's `DeviceTransport` arrives in M2 (D-034). Tests: unit tests for argument parsing and event formatting against fixture reports; HIL.
- [x] Firmware link layer: `firmware/lib/link/` (built as `lib/link_layer`) sends HELLO on host mount, on REQUEST_HELLO, and after leaving Setup; HEARTBEAT every 2 s with uptime and encoder mode; per direction seq counter; paired flag from HELLO_ACK. Tests: native unit tests against a fake clock and fake transport (HELLO triggers, heartbeat timing, seq wrap at 255); HIL with `ctl-probe`.
- [x] TEST_INJECT and a HIL script (note: `pio run -t upload` from a running app sometimes exits 1 with "could not open port" after a verified flash, a race in the final reset; the script must check the device re-enumerates rather than trust the exit code): a debug env in `platformio.ini` that compiles TEST_INJECT handling; release ignores the type. `ctl-probe --inject <script>` sends events; `scripts/hil.sh` flashes debug firmware, injects a press sequence, and asserts the decoded output. Tests: native test that the release build drops TEST_INJECT; `scripts/hil.sh` run by Claude with the board attached.
- [x] **Fix (blocks M2): two host writes within about a second of boot reset the device.** Root cause: a TinyUSB IN stall in Arduino-ESP32 2.0.17. A device report in flight when a second host report lands never completes; sends stay broken and macOS resets the device (the CPU never rebooted). Fixed by moving to Arduino-ESP32 3.3.12 (D-045): same trigger, 2.x failed 2 of 2, 3.x passed 3 of 3; `scripts/hil.sh` with no settle wait passed 3 runs in a row. Also fixed a separate receive bug: the core's unread 256 byte queue truncated every host report after the fourth (D-046).
- [x] Firmware buttons and encoder input: `firmware/lib/input/` debounce, down, up, long press (fires once at 600 ms held, before up), quadrature decode, and 20 ms detent coalescing. Pins in `firmware/include/board_pins.h` marked PLACEHOLDER until Keith's wiring is known. Tests: native unit tests feeding simulated pin sequences (bounce, long press, fast turns); acceptance step pressing each control with `ctl-probe` running. Done 2026-11-07: 14 native tests. Flashed: `scripts/hil.sh` passed, HELLO lists the 6 placeholder controls, and 10 s with nothing wired gave no phantom BUTTON or ENCODER events. Real presses and turns wait on wiring (`acceptance.md` M1 step 5).
- [x] Encoder modes and Consumer Control: `firmware/lib/modes/` routes the encoder by mode. Volume sends Consumer Control volume steps and press toggles mute; Mapped sends ENCODER events; Menu drives the OLED menu. Mode reported in HELLO and HEARTBEAT. Tests: native unit tests for routing per mode; HIL for ENCODER in Mapped mode via TEST_INJECT; acceptance step for volume with `ctl-probe` closed (exit criterion). Done 2026-11-07: `lib/modes` with 10 native tests; TEST_INJECT now replays through the router. The extended `scripts/hil.sh` (volume, mute, long press to menu) is written but not yet run: flashing needs Keith's OK.
- [x] OLED status and firmware owned menu: `firmware/lib/display/` (built as `lib/menu` for state and text, `src/display.cpp` for drawing) for the SSD1306 over I2C; status line (connected, paired, mode), firmware menu (Volume, Mapped, Setup, Device info), Setup shows the serial and sends HELLO on exit, labels at most 16 chars. Pick the display library then and log it. Tests: native unit tests for the menu model (navigation, selection, truncation); acceptance step for what the screen shows. Done 2026-11-07: `lib/menu` with 12 native tests, U8g2 driver (D-050). On hardware: passed acceptance step 7 with Keith's display, all three modes and Setup.
- [x] Freeze protocol v1: reconcile `protocol.md` with the codec as built, drop "draft", regenerate the header and vectors, and run the M1 acceptance pass. Tests: full `just ci`; `acceptance.md` M1 steps. Done 2026-11-07: `protocol.md` reconciled (unique control IDs, nonzero delta, mode routing, PID 0x8FDE) and marked frozen; `protocol/tests/frozen.rs` pins the exact v1 bytes and fails on any wire change even after the vectors are regenerated (D-051). The acceptance pass is split out below.
- [x] M1 acceptance pass on release firmware: HIL PASS (`b7c213b`, board left on release); steps 3, 4, 6, 7 PASS; step 5 (buttons) and step 2's second unit check DEFERRED by Keith 2026-11-07 and carried into M2.

Exit criteria:

- [x] 20 unplug and replug cycles, a HELLO every time (`ctl-probe --count-hellos`, `acceptance.md`). Keith, 2026-11-07: 20 of 20.
- [x] Typing on the built in keyboard produces nothing in `ctl-probe` (manual, `acceptance.md`). Keith, 2026-11-07.
- [x] The encoder in Volume mode moves the macOS volume with `ctl-probe` closed (manual, `acceptance.md`). Keith confirmed 2026-11-07.
- [x] Codec rejects malformed reports without panicking (proptest in `cargo test`). Both directions, 2048 cases per property, 2026-11-07.

## M2 Headless engine (done 2026-11-09)

Goal: the app turns device events into Mac actions with no UI. The devices module opens allowlisted units and tracks heartbeats; the router picks one mapping per event (app scope, then Everywhere); three actions run (Open app, Volume, Run AppleScript; Open dashboard is deferred, D-055), all driven by a hand written JSON config. MockTransport and RecordingExecutor make the whole pipeline testable without hardware.

Riskiest unknown: the native macOS calls the actions need (NSWorkspace to launch apps, open URLs, and read the frontmost app; CoreAudio for volume and mute), all `unsafe` bridging in `macos/`, called from a background thread inside a running Tauri app. Task 1 proves them before anything depends on them.

Carried over from M1 (hardware Keith doesn't have yet; not M1 exit criteria):

- [x] Press each of the four buttons with `just probe` running and see BUTTON down, long press, and up (`acceptance.md` M1 step 5). Keith, 2026-11-09: all four pass on D0 to D3, no pin changes needed.
- [ ] Plug in a second board and confirm a different USB serial (`acceptance.md` M1 step 2). Waits on a second XIAO.


Tasks:

- [x] macOS bindings: in `app/src-tauri/src/macos/`, safe wrappers for the frontmost app's bundle ID and opening an app by bundle ID (NSWorkspace via the `objc2` crates), and default output volume get and set and mute get and set (CoreAudio). Called from a non main thread, as the engine will. Add a lib target (`app/src-tauri/src/lib.rs`) so engine modules are testable outside Tauri (D-052). Tests: macOS only unit tests (frontmost ID is non empty; volume set then restored; mute toggled then restored, skipped when there's no output device, as on CI); `acceptance.md` M2 step 1 (opens Mail, follows the frontmost app). Done 2026-11-09: `frontmost_bundle_id`, `open_app`, `volume`/`set_volume`, `muted`/`set_muted` in `macos/` (NSWorkspace calls need no `unsafe` with objc2; volume through the AudioObject API, D-056); lib target `detent_lib`; 4 unit tests, the audio one restoring the system state; `just macos-check` for acceptance step 1.
- [x] Config schema v1 and store: `app/src-tauri/src/store/` reads versioned JSON from `~/Library/Application Support/Detent/config.json`: allowlisted units (VID, PID, serial, name), and mappings of control plus trigger (button press or long press; encoder clockwise or counterclockwise), scope (Everywhere or an app bundle ID), and action (Open app, Volume up, down, set, mute, Run AppleScript; Open dashboard waits on D-055). Unknown fields, unknown versions, and bad values are errors with a path to the field. A commented example config in `app/src-tauri/examples/`. Tests: unit (good and bad samples, every validation rule, the example parses). Done 2026-11-09: `store::parse` and `store::load` with 17 tests; `examples/config.example.json` plus `examples/config.md` as the field reference (JSON can't hold comments); D-057.
- [x] Router: `app/src-tauri/src/router.rs` resolves an event (device, control, trigger) plus the frontmost bundle ID to at most one mapping: app scope beats Everywhere, otherwise nothing. Pure. Tests: table driven unit tests, including the acceptance scenario "app scope beats Everywhere; Everywhere used with no app match". Done 2026-11-09: `Router::new(&Config)` indexes mappings by (serial, control, trigger); `resolve` picks the app mapping for the frontmost bundle ID (case insensitive), else Everywhere, else nothing. `trigger_of` maps a decoded message to a trigger: button down is press, long press is long_press, release and non input messages trigger nothing, an encoder turn gives a direction and detent count. 9 tests in `tests/router.rs`; mutations (Everywhere wins, release triggers) both caught.
- [x] Actions: `app/src-tauri/src/actions/` with an `Action` trait and one executor per type (Open app, Volume, Run AppleScript through `osascript` with a timeout, D-053), each returning a result Debug can show; `RecordingExecutor` for tests and Dry run. Tests: unit for argument validation, the recording executor, and AppleScript timeout and error reporting (a harmless script); real effects in `acceptance.md`. Done 2026-11-09: `Executor` trait with `SystemExecutor` and `RecordingExecutor` (D-058); arguments checked by `Action::check_args`, shared with the config loader; volume actions unmute like the keys (D-059); AppleScript on stdin in its own process group, killed as a group on timeout (D-060). 15 tests in `tests/actions.rs`; mutations (no group kill, check after run) both caught.
- [x] Transport: `app/src-tauri/src/transport/` with the `DeviceTransport` trait, `UsbHid` over hidapi (non exclusive, report ID 6 in and out, foreign report IDs dropped, D-036), and a scripted `MockTransport`. Tests: unit through MockTransport (reports in and out, foreign IDs dropped, disconnect surfaces as an error); HIL: `UsbHid` reads a HELLO from the board. Done 2026-11-09: one `HidTransport` does the framing over a raw link; `UsbHid` runs it over hidapi (non exclusive), `MockTransport` over a scripted `MockHid` that can push reports from another thread and unplug (D-061). `Hid::channels` and `Hid::open` for the devices module. 10 unit tests in `tests/transport.rs` (dropping foreign IDs mutation caught). HIL `just hil-app` (then `hil-transport`) PASS on the board (`E072A1FC4EDC`, release firmware): opened non exclusive and read a HELLO with 5 buttons and the encoder in Volume mode.
- [x] Devices: `app/src-tauri/src/devices/` polls the device list every second, opens allowlisted units only (others listed, never read; ignore list honored), sends REQUEST_HELLO then HELLO_ACK paired, decodes events with the codec and tags them with a `DeviceId`, marks a unit disconnected after 6 s without HEARTBEAT, and reopens after replug. Tests: integration against MockTransport (attach, HELLO, presses, detach, garbage, heartbeat loss, an unknown device ignored, an ignored device stays ignored). Done 2026-11-09: `Devices::step(now)` over a `Bus` (`Hid` or `MockBus`), emitting Connected, Hello, Input, Rejected, Disconnected (link, detached, heartbeat), OpenFailed, and Unknown events tagged with `DeviceId` (VID, PID, serial) (D-062). 19 integration tests in `tests/devices.rs`; mutations (ignore list skipped, garbage counted as heard) caught. HIL `just hil-app` PASS on the board: connected, HELLO 9 ms after REQUEST_HELLO, acked, still up after 7 s on heartbeats. The LED and OLED showing paired is acceptance step 2.
- [x] Engine: wire devices, router (frontmost app from `macos`), and actions on a background thread in the app, started at launch from the config, with a log of each event, resolved mapping, action result, and press to dispatch time. Tray shows the device as connected or not. Tests: integration of the whole pipeline (MockTransport, RecordingExecutor, a fake frontmost provider), including dispatch time well under 100 ms; HIL: the debug firmware's TEST_INJECT drives the real app with Dry run. Done 2026-11-09: engine built (D-063): `Engine` plus a worker thread, log at `~/Library/Logs/Detent/detent.log`, tray device line, `--dry-run` and `--config` (D-064). 10 pipeline tests in `tests/engine.rs`, worst of 20 dispatches under 50 ms; inlining actions on the engine thread was caught. HIL PASS (see `acceptance.md` M2 results): debug firmware's TEST_INJECT drove the real app in dry run; every press resolved to its mapping, dispatch 0.2 to 0.7 ms, nothing run; board back on release firmware.
- [x] M2 acceptance pass on a signed build: `just build-dev`, a hand written config, and the steps in `acceptance.md` M2, ending with a real button opening Mail. Done 2026-11-09: signed `Detent.app` (Mac Developer identity), Keith's config, steps 1 to 8 PASS (see `acceptance.md` M2 results). Found and fixed on the way: `macos-check frontmost` read a stale app (no run loop; `macos::run_loop_for`), and step 5's script `activate` alone opens no Finder window and needs no Automation permission (script now counts windows, reopens if none, activates).

Exit criteria:

- [x] Router and codec unit tests pass; integration tests pass against MockTransport. Passed 2026-11-09 in CI (`3bc5a3b`): router, codec, transport, devices, and engine pipeline tests.
- [x] A real button opens Mail (signed build, `acceptance.md` M2). Keith, 2026-11-09: quit Mail, pressed button 1, Mail opened (step 6).

## M3 App shell and pairing (current)

Goal: Detent is a real background app. It starts at login, survives sleep and wake, and a new device is paired from the Devices screen through a short wizard, with no hand written config. Tray app, launch at login, sleep and wake recovery, Devices screen, pairing wizard, ignore list, persistence with schema version.

Riskiest unknown: launch at login and wake recovery on a signed build. Whether `tauri-plugin-autostart` really uses SMAppService (as `product.md` assumes) or a LaunchAgent, and whether hidapi handles survive sleep or must be reopened. Both only show up on Keith's Mac, so they go first.

Carried over (hardware Keith doesn't have yet; not an M3 exit criterion):

- [ ] Plug in a second board and confirm a different USB serial (`acceptance.md` M1 step 2). Waits on a second XIAO.

Tasks:

- [x] Login item and wake recovery: launch at login through SMAppService (check what `tauri-plugin-autostart` uses; call `SMAppService.mainApp` from `macos/` if it doesn't, decisions row either way), a tray toggle for it, and a wake hook (NSWorkspace did wake notification, in `macos/`) that makes `devices` close and reopen every unit at once instead of waiting out the 6 s silence. Tests: devices integration test for the wake reopen (MockBus); unit test for the toggle state; `acceptance.md` M3 steps 3 and 4 (reboot, sleep and wake) on a signed build. Done 2026-11-09: the plugin offers only a LaunchAgent or AppleScript, so SMAppService is called directly (`macos::login_item`, `set_login_item`, D-067); tray check item Open at Login (refreshed on hover). Wake detected by the wall clock outrunning `Instant` (D-068); `Devices::woke` closes every unit and rescans in the same step. Tests: 2 devices tests (mutation dropping the immediate rescan caught), sleep gap unit test, tray label test, read only status test. Manual: `acceptance.md` M3 steps 3 and 4, on a signed build. Early check on Keith's Mac: Open at Login works after fixing `NotFound` (a never registered bundle) to mean off; a DarkWake sleep kept the board connected; the full sleep reopen path is still untested.
- [x] Config writes and live reload: `store::save` serializes the config and writes it atomically (temp file, then rename), keeping field order and the v1 schema; the engine takes a new config without a relaunch, keeping connected units that are still allowed and closing ones that aren't. Tests: round trip (parse, save, parse equal), a failed write leaves the old file intact, engine integration test reloading a config that pairs, unpairs, and ignores units. Done 2026-11-09: `store::to_json` and `store::save` (validated, canonical, temp then rename, D-069); `Devices::set_lists` and `Engine::reload` / `EngineHandle::reload` (new mappings at once, `Lost::Unpaired` for units no longer allowed, new units opened at once, others untouched). 6 store tests, 4 devices tests, 2 engine tests; mutation dropping the ignore check on reload caught.
- [x] Pairing identification (D-066): while the wizard is open, `devices` may open unpaired Detent units (Detent VID and PID only, not ignored) read only, send REQUEST_HELLO but never HELLO_ACK, and report each one's HELLO and first BUTTON, closing them when the wizard ends. Nothing they send reaches the router. Tests: devices integration tests (candidate identified by a press, never routed, closed on wizard end, ignored units still never opened, a paired unit unaffected). Done 2026-11-09: `Devices::start_identify` / `end_identify` and `EngineHandle::identify`; candidates report `CandidateFound`, `CandidateHello`, `CandidatePressed` (a button down, or a HELLO after the first, which is leaving Setup), and `CandidateGone`; pairing one closes its read only handle and the rescan opens it paired. 9 devices tests and 1 engine test; mutation treating candidates as paired caught (3 tests). HIL on the board, treated as unpaired: found, HELLO read, never connected or routed.
- [x] App state and IPC: a Rust `AppState` the engine sink updates (paired units with name, model, firmware, connected; unknown units; ignored units; last problem) and Tauri commands for the UI (get state, start and end pairing, pair with a name, rename, unpair, ignore, unignore; pause moved to the Tray task, which owns its persistence), each writing the config through `store::save` and reloading the engine; state changes pushed to the UI as one event. The settings window gets IPC; nothing else does (dashboards stay without it, D-009). Tests: unit tests for `AppState` updates from records and for each command's config edit. Done 2026-11-09: `detent_lib::app_state` (`AppState::new`, `apply`, `set_config`, `set_identifying`; edits `pair`, `rename`, `unpair`, `mappings_of`, `ignore`, `unignore`, D-070) and `src/commands.rs` (9 commands, allow listed for the settings window only, D-071), one `state` event per change. Device events now carry the USB product string (model) and report `UnknownGone`. 15 app_state tests, 1 devices test, a capability check test. Not yet driven from a UI (task 5).
- [ ] Devices screen: `app/ui/` in plain TypeScript (D-065). Paired units with name, model, firmware, and a live connected dot; rename and unpair; the Ignored list with un-ignore; Pair new device. Window closes to hide (already true). Tests: `tsc --noEmit`; the logic stays in Rust and is tested there; `acceptance.md` M3 step 1.
- [ ] Pairing wizard: the four steps in `product.md` (plug in and list candidates, press a button to pick one, name it prefilled from the model, done with the starter template checkbox) plus Ignore on a candidate. Starter template: PLACEHOLDER until Keith decides (open question). Tests: `tsc --noEmit`; `acceptance.md` M3 step 2 (fresh device pairs in under 2 minutes, timed).
- [ ] Tray: status dot (connected, none, problem) in the menu, Pause toggle (input is logged but nothing runs; survives relaunch; the engine pause and a Tauri command land here, moved from the App state task), a "new device found" line that opens the wizard, Open Detent, Quit. Tests: unit tests for the labels and the engine's pause; `acceptance.md` M3 step 5.
- [ ] M3 acceptance pass on a signed build: `just build-dev`, an empty config, and the steps in `acceptance.md` M3.

Exit criteria:

- [ ] A fresh device pairs in under 2 minutes (signed build, `acceptance.md` M3 step 2).
- [ ] After reboot the device works with no user action (signed build, `acceptance.md` M3 step 3).

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

The Open dashboard action: the app's own dashboard page (content to be decided, D-055) full screen in an app window on a chosen display, app owned OLED menu items via MENU and MENU_SELECT, Mapped encoder mode, STATUS messages.

Exit criteria:

- One button shows the app's dashboard full screen on the secondary display (what it shows is an open question, D-055).
- Unplugging that display doesn't crash or strand the window.

## M7 Hardening and v0.1

Debug mode and log export, crash recovery, config migration tests, notarized release build.

Noted during M2 (do here, not before):

- [ ] An AppleScript's first run waits on macOS's Automation prompt, and that wait counts against its timeout (5 s by default): in M2 step 5 the first press took 4.6 s. A slow click on Allow stops the script (the grant still sticks). Don't count prompt time, or retry once after a timeout on the first run against an app. Found 2026-11-09.

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
- [x] Homepage container URL, once it's running. Not needed: Keith dropped the Homepage use on 2026-11-09 (D-055).
- [x] M2's summary said "Open URL", which isn't in the fixed v1 action list. Keith, 2026-11-09: defer it; Open dashboard should open the app's own dashboard instead (D-055).
- [ ] What should the app's dashboard show? It's the target of the Open dashboard action (a web page shown in an app window, M6). Until decided, Open dashboard isn't built and `product.md` still describes it as a URL.
- [ ] Starter template for the pairing wizard's last step: which mappings a fresh `Detent Dev` gets when the box is ticked (for example button 1 Open Mail, 2 and 3 volume, 4 mute, encoder volume). Placeholder in M3: an empty template, with the checkbox shown but explained as "nothing yet".
- [ ] Pin wiring on the first device: which XIAO pins carry each button, the encoder A, B, and switch, and the OLED's I2C. `firmware/include/board_pins.h` uses a PLACEHOLDER map until then.
- [ ] Model name for the USB product string `Detent <model name>`. Placeholder `Detent Dev`.
- [ ] `protocol.md` says changes need a version bump, but v1 is a draft until the end of M1. May the draft change at version 1 (still one atomic commit of doc, crate, and header) until the freeze? Placeholder: yes, version stays 1 until the freeze. Moot since the 2026-11-07 freeze: the placeholder was followed, and every change from now on bumps the version. Keith to confirm and tick.
- [ ] File the Espressif usb-pids PR once the repo is public. Unblocked: the repo is public and Apache-2.0. Keith files it (it's a PR from his GitHub account to `espressif/usb-pids`).

## Session log

Newest first. One or two lines per session: what landed, what's next.

- 2026-11-09: M2 done. Landed the router, actions (system and recording executors), transport (UsbHid, MockTransport), devices (pairing, heartbeat, replug), and the engine with its log and tray line; Keith passed acceptance steps 1 to 8 on a signed build, including a real button opening Mail. Fixed on the way: the frontmost check tool's stale reading and step 5's Finder script. Next: plan M3. Waiting on Keith: the second board serial check (needs a second XIAO).
- 2026-11-07 (end): M1 done. Landed buttons and encoder input, encoder modes with Consumer Control volume and mute, the OLED status screen and menu, debug input tracing, and the v1 protocol freeze guarded by `frozen.rs`; all four exit criteria passed on Keith's hardware and the board is on release `b7c213b`. Next: `/plan-milestone` for M2 (headless engine). Waiting on Keith: tactile buttons for M1 step 5 and a second XIAO for step 2's serial check, both carried into M2; confirm the moot draft-version open question.
- 2026-11-07 (later): TEST_INJECT and `scripts/hil.sh` landed and found a USB stall; root caused to Arduino-ESP32 2.0.17 and fixed by moving to 3.3.12 on PlatformIO 6.2 (D-045), plus a receive queue fix (D-046). Next: buttons and encoder input. Keith can now run the 20 replug exit criterion.
- 2026-11-07: M1 spike done. Option A (one HID interface, vendor report ID 6 with 63 byte reports, Consumer Control report ID 4) works on the XIAO with no Input Monitoring; protocol, crate, and header updated together (D-036). Next: codec, device to app.
- 2026-11-04 (later): M0 exit criteria both pass. Keith installed a Mac Developer cert and passed acceptance steps 1 to 6; firmware steps 7 to 9 are still to run. Next: `/plan-milestone` for M1.
- 2026-11-04: All nine M0 tasks landed; CI green and repo public. M0 closes when Keith passes `acceptance.md` M0 steps 1 to 6 (needs an Apple Development certificate first). Then `/plan-milestone` for M1. License is now urgent (repo public, no LICENSE).
- 2026-11-04: Plan written in claude.ai and split into these docs. Next: `/start-task` on the first M0 task.
