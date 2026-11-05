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

# Build the ESP32-S3 firmware
fw:
    pio run -d firmware

# Flash the firmware to a connected device
fw-upload:
    pio run -d firmware -t upload
