//! All objc and CoreGraphics bridging lives here, and only here.
//!
//! This is the one module allowed `unsafe` (CLAUDE.md, D-021). Every unsafe
//! call gets a safe wrapper; nothing outside this module sees raw FFI.

#![allow(unsafe_code)]

mod accessibility;

pub use accessibility::accessibility_trusted;
