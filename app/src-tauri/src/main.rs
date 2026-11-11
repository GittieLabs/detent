//! Detent: macOS menu bar app. The tray, plus the engine on background
//! threads (`detent_lib::engine`); see `docs/roadmap.md`.

mod commands;
mod hud;
mod launch;
mod tray;

use tauri::{Manager, RunEvent, Runtime, WebviewUrl, WebviewWindowBuilder, WindowEvent};

/// Label of the settings window. Matches `capabilities/default.json`.
pub const SETTINGS_WINDOW: &str = "main";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let launch = launch::parse_args(&args);
    let app = tauri::Builder::default()
        .setup(move |app| {
            // Belt and braces with LSUIElement in Info.plist: `tauri dev` runs an
            // unbundled binary that never reads the plist.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let tray = tray::build(app.handle(), launch.dry_run)?;
            // Kept for the app's lifetime; the process exit ends its threads.
            app.manage(launch::start(&launch, app.handle().clone(), tray));
            if launch.settings {
                open_settings(app.handle())?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the settings window mid wizard closes the candidates it
            // had open read only (D-066); reopening starts on Devices.
            if window.label() == SETTINGS_WINDOW && matches!(event, WindowEvent::Destroyed) {
                if let Some(shared) = window.try_state::<launch::Shared>() {
                    if shared.state.lock().is_ok_and(|s| s.identifying) {
                        let _ = commands::set_identifying(&shared, false);
                    }
                    // Dry run lives only while Test shows (D-079).
                    if shared.state.lock().is_ok_and(|s| s.dry_run) {
                        commands::set_dry_run_now(&shared, false);
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::start_pairing,
            commands::end_pairing,
            commands::pair,
            commands::rename,
            commands::mappings_of,
            commands::unpair,
            commands::ignore,
            commands::unignore,
            commands::set_paused,
            commands::list_apps,
            commands::set_dry_run,
            commands::set_mapping,
            commands::remove_mapping,
            commands::apply_template,
            commands::fix_accessibility,
            commands::set_menu_item,
            commands::remove_menu_item,
            commands::move_menu_item,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build the Detent app");

    app.run(|app, event| match event {
        // Closing the last window must not quit a tray app. Quit from the tray
        // calls `exit(0)`, which arrives with a code and is let through.
        RunEvent::ExitRequested {
            code: None, api, ..
        } => api.prevent_exit(),
        // Quitting (the tray, logout, shutdown): release the units so their
        // LED and OLED stop saying paired (D-076).
        RunEvent::Exit => {
            if let Some(shared) = app.try_state::<launch::Shared>() {
                if let Some(engine) = &shared.engine {
                    engine.stop();
                }
            }
        }
        _ => {}
    });
}

/// Shows the settings window, creating it on first use.
pub fn open_settings<R: Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    let window = match app.get_webview_window(SETTINGS_WINDOW) {
        Some(window) => window,
        None => {
            WebviewWindowBuilder::new(app, SETTINGS_WINDOW, WebviewUrl::App("index.html".into()))
                .title("Detent")
                .inner_size(900.0, 600.0)
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
    fn capability_allows_exactly_the_app_commands() {
        let cap: Value = serde_json::from_str(CAPABILITY).expect("capability parses");
        let mut allowed: Vec<String> = cap["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|p| p.as_str())
            .filter(|p| p.starts_with("allow-"))
            .map(str::to_owned)
            .collect();
        let mut want: Vec<String> = crate::commands::ALL
            .iter()
            .map(|c| format!("allow-{}", c.replace('_', "-")))
            .collect();
        allowed.sort();
        want.sort();
        assert_eq!(allowed, want);
        let build = include_str!("../build.rs");
        for c in crate::commands::ALL {
            assert!(
                build.contains(&format!("\"{c}\"")),
                "{c} missing from build.rs"
            );
        }
    }

    #[test]
    fn capability_covers_only_the_settings_window() {
        let cap: Value = serde_json::from_str(CAPABILITY).expect("capability parses");
        assert_eq!(cap["windows"], serde_json::json!([super::SETTINGS_WINDOW]));
    }
}
