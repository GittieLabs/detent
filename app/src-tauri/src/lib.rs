//! Detent's engine, as a library: the modules `docs/architecture.md` lists,
//! testable without starting Tauri (D-052). `main.rs` is the Tauri app.

#[cfg(target_os = "macos")]
pub mod macos;
pub mod store;
