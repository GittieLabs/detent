//! USB identity. See "Identity" in `docs/protocol.md` and D-014.

/// Espressif's vendor ID.
pub const VID: u16 = 0x303A;

// PLACEHOLDER: replace with Espressif usb-pids allocation.
// Chosen 2026-11-04 near the top of 0x8000..=0x8FFF, far above the highest
// allocated PID then (0x83EA), so it won't collide before the PR merges.
// Never use TinyUSB or IDF defaults such as 0x4001.
pub const PID: u16 = 0x8FDE;
