# Detent

A macOS menu bar app and ESP32-S3 firmware for building your own physical controllers. Buttons and an encoder on a USB device map to Mac actions: open an app, show a web dashboard full screen, send a shortcut to a specific app, change the volume. Only devices you pair are listened to, so your keyboard never triggers anything.

Status: pre-alpha, M0. See `docs/roadmap.md`.

## Layout

- `app/` Tauri 2 app (Rust backend, TypeScript UI)
- `firmware/` PlatformIO project for ESP32-S3
- `protocol/` Rust crate defining the USB wire format; the C header for firmware is generated from it
- `tools/ctl-probe/` CLI that prints decoded device events
- `docs/` product, architecture, protocol, decisions, roadmap, acceptance

## Building (macOS)

Requirements: Rust stable, Node with pnpm, PlatformIO CLI, Xcode command line tools, and an Apple Development signing identity.

Find your identity, set it, and build:

```
security find-identity -v -p codesigning
export APPLE_SIGNING_IDENTITY="Apple Development: Your Name (TEAMID)"
scripts/build-dev.sh        # or: just build-dev
```

The script builds a debug `Detent.app` at `target/debug/bundle/macos/Detent.app`, signs it, and checks the signature. It stops with a clear message if `APPLE_SIGNING_IDENTITY` is unset, set to `-` (ad hoc), or doesn't match an identity in your keychain.

Dev builds are signed with a stable identity so macOS permission grants (Accessibility) survive rebuilds. Ad hoc signatures change every build and reset those grants. Keep the identity in your shell profile or environment; it never goes in the repo.

Other tasks: `just fmt`, `just lint`, `just test`, `just fw`. Run `just` to list them.

## License

TBD.
