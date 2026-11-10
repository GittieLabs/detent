//! The running apps the user could pick (D-080).

use objc2_app_kit::{NSApplicationActivationPolicy, NSWorkspace};

/// `(bundle ID, name)` of each running app with a Dock icon (regular
/// activation policy), skipping any without a bundle ID or name.
pub fn running_apps() -> Vec<(String, String)> {
    // objc2 marks these NSWorkspace and NSRunningApplication calls safe; they
    // may be made from any thread.
    let apps = NSWorkspace::sharedWorkspace().runningApplications();
    apps.iter()
        .filter(|app| app.activationPolicy() == NSApplicationActivationPolicy::Regular)
        .filter_map(|app| {
            let id = app.bundleIdentifier()?.to_string();
            let name = app.localizedName()?.to_string();
            Some((id, name))
        })
        .collect()
}
