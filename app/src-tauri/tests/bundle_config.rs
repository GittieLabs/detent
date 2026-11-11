//! The bundle settings a notarized release depends on (D-099): the hardened
//! runtime stays on, the entitlements file is wired in and grants Apple
//! Events (Run AppleScript), and Info.plist explains the Automation prompt.

use std::path::PathBuf;

fn src_tauri(file: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn conf() -> serde_json::Value {
    serde_json::from_str(&src_tauri("tauri.conf.json")).expect("tauri.conf.json parses")
}

#[test]
fn hardened_runtime_is_not_turned_off() {
    let mac = &conf()["bundle"]["macOS"];
    assert_ne!(mac["hardenedRuntime"], serde_json::Value::Bool(false));
}

#[test]
fn entitlements_grant_apple_events_and_nothing_risky() {
    let mac = &conf()["bundle"]["macOS"];
    let file = mac["entitlements"].as_str().expect("entitlements set");
    let text = src_tauri(file);
    assert!(
        text.contains("<key>com.apple.security.automation.apple-events</key>\n\t<true/>"),
        "{text}"
    );
    // These weaken the hardened runtime; none is needed (D-099).
    for weak in [
        "cs.allow-jit",
        "cs.allow-unsigned-executable-memory",
        "cs.disable-library-validation",
        "cs.allow-dyld-environment-variables",
        "get-task-allow",
    ] {
        assert!(!text.contains(weak), "{file} has {weak}");
    }
}

#[test]
fn info_plist_explains_the_automation_prompt() {
    let plist = src_tauri("Info.plist");
    assert!(plist.contains("<key>NSAppleEventsUsageDescription</key>"));
    assert!(plist.contains("<key>LSUIElement</key>"));
}

#[test]
fn the_release_is_version_0_1_0() {
    assert_eq!(env!("CARGO_PKG_VERSION"), "0.1.0");
}

#[test]
fn panics_unwind_so_crash_recovery_can_catch_them() {
    // `panic = "abort"` in any profile would turn every caught panic (the
    // engine and action worker, M9) into a crash.
    for file in ["../../Cargo.toml", "Cargo.toml"] {
        let text = src_tauri(file);
        assert!(
            !text.contains("panic = \"abort\""),
            "{file} aborts on panic"
        );
    }
}
