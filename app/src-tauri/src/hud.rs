//! The volume overlay window (D-083): Detent's own level bar, shown for
//! about a second after a volume action runs, near the bottom of the display
//! with the pointer. Borderless, transparent, click through, never focused,
//! and with no IPC capabilities (the `default` capability names only the
//! settings window): Rust passes it the level by script.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use detent_lib::app_state::Hud;
use tauri::{
    AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

/// The overlay window's label. Not in any capability.
pub const HUD_WINDOW: &str = "hud";
/// How long it stays up after the last volume action.
const SHOW_FOR: Duration = Duration::from_millis(1200);
const WIDTH: f64 = 220.0;
const HEIGHT: f64 = 64.0;
/// Above the bottom of the display, like macOS's own HUD.
const FROM_BOTTOM: f64 = 140.0;

/// Shows the overlay from any thread.
#[derive(Clone)]
pub struct Overlay {
    app: AppHandle,
    /// Bumped on every show; a pending hide only fires if it still matches.
    shown: Arc<AtomicU64>,
}

fn args(hud: Hud) -> (u8, &'static str) {
    match hud {
        Hud::Level(level) => (level, "null"),
        Hud::Muted(true) => (0, "true"),
        Hud::Muted(false) => (current_level(), "false"),
    }
}

fn current_level() -> u8 {
    #[cfg(target_os = "macos")]
    return detent_lib::macos::volume()
        .map(|v| (v.clamp(0.0, 1.0) * 100.0).round() as u8)
        .unwrap_or(0);
    #[cfg(not(target_os = "macos"))]
    return 0;
}

impl Overlay {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            shown: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn show(&self, hud: Hud) {
        let (level, muted) = args(hud);
        let generation = self.shown.fetch_add(1, Ordering::Relaxed) + 1;
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            if let Err(e) = show_now(&app, level, muted) {
                eprintln!("detent: volume overlay: {e}");
            }
        });
        let app = self.app.clone();
        let shown = self.shown.clone();
        std::thread::spawn(move || {
            std::thread::sleep(SHOW_FOR);
            if shown.load(Ordering::Relaxed) == generation {
                let _ = app.clone().run_on_main_thread(move || {
                    if let Some(w) = app.get_webview_window(HUD_WINDOW) {
                        let _ = w.hide();
                    }
                });
            }
        });
    }
}

fn build(app: &AppHandle, level: u8, muted: &str) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, HUD_WINDOW, WebviewUrl::App("hud.html".into()))
        .title("Detent volume")
        .inner_size(WIDTH, HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .focused(false)
        .focusable(false)
        .visible(false)
        .initialization_script(format!("window.__hudInit = [{level}, {muted}];"))
        .build()?;
    window.set_ignore_cursor_events(true)?;
    Ok(window)
}

fn show_now(app: &AppHandle, level: u8, muted: &str) -> tauri::Result<()> {
    let window = match app.get_webview_window(HUD_WINDOW) {
        Some(w) => {
            w.eval(format!(
                "window.detentHud && window.detentHud({level}, {muted});"
            ))?;
            w
        }
        None => build(app, level, muted)?,
    };
    // The display with the pointer, else the main one.
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    if let Some(m) = monitor {
        let scale = m.scale_factor();
        let (pos, size) = (m.position(), m.size());
        let x = pos.x as f64 + (size.width as f64 - WIDTH * scale) / 2.0;
        let y = pos.y as f64 + size.height as f64 - (HEIGHT + FROM_BOTTOM) * scale;
        window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32))?;
    }
    window.show()
}
