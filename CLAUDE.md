# Detent

Detent is an open source macOS menu bar app (Tauri 2, Rust backend, TypeScript UI) plus ESP32-S3 firmware for a USB controller device. The device reports button presses and encoder turns over a vendor defined HID interface; the app maps them to Mac actions. Repo: `gittielabs/detent`. Owner: Keith Elliott.

## Read before you work

Every session, before writing code:

1. `docs/product.md` for intent, scope, and non goals. These rarely change.
2. `docs/architecture.md` for how the pieces fit.
3. The **current milestone** section of `docs/roadmap.md`. Only work on tasks listed there.

Also read `docs/protocol.md` whenever you touch firmware, the `protocol/` crate, transport, or device manager code. Skim `docs/decisions.md` before proposing anything that might contradict an earlier decision.

## Rules

- **Stay in the current milestone.** If a task reveals work needed elsewhere, add it to `docs/roadmap.md` under the right milestone and stop. Don't do it now.
- **Protocol changes are atomic.** Never edit `docs/protocol.md` without bumping `PROTOCOL_VERSION`, updating the Rust crate in `protocol/`, and regenerating the C header in `firmware/include/detent_protocol.h`, all in one commit. A test checks the header matches the crate.
- **Log decisions.** Any choice between two reasonable options gets a new row in `docs/decisions.md` with what was rejected and why. The log is append only; a changed decision gets a new row that supersedes the old one.
- **Don't fake hardware or permissions.** Anything that needs a physical button press, a macOS permission dialog, sleep and wake, or a signed build on a clean account goes into `docs/acceptance.md` as a step for Keith. Never mark those passed yourself.
- **Signing secrets stay out of the repo.** Signing identity and team ID come from env vars (`APPLE_SIGNING_IDENTITY`, `APPLE_TEAM_ID`). Never commit them or a provisioning profile.
- **Complexity guardrails are real.** Three screens maximum (Devices, Mappings, Test). The action list in `docs/product.md` is fixed for v1. A new action type needs a decisions row justifying it.
- **No `unsafe` outside `app/src-tauri/src/macos/`.** All objc and CoreGraphics bridging lives in that one module.
- **Ask, don't guess, on open questions.** Anything listed under Open questions in `docs/roadmap.md` is Keith's call. Use a clearly marked placeholder and keep going.

## Definition of done for a task

- Tests for the change exist and pass (`cargo test --workspace`).
- `cargo fmt --check` and `cargo clippy --workspace -- -D warnings` are clean.
- `pnpm -C app tsc --noEmit` passes if UI code changed.
- `pio run -d firmware` builds if firmware changed.
- The task is ticked in `docs/roadmap.md`.
- A decisions row was added if a choice was made.

A milestone is done when every exit criterion in `docs/roadmap.md` passes on a **signed** build, including the manual steps in `docs/acceptance.md`.

## Commands

- `/plan-milestone` breaks the next milestone into concrete tasks in `docs/roadmap.md`.
- `/start-task` picks up the next open task, restates it with its tests, then builds it.
- `/wrap-session` ticks finished tasks, logs decisions, and writes what's next.

## Layout

```
detent/
  app/                  Tauri 2 app (src-tauri = Rust, ui = TypeScript)
  firmware/             PlatformIO project, ESP32-S3, Arduino-ESP32 + TinyUSB
  protocol/             Rust crate: wire format, codec, constants. Source of truth.
  tools/ctl-probe/      CLI that opens the device and prints decoded events
  scripts/              build-dev.sh, gen-header.sh, etc.
  docs/                 product, architecture, protocol, decisions, roadmap, acceptance
```

## Tooling

Rust stable, Node with pnpm, PlatformIO CLI, Xcode command line tools. Keith works CLI first (tmux, Neovim), so prefer scripts and `just` targets over IDE steps.
