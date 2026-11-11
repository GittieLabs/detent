//! Detent's engine, as a library: the modules `docs/architecture.md` lists,
//! testable without starting Tauri (D-052). `main.rs` is the Tauri app.

pub mod actions;
pub mod app_state;
pub mod apps;
pub mod devices;
pub mod displays;
pub mod engine;
pub mod keys;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod models;
pub mod router;
pub mod store;
pub mod transport;
