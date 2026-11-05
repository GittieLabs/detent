//! Tray icon and menu. The tray is the only UI shown at launch.

use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Runtime,
};

pub const TRAY_ID: &str = "detent";
pub const ID_ACCESSIBILITY: &str = "accessibility";
pub const ID_OPEN: &str = "open";
pub const ID_QUIT: &str = "quit";

/// Text of the Accessibility status line. `None` means not yet checked; the
/// `macos::accessibility_trusted()` task wires in the real value.
pub fn accessibility_label(trusted: Option<bool>) -> &'static str {
    match trusted {
        Some(true) => "Accessibility: granted",
        Some(false) => "Accessibility: not granted",
        None => "Accessibility: unknown",
    }
}

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let accessibility = MenuItem::with_id(
        app,
        ID_ACCESSIBILITY,
        accessibility_label(None),
        false,
        None::<&str>,
    )?;
    let open = MenuItem::with_id(app, ID_OPEN, "Open Detent", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, ID_QUIT, "Quit Detent", true, Some("Cmd+Q"))?;
    let menu = Menu::with_items(
        app,
        &[
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
    Ok(())
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

    #[test]
    fn menu_ids_are_distinct() {
        let ids = [ID_ACCESSIBILITY, ID_OPEN, ID_QUIT];
        for (i, a) in ids.iter().enumerate() {
            for b in &ids[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
