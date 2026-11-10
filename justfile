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

# Build the ESP32-S3 firmware
fw:
    pio run -d firmware

# Build the debug firmware (TEST_INJECT enabled; never ship it)
fw-debug:
    pio run -d firmware -e xiao_esp32s3_debug

# Run the firmware's native C++ tests (no hardware)
fw-test:
    pio test -d firmware -e native

# Flash the firmware to a connected device
fw-upload:
    pio run -d firmware -t upload

# Run every CI check locally (same as .github/workflows/ci.yml)
ci: lint test
    pnpm -C app install --frozen-lockfile
    pnpm -C app tsc --noEmit
    pio run -d firmware -e xiao_esp32s3 -e xiao_esp32s3_debug
    pio test -d firmware -e native
