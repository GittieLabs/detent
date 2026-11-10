//! Detent's engine, as a library: the modules `docs/architecture.md` lists,
//! testable without starting Tauri (D-052). `main.rs` is the Tauri app.

pub mod actions;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod router;
pub mod store;
pub mod transport;
