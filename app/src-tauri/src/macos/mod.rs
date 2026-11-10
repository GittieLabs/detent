//! All objc and CoreGraphics bridging lives here, and only here.
//!
//! This is the one module allowed `unsafe` (CLAUDE.md, D-021). Every unsafe
//! call gets a safe wrapper; nothing outside this module sees raw FFI.

#![allow(unsafe_code)]

mod accessibility;
mod audio;
mod workspace;

pub use accessibility::accessibility_trusted;
pub use audio::{muted, set_muted, set_volume, volume, AudioError};
pub use workspace::{frontmost_bundle_id, open_app, run_loop_for, OpenAppError};
