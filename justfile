# Detent tasks. Run `just` to list them.

default:
    @just --list

# Format all Rust code
fmt:
    cargo fmt --all

# Check formatting and run clippy with warnings as errors
lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings

# Run all Rust tests
test:
    cargo test --workspace

# Build and sign a dev build (needs APPLE_SIGNING_IDENTITY)
build-dev:
    scripts/build-dev.sh

# Release build signed with Developer ID, as a DMG (local only; D-099)
build-release:
    scripts/build-release.sh

# Notarize and staple the release DMG (uploads it to Apple; needs APPLE_NOTARY_PROFILE)
notarize-release:
    scripts/build-release.sh --notarize

# Build the documentation site into site/book (needs mdbook 0.4.52)
site:
    mdbook build site

# Hardware in the loop test: flashes debug then release firmware (board on USB)
hil:
    scripts/hil.sh

# App HIL: transport and devices against the board (any firmware, no flashing)
hil-app:
    cargo test -p detent -- --ignored --nocapture --test-threads 1

# Exercise the macOS bindings by hand: open <bundle-id> | frontmost | volume | mute
macos-check *ARGS:
    cargo run -q -p detent --example macos_check -- {{ARGS}}

# Run ctl-probe, e.g. `just probe`, `just probe --raw`, `just probe --count-hellos 20`
probe *ARGS:
    cargo run -q -p ctl-probe -- {{ARGS}}

# Regenerate firmware/include/detent_protocol.h from the protocol crate
gen-header:
    scripts/gen-header.sh

# Build the firmware for every board (XIAO, Feather, DevKitC, Pico; release and debug)
fw:
    pio run -d firmware -e xiao_esp32s3 -e xiao_esp32s3_debug -e feather_esp32s3 -e feather_esp32s3_debug -e devkitc_esp32s3 -e devkitc_esp32s3_debug -e pico -e pico_spike

# Build the debug firmware (TEST_INJECT enabled; never ship it)
fw-debug:
    pio run -d firmware -e xiao_esp32s3_debug

# Run the firmware's native C++ tests (no hardware)
fw-test:
    pio test -d firmware -e native

# Flash a board's firmware, e.g. `just fw-upload` (XIAO) or `just fw-upload feather_esp32s3`
fw-upload env="xiao_esp32s3":
    pio run -d firmware -e {{env}} -t upload

# Run every CI check locally (same as .github/workflows/ci.yml)
ci: lint test
    pnpm -C app install --frozen-lockfile
    pnpm -C app tsc --noEmit
    pio run -d firmware -e xiao_esp32s3 -e xiao_esp32s3_debug -e feather_esp32s3 -e feather_esp32s3_debug -e devkitc_esp32s3 -e devkitc_esp32s3_debug -e pico -e pico_spike
    pio test -d firmware -e native
