//! Tray icon and menu. The tray is the only UI shown at launch.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use detent_lib::app_state::AppState;
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager, Runtime, Wry,
};

use crate::launch::Shared;

pub const TRAY_ID: &str = "detent";
pub const ID_ACCESSIBILITY: &str = "accessibility";
pub const ID_DEVICE: &str = "device";
pub const ID_LOGIN: &str = "login";
pub const ID_NEW_DEVICE: &str = "new_device";
pub const ID_PAUSE: &str = "pause";
pub const ID_OPEN: &str = "open";
pub const ID_QUIT: &str = "quit";

/// Accessibility status for this process. `None` off macOS, where the grant
/// doesn't exist.
pub fn accessibility_status() -> Option<bool> {
    #[cfg(target_os = "macos")]
    return Some(detent_lib::macos::accessibility_trusted());
    #[cfg(not(target_os = "macos"))]
    return None;
}

/// Text of the Accessibility status line.
pub fn accessibility_label(trusted: Option<bool>) -> &'static str {
    match trusted {
        Some(true) => "Accessibility: granted",
        Some(false) => "Accessibility: not granted",
        None => "Accessibility: unknown",
    }
}

/// What the engine is doing, for the tray's device line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceStatus<'a> {
    /// Not started yet.
    Starting,
    /// Running; names of the connected units.
    Running(&'a [String]),
    /// The config didn't load or USB is unavailable (details in the log).
    Problem,
}

/// Open at login as the tray shows it, `None` off macOS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Login {
    Off,
    On,
    NeedsApproval,
    Unavailable,
}

/// The current login item state.
pub fn login_status() -> Option<Login> {
    #[cfg(target_os = "macos")]
    return Some(match detent_lib::macos::login_item() {
        detent_lib::macos::LoginItem::Off => Login::Off,
        detent_lib::macos::LoginItem::On => Login::On,
        detent_lib::macos::LoginItem::NeedsApproval => Login::NeedsApproval,
        detent_lib::macos::LoginItem::Unavailable => Login::Unavailable,
    });
    #[cfg(not(target_os = "macos"))]
    return None;
}

/// Text, tick, and whether it can be clicked, for the Open at login item.
pub fn login_item_view(state: Option<Login>) -> (&'static str, bool, bool) {
    match state {
        Some(Login::On) => ("Open at Login", true, true),
        Some(Login::Off) => ("Open at Login", false, true),
        Some(Login::NeedsApproval) => ("Open at Login: allow in System Settings", true, true),
        Some(Login::Unavailable) | None => ("Open at Login: not available", false, false),
    }
}

/// Text shown when macOS refused to change open at login.
pub const LOGIN_FAILED: &str = "Open at Login: couldn't change it";

/// Flips open at login. Returns the state that results, or the error.
fn toggle_login(current: Option<Login>) -> Result<Option<Login>, String> {
    #[cfg(target_os = "macos")]
    {
        let on = !matches!(current, Some(Login::On | Login::NeedsApproval));
        detent_lib::macos::set_login_item(on)?;
    }
    let _ = current;
    Ok(login_status())
}

fn show_login<R: Runtime>(item: &CheckMenuItem<R>, state: Option<Login>) {
    let (text, checked, enabled) = login_item_view(state);
    let _ = item.set_text(text);
    let _ = item.set_checked(checked);
    let _ = item.set_enabled(enabled);
}

/// Text of the device line, led by its status dot (D-074): green connected,
/// white none, red a problem.
pub fn device_label(status: &DeviceStatus, dry_run: bool) -> String {
    let base = match status {
        DeviceStatus::Starting => "⚪ Starting".to_owned(),
        DeviceStatus::Running([]) => "⚪ No device connected".to_owned(),
        DeviceStatus::Running([one]) => format!("🟢 Connected: {one}"),
        DeviceStatus::Running(many) => format!("🟢 Connected: {} devices", many.len()),
        DeviceStatus::Problem => "🔴 Problem: see the log".to_owned(),
    };
    if dry_run {
        format!("{base} (dry run)")
    } else {
        base
    }
}

/// The line shown while an unpaired Detent is plugged in; it opens the wizard.
pub const NEW_DEVICE: &str = "New device found: Pair…";
/// Text shown when Pause couldn't be saved (the config isn't writable).
pub const PAUSE_FAILED: &str = "Pause: couldn't save the config";

/// What the tray shows for a given app state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    pub device: String,
    pub paused: bool,
    pub new_device: bool,
}

pub fn view(state: &AppState, dry_run: bool) -> View {
    let connected: Vec<String> = state
        .paired
        .iter()
        .filter(|u| u.connected)
        .map(|u| u.name.clone())
        .collect();
    let line = if state.problem.is_some() {
        DeviceStatus::Problem
    } else {
        DeviceStatus::Running(&connected)
    };
    let mut device = device_label(&line, dry_run);
    match (state.problem.is_some(), state.recovered) {
        (true, _) | (_, 0) => {}
        (false, 1) => device.push_str(" · recovered from an error"),
        (false, n) => device.push_str(&format!(" · recovered from {n} errors")),
    }
    View {
        device,
        paused: state.paused,
        // A unit with no serial can't be paired, so it isn't offered.
        new_device: state.unknown.iter().any(|u| u.serial.is_some()),
    }
}

/// The menu items the app keeps current.
#[derive(Clone)]
pub struct Tray {
    menu: Menu<Wry>,
    device: MenuItem<Wry>,
    pause: CheckMenuItem<Wry>,
    new_device: MenuItem<Wry>,
    /// Whether `new_device` is in the menu (it's inserted and removed).
    showing_new: Arc<AtomicBool>,
}

impl Tray {
    pub fn show(&self, view: &View) {
        if let Err(e) = self.device.set_text(&view.device) {
            eprintln!("detent: could not update the tray: {e}");
        }
        let _ = self.pause.set_text("Pause");
        let _ = self.pause.set_checked(view.paused);
        if self.showing_new.swap(view.new_device, Ordering::Relaxed) != view.new_device {
            let done = if view.new_device {
                self.menu.insert(&self.new_device, 1)
            } else {
                self.menu.remove(&self.new_device)
            };
            if let Err(e) = done {
                eprintln!("detent: could not update the tray: {e}");
            }
        }
    }
}

fn toggle_pause(app: &AppHandle<Wry>, tray: &Tray) {
    let Some(shared) = app.try_state::<Shared>() else {
        return;
    };
    let on = !shared
        .state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .paused;
    if let Err(e) = crate::commands::set_paused_now(&shared, on) {
        eprintln!("detent: pause: {e}");
        let _ = tray.pause.set_checked(!on);
        let _ = tray.pause.set_text(PAUSE_FAILED);
    }
}

fn pair_new_device(app: &AppHandle<Wry>) {
    // Pairing starts first, so the window opens on the wizard.
    if let Some(shared) = app.try_state::<Shared>() {
        let _ = crate::commands::set_identifying(&shared, true);
    }
    if let Err(err) = crate::open_settings(app) {
        eprintln!("detent: could not open settings window: {err}");
    }
}

/// Builds the tray. Returns the items the app keeps current.
pub fn build(app: &AppHandle<Wry>, dry_run: bool) -> tauri::Result<Tray> {
    let device = MenuItem::with_id(
        app,
        ID_DEVICE,
        device_label(&DeviceStatus::Starting, dry_run),
        false,
        None::<&str>,
    )?;
    let accessibility = MenuItem::with_id(
        app,
        ID_ACCESSIBILITY,
        accessibility_label(accessibility_status()),
        false,
        None::<&str>,
    )?;
    let (text, checked, enabled) = login_item_view(login_status());
    let login = CheckMenuItem::with_id(app, ID_LOGIN, text, enabled, checked, None::<&str>)?;
    let new_device = MenuItem::with_id(app, ID_NEW_DEVICE, NEW_DEVICE, true, None::<&str>)?;
    let pause = CheckMenuItem::with_id(app, ID_PAUSE, "Pause", true, false, None::<&str>)?;
    let open = MenuItem::with_id(app, ID_OPEN, "Open Detent", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, ID_QUIT, "Quit Detent", true, Some("Cmd+Q"))?;
    let menu = Menu::with_items(
        app,
        &[
            &device,
            &PredefinedMenuItem::separator(app)?,
            &pause,
            &accessibility,
            &login,
            &PredefinedMenuItem::separator(app)?,
            &open,
            &quit,
        ],
    )?;

    let tray = Tray {
        menu: menu.clone(),
        device,
        pause,
        new_device,
        showing_new: Arc::new(AtomicBool::new(false)),
    };

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .icon_as_template(true)
        .tooltip("Detent")
        .menu(&menu)
        // Re-check on hover and click so a grant or login item approval made
        // in System Settings shows up without a relaunch. Both calls are
        // cheap and never prompt.
        .on_tray_icon_event({
            let login = login.clone();
            move |_tray, _event| {
                let label = accessibility_label(accessibility_status());
                if let Err(err) = accessibility.set_text(label) {
                    eprintln!("detent: could not update accessibility label: {err}");
                }
                show_login(&login, login_status());
            }
        })
        .on_menu_event({
            let tray = tray.clone();
            move |app, event| match event.id().as_ref() {
                ID_NEW_DEVICE => pair_new_device(app),
                ID_PAUSE => toggle_pause(app, &tray),
                ID_OPEN => {
                    if let Err(err) = crate::open_settings(app) {
                        eprintln!("detent: could not open settings window: {err}");
                    }
                }
                ID_LOGIN => match toggle_login(login_status()) {
                    Ok(state) => show_login(&login, state),
                    Err(e) => {
                        eprintln!("detent: open at login: {e}");
                        show_login(&login, login_status());
                        let _ = login.set_text(LOGIN_FAILED);
                    }
                },
                ID_QUIT => app.exit(0),
                _ => {}
            }
        })
        .build(app)?;
    Ok(tray)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessibility_label_covers_every_state() {
        assert_eq!(accessibility_label(Some(true)), "Accessibility: granted");
        assert_eq!(
            accessibility_label(Some(false)),
            "Accessibility: not granted"
        );
        assert_eq!(accessibility_label(None), "Accessibility: unknown");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_status_is_always_known() {
        let label = accessibility_label(accessibility_status());
        assert_ne!(label, accessibility_label(None));
    }

    #[test]
    fn device_label_covers_every_state() {
        let one = ["Desk controller".to_owned()];
        let two = ["a".to_owned(), "b".to_owned()];
        assert_eq!(
            device_label(&DeviceStatus::Running(&[]), false),
            "⚪ No device connected"
        );
        assert_eq!(
            device_label(&DeviceStatus::Running(&one), false),
            "🟢 Connected: Desk controller"
        );
        assert_eq!(
            device_label(&DeviceStatus::Running(&two), false),
            "🟢 Connected: 2 devices"
        );
        assert_eq!(
            device_label(&DeviceStatus::Problem, false),
            "🔴 Problem: see the log"
        );
        assert_eq!(
            device_label(&DeviceStatus::Running(&one), true),
            "🟢 Connected: Desk controller (dry run)"
        );
        assert_eq!(device_label(&DeviceStatus::Starting, false), "⚪ Starting");
    }

    #[test]
    fn login_item_view_covers_every_state() {
        assert_eq!(
            login_item_view(Some(Login::On)),
            ("Open at Login", true, true)
        );
        assert_eq!(
            login_item_view(Some(Login::Off)),
            ("Open at Login", false, true)
        );
        assert_eq!(
            login_item_view(Some(Login::NeedsApproval)),
            ("Open at Login: allow in System Settings", true, true)
        );
        assert_eq!(
            login_item_view(Some(Login::Unavailable)),
            ("Open at Login: not available", false, false)
        );
        assert_eq!(
            login_item_view(None),
            login_item_view(Some(Login::Unavailable))
        );
    }

    #[test]
    fn menu_ids_are_distinct() {
        let ids = [
            ID_DEVICE,
            ID_ACCESSIBILITY,
            ID_LOGIN,
            ID_NEW_DEVICE,
            ID_PAUSE,
            ID_OPEN,
            ID_QUIT,
        ];
        for (i, a) in ids.iter().enumerate() {
            for b in &ids[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    fn state() -> AppState {
        let config = detent_lib::store::parse(
            r#"{ "version": 1, "devices": [{ "name": "Desk", "vid": "0x303A", "pid": "0x8FDE", "serial": "E072A1FC4EDC" }] }"#,
        )
        .unwrap();
        AppState::new(&config)
    }

    #[test]
    fn the_view_follows_the_state() {
        let mut s = state();
        assert_eq!(
            view(&s, false),
            View {
                device: "⚪ No device connected".into(),
                paused: false,
                new_device: false,
            }
        );
        s.paired[0].connected = true;
        s.paused = true;
        let v = view(&s, false);
        assert_eq!(v.device, "🟢 Connected: Desk");
        assert!(v.paused);
        s.problem = Some("config not loaded".into());
        assert_eq!(view(&s, false).device, "🔴 Problem: see the log");
    }

    #[test]
    fn a_recovery_shows_on_the_device_line_until_relaunch() {
        let mut s = state();
        s.paired[0].connected = true;
        s.recovered = 1;
        assert_eq!(
            view(&s, false).device,
            "🟢 Connected: Desk · recovered from an error"
        );
        s.recovered = 3;
        assert_eq!(
            view(&s, true).device,
            "🟢 Connected: Desk (dry run) · recovered from 3 errors"
        );
        s.problem = Some("config not loaded".into());
        assert_eq!(view(&s, false).device, "🔴 Problem: see the log");
    }

    #[test]
    fn only_a_pairable_unit_is_new_device_news() {
        use detent_lib::app_state::UnknownUnit;
        let mut s = state();
        s.unknown.push(UnknownUnit {
            vid: 0x303A,
            pid: 0x8FDE,
            serial: None,
        });
        assert!(!view(&s, false).new_device, "no serial: can't be paired");
        s.unknown.push(UnknownUnit {
            vid: 0x303A,
            pid: 0x8FDE,
            serial: Some("AABBCCDDEEFF".into()),
        });
        assert!(view(&s, false).new_device);
    }
}
