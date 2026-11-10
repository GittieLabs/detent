//! Tray icon and menu. The tray is the only UI shown at launch.

use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Runtime,
};

pub const TRAY_ID: &str = "detent";
pub const ID_ACCESSIBILITY: &str = "accessibility";
pub const ID_DEVICE: &str = "device";
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
    let open = MenuItem::with_id(app, ID_OPEN, "Open Detent", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, ID_QUIT, "Quit Detent", true, Some("Cmd+Q"))?;
    let menu = Menu::with_items(
        app,
        &[
            &device,
            &accessibility,
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
        // Re-check on hover and click so a grant made in System Settings shows
        // up without a relaunch. The call is cheap and never prompts.
        .on_tray_icon_event(move |_tray, _event| {
            let label = accessibility_label(accessibility_status());
            if let Err(err) = accessibility.set_text(label) {
                eprintln!("detent: could not update accessibility label: {err}");
            }
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            ID_OPEN => {
                if let Err(err) = crate::open_settings(app) {
                    eprintln!("detent: could not open settings window: {err}");
                }
            }
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
    fn menu_ids_are_distinct() {
        let ids = [ID_DEVICE, ID_ACCESSIBILITY, ID_OPEN, ID_QUIT];
        for (i, a) in ids.iter().enumerate() {
            for b in &ids[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
