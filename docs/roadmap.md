# Roadmap

Changes every session. This is the only place progress is tracked.

**Current milestone: M0 Foundations**

Milestones are ordered by risk: macOS signing and permissions, then the USB protocol, are proven before any UI exists. The engine runs headless before it gets a window. The next milestone starts only when every exit criterion of the current one passes on a signed build.

## M0 Foundations (current)

Goal: a signed, tray only Tauri app in a monorepo with CI, and proof that TCC grants survive rebuilds.

Tasks:

- [x] Create the Cargo workspace with members `protocol`, `app/src-tauri`, `tools/ctl-probe`. Add `rust-toolchain.toml` (stable), `.gitignore`, `rustfmt.toml`, and a `justfile` with `fmt`, `lint`, `test`, `build-dev`, `fw` targets.
- [x] Scaffold the Tauri 2 app in `app/` with a TypeScript UI (Vite, no UI framework decision yet; plain TS is fine for M0). Bundle identifier `com.gittielabs.detent` (placeholder until Keith confirms). Tray only: `LSUIElement` true, no window on launch, tray menu with "Accessibility: granted / not granted", "Open Detent" (stub window), "Quit".
- [x] Add `app/src-tauri/src/macos/` with an `accessibility_trusted()` wrapper around `AXIsProcessTrusted`, shown in the tray menu. This is the probe for the TCC exit criterion.
- [ ] `scripts/build-dev.sh`: builds and signs the app with `APPLE_SIGNING_IDENTITY` from the environment, fails clearly if it's unset. Document usage in `README.md`.
- [ ] Create the `protocol` crate with `PROTOCOL_VERSION`, message type constants, VID, and a placeholder PID constant in `src/ids.rs` (pick an unused value in `0x8000..0x8FFF`, mark it `// PLACEHOLDER: replace with Espressif usb-pids allocation`). No codec yet.
- [ ] Create the PlatformIO project in `firmware/` targeting ESP32-S3 with Arduino-ESP32, TinyUSB enabled. Board is `seeed_xiao_esp32s3` unless Keith says otherwise. It only needs to build and blink in M0.
- [ ] `scripts/gen-header.sh` stub plus a test that will compare generated header and crate constants (can assert trivially for now).
- [ ] GitHub Actions on `macos-latest`: `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, `pnpm -C app tsc --noEmit`, `pio run -d firmware`.
- [ ] Write the M0 steps in `acceptance.md` for Keith.

Exit criteria:

- [ ] Two consecutive signed rebuilds keep the Accessibility grant without re prompting (manual, `acceptance.md`).
- [ ] CI green on main.

## M1 Device speaks

Firmware enumerates with VID, PID, serial, and both HID interfaces. Sends HELLO, BUTTON, ENCODER, HEARTBEAT; handles REQUEST_HELLO and HELLO_ACK. Encoder Volume mode sends Consumer Control; the OLED shows status and the firmware owned menu. Protocol codec in the `protocol` crate with property tests. `ctl-probe` CLI opens the device by VID/PID and prints decoded events. `protocol.md` frozen at v1.

Exit criteria:

- 20 unplug and replug cycles, a HELLO every time.
- Typing on the built in keyboard produces nothing in `ctl-probe`.
- The encoder in Volume mode moves the macOS volume with `ctl-probe` closed.
- Codec rejects malformed reports without panicking (proptest).

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

- [ ] GitHub and trademark check on "Detent" before the repo goes public.
- [ ] Bundle identifier: `com.gittielabs.detent` or another domain.
- [ ] License for app and firmware: MIT or Apache 2.0.
- [ ] Button count on the first device. Doesn't block app work; HELLO reports it.
- [ ] Exact ESP32-S3 board (assumed XIAO ESP32-S3).
- [ ] App and tray icons are placeholders (a knob outline in `app/src-tauri/icons/`). Final artwork before v0.1.
- [ ] Homepage container URL, once it's running.
- [ ] File the Espressif usb-pids PR once the repo is public.

## Session log

Newest first. One or two lines per session: what landed, what's next.

- 2026-11-04: Plan written in claude.ai and split into these docs. Next: `/start-task` on the first M0 task.
