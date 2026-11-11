//! All objc and CoreGraphics bridging lives here, and only here.
//!
//! This is the one module allowed `unsafe` (CLAUDE.md, D-021). Every unsafe
//! call gets a safe wrapper; nothing outside this module sees raw FFI.

#![allow(unsafe_code)]

mod accessibility;
mod apps;
mod audio;
mod automation;
mod keys;
mod login;
mod workspace;

pub use accessibility::{accessibility_trusted, request_accessibility};
pub use apps::running_apps;
pub use audio::{muted, set_muted, set_volume, volume, AudioError};
pub use automation::automation_consent;
pub use keys::{post_combos, secure_input_on};
pub use login::{login_item, set_login_item, LoginItem};
pub use workspace::{frontmost_bundle_id, open_app, run_loop_for, OpenAppError};
