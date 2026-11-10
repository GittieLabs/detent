//! Tray icon and menu. The tray is the only UI shown at launch.

use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Runtime,
};

pub const TRAY_ID: &str = "detent";
pub const ID_ACCESSIBILITY: &str = "accessibility";
pub const ID_DEVICE: &str = "device";
pub const ID_LOGIN: &str = "login";
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
    /// The config is missing or invalid (details in the log).
    ConfigProblem,
    UsbUnavailable,
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

/// Text of the device line.
pub fn device_label(status: &DeviceStatus, dry_run: bool) -> String {
    let base = match status {
        DeviceStatus::Starting => "Starting".to_owned(),
        DeviceStatus::Running([]) => "No device connected".to_owned(),
        DeviceStatus::Running([one]) => format!("Connected: {one}"),
        DeviceStatus::Running(many) => format!("Connected: {} devices", many.len()),
        DeviceStatus::ConfigProblem => "Config problem: see the log".to_owned(),
        DeviceStatus::UsbUnavailable => "USB unavailable: see the log".to_owned(),
    };
    if dry_run {
        format!("{base} (dry run)")
    } else {
        base
    }
}

/// Builds the tray. Returns the device line, which the engine keeps current.
pub fn build<R: Runtime>(app: &AppHandle<R>, dry_run: bool) -> tauri::Result<MenuItem<R>> {
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
    let open = MenuItem::with_id(app, ID_OPEN, "Open Detent", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, ID_QUIT, "Quit Detent", true, Some("Cmd+Q"))?;
    let menu = Menu::with_items(
        app,
        &[
            &device,
            &accessibility,
            &login,
            &PredefinedMenuItem::separator(app)?,
            &open,
            &quit,
        ],
    )?;

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
        .on_menu_event(move |app, event| match event.id().as_ref() {
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
        })
        .build(app)?;
    Ok(device)
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
            "No device connected"
        );
        assert_eq!(
            device_label(&DeviceStatus::Running(&one), false),
            "Connected: Desk controller"
        );
        assert_eq!(
            device_label(&DeviceStatus::Running(&two), false),
            "Connected: 2 devices"
        );
        assert_eq!(
            device_label(&DeviceStatus::ConfigProblem, false),
            "Config problem: see the log"
        );
        assert_eq!(
            device_label(&DeviceStatus::Running(&one), true),
            "Connected: Desk controller (dry run)"
        );
        assert_eq!(device_label(&DeviceStatus::Starting, false), "Starting");
        assert_eq!(
            device_label(&DeviceStatus::UsbUnavailable, false),
            "USB unavailable: see the log"
        );
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
        let ids = [ID_DEVICE, ID_ACCESSIBILITY, ID_LOGIN, ID_OPEN, ID_QUIT];
        for (i, a) in ids.iter().enumerate() {
            for b in &ids[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
