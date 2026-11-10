//! NSWorkspace: the frontmost app (no permission needed) and launching apps
//! by bundle ID. Used by the router (app scope) and the Open app action.

use objc2_app_kit::NSWorkspace;
use std::time::Duration;

use objc2_foundation::{NSDate, NSRunLoop, NSString};

/// Why an app couldn't be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenAppError {
    /// No installed app has this bundle ID.
    NotInstalled(String),
    /// Launch Services refused or failed to open it.
    LaunchFailed(String),
}

impl std::fmt::Display for OpenAppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInstalled(id) => write!(f, "no installed app has bundle ID {id}"),
            Self::LaunchFailed(id) => write!(f, "macOS couldn't open {id}"),
        }
    }
}

impl std::error::Error for OpenAppError {}

/// The bundle ID of the frontmost app, or `None` when there isn't one with a
/// bundle (rare: a bare executable, or no GUI session). Never prompts.
///
/// AppKit keeps this value current from notifications delivered on the main
/// run loop, so the process must be running one: the Tauri app does, from any
/// thread. A command line tool that never runs it reads the same app forever
/// (seen in acceptance M2 step 1); such a tool calls [`run_loop_for`] on its
/// main thread between reads.
pub fn frontmost_bundle_id() -> Option<String> {
    // objc2 marks these NSWorkspace and NSRunningApplication calls safe; they
    // may be made from any thread.
    let workspace = NSWorkspace::sharedWorkspace();
    let app = workspace.frontmostApplication()?;
    let id = app.bundleIdentifier()?;
    Some(id.to_string())
}

/// Runs the calling thread's run loop for `d`, delivering pending AppKit
/// notifications. For command line tools on their main thread; the app's
/// main run loop already runs.
pub fn run_loop_for(d: Duration) {
    let until = NSDate::dateWithTimeIntervalSinceNow(d.as_secs_f64());
    NSRunLoop::currentRunLoop().runUntilDate(&until);
}

/// Opens (or brings forward) the app with this bundle ID. Returns once Launch
/// Services has accepted the request, not when the app has finished launching.
pub fn open_app(bundle_id: &str) -> Result<(), OpenAppError> {
    let id = NSString::from_str(bundle_id);
    let workspace = NSWorkspace::sharedWorkspace();
    let url = workspace
        .URLForApplicationWithBundleIdentifier(&id)
        .ok_or_else(|| OpenAppError::NotInstalled(bundle_id.to_owned()))?;
    if workspace.openURL(&url) {
        Ok(())
    } else {
        Err(OpenAppError::LaunchFailed(bundle_id.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_bundle_id_is_not_installed() {
        assert_eq!(
            open_app("com.gittielabs.detent.no-such-app"),
            Err(OpenAppError::NotInstalled(
                "com.gittielabs.detent.no-such-app".into()
            ))
        );
    }

    #[test]
    fn frontmost_app_has_a_bundle_id_in_a_gui_session() {
        let id = frontmost_bundle_id();
        // CI runners may have no GUI session; locally there's always a frontmost app.
        if std::env::var_os("CI").is_none() {
            let id = id.expect("a frontmost app");
            assert!(id.contains('.'), "looks like a bundle ID: {id}");
        }
    }

    #[test]
    fn works_from_a_background_thread() {
        // The engine calls these off the main thread.
        let handle = std::thread::spawn(|| {
            let _ = frontmost_bundle_id();
            open_app("com.gittielabs.detent.no-such-app").is_err()
        });
        assert!(handle.join().expect("thread didn't panic"));
    }
}
