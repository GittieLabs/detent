//! Detent: macOS menu bar app. Tray only in M0; see `docs/roadmap.md`.

#[cfg(target_os = "macos")]
mod macos;
mod tray;

use tauri::{Manager, RunEvent, Runtime, WebviewUrl, WebviewWindowBuilder};

/// Label of the settings window. Matches `capabilities/default.json`.
pub const SETTINGS_WINDOW: &str = "main";

fn main() {
    let app = tauri::Builder::default()
        .setup(|app| {
            // Belt and braces with LSUIElement in Info.plist: `tauri dev` runs an
            // unbundled binary that never reads the plist.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            tray::build(app.handle())?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build the Detent app");

    app.run(|_app, event| {
        // Closing the last window must not quit a tray app. Quit from the tray
        // calls `exit(0)`, which arrives with a code and is let through.
        if let RunEvent::ExitRequested {
            code: None, api, ..
        } = event
        {
            api.prevent_exit();
        }
    });
}

/// Shows the settings window, creating it on first use.
pub fn open_settings<R: Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    let window = match app.get_webview_window(SETTINGS_WINDOW) {
        Some(window) => window,
        None => {
            WebviewWindowBuilder::new(app, SETTINGS_WINDOW, WebviewUrl::App("index.html".into()))
                .title("Detent")
                .inner_size(720.0, 480.0)
                .build()?
        }
    };
    window.show()?;
    window.set_focus()
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    const CONF: &str = include_str!("../tauri.conf.json");
    const PLIST: &str = include_str!("../Info.plist");
    const CAPABILITY: &str = include_str!("../capabilities/default.json");

    fn conf() -> Value {
        serde_json::from_str(CONF).expect("tauri.conf.json parses")
    }

    #[test]
    fn bundle_identifier_is_the_placeholder() {
        // PLACEHOLDER until Keith confirms (roadmap open question).
        assert_eq!(conf()["identifier"], "com.gittielabs.detent");
    }

    #[test]
    fn no_window_opens_on_launch() {
        let windows = conf()["app"]["windows"].as_array().cloned();
        assert_eq!(windows, Some(vec![]));
    }

    #[test]
    fn info_plist_hides_the_dock_icon() {
        let compact: String = PLIST.split_whitespace().collect();
        assert!(compact.contains("<key>LSUIElement</key><true/>"));
    }

    #[test]
    fn capability_covers_only_the_settings_window() {
        let cap: Value = serde_json::from_str(CAPABILITY).expect("capability parses");
        assert_eq!(cap["windows"], serde_json::json!([super::SETTINGS_WINDOW]));
    }
}
