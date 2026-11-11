//! The dashboard window (D-088, D-089): a web page on a chosen display, full
//! screen (borderless, sized to the display; native full screen fights
//! Spaces) or in an ordinary resizable window. Shown without taking focus
//! (full screen can't take it at all), and with no IPC capabilities: the
//! `default` capability names only the settings window, so a remote page can
//! never call into Rust. A second press hides it; so does a close button (full
//! screen: shown on hover; a window: its own), and unplugging the device when
//! the action asks (D-090). The hover button works without IPC: an injected
//! script navigates to a reserved address Detent intercepts. A watcher keeps
//! a full screen dashboard on a connected display: if its display is
//! unplugged it moves to the main one.

use std::sync::{mpsc, Mutex};
use std::time::Duration;

use detent_lib::actions::{ActionError, Done, Executor, Outcome};
use detent_lib::displays::{self, Display};
use detent_lib::store::{Action, DashboardMode};
use tauri::{
    AppHandle, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

/// The dashboard windows' labels, one per mode, so switching modes only hides
/// the other (rebuilding one label at once fails while macOS tears the old
/// window down). Neither is in any capability.
const FULLSCREEN_WINDOW: &str = "dashboard-full";
const WINDOWED_WINDOW: &str = "dashboard-window";

fn label(mode: DashboardMode) -> &'static str {
    match mode {
        DashboardMode::Fullscreen => FULLSCREEN_WINDOW,
        DashboardMode::Window => WINDOWED_WINDOW,
    }
}
/// How often the watcher checks the window is still on a connected display.
const WATCH_EVERY: Duration = Duration::from_secs(2);

/// How the dashboard was last shown: for the watcher, to rebuild the window
/// when the mode changes, and to know whether losing the device closes it.
struct Shown {
    display: Option<String>,
    mode: DashboardMode,
    close_without_device: bool,
}

static WANTED: Mutex<Shown> = Mutex::new(Shown {
    display: None,
    mode: DashboardMode::Fullscreen,
    close_without_device: true,
});

/// Navigating here closes the dashboard (the hover close button). The
/// `.invalid` name never resolves, so nothing is ever fetched.
const CLOSE_HOST: &str = "detent-close.invalid";

/// Injected into every page a full screen dashboard loads, before the page's
/// own scripts: a close button that shows while the mouse moves.
const CLOSE_SCRIPT: &str = r##"(() => {
  if (window.top !== window) return;
  const close = () => { window.location.href = "https://detent-close.invalid/"; };
  const add = () => {
    if (document.getElementById("__detent_close")) return;
    const b = document.createElement("button");
    b.id = "__detent_close";
    b.textContent = "✕  Close";
    b.title = "Close the dashboard";
    b.setAttribute("aria-label", "Close dashboard");
    // Bottom right, clear of page menus and the scroll bar; big and high
    // contrast on light or dark pages. It only shows while the mouse moves.
    Object.assign(b.style, {
      position: "fixed", bottom: "28px", right: "36px", zIndex: "2147483647",
      height: "52px", padding: "0 22px", borderRadius: "26px",
      border: "3px solid #ffffff", background: "rgba(18,18,20,0.92)",
      color: "#ffffff", cursor: "pointer", letterSpacing: "0.02em",
      font: "600 18px -apple-system, BlinkMacSystemFont, sans-serif",
      boxShadow: "0 4px 18px rgba(0,0,0,0.55), 0 0 0 1px rgba(0,0,0,0.6)",
      opacity: "0", pointerEvents: "none", transition: "opacity 0.2s",
    });
    b.addEventListener("click", close);
    document.documentElement.appendChild(b);
    let hide;
    const show = () => {
      b.style.opacity = "1";
      b.style.pointerEvents = "auto";
      clearTimeout(hide);
      hide = setTimeout(() => { b.style.opacity = "0"; b.style.pointerEvents = "none"; }, 2000);
    };
    // Detent's own app isn't active while you use another, so WebKit sends
    // the page no mouse moves: Detent watches the cursor and calls this.
    window.__detentShowClose = show;
    addEventListener("mousemove", show, true);
  };
  if (document.readyState === "loading") addEventListener("DOMContentLoaded", add);
  else add();
})();"##;

/// A display's work area: its bounds less the menu bar (and the Dock, when
/// it's on that display), so a full screen dashboard never covers them.
fn display_of(m: &Monitor) -> Display {
    let area = m.work_area();
    Display {
        name: m.name().cloned(),
        x: area.position.x,
        y: area.position.y,
        width: area.size.width,
        height: area.size.height,
    }
}

/// Connected displays and the main one's index.
fn displays_now(app: &AppHandle) -> (Vec<Display>, usize) {
    let all: Vec<Display> = app
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(display_of)
        .collect();
    let main = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| display_of(&m))
        .and_then(|p| all.iter().position(|d| *d == p))
        .unwrap_or(0);
    (all, main)
}

/// Names of the connected displays, main first, for the editor.
pub fn display_names(app: &AppHandle) -> Vec<String> {
    let (all, main) = displays_now(app);
    let mut names: Vec<String> = Vec::new();
    if let Some(n) = all.get(main).and_then(|d| d.name.clone()) {
        names.push(n);
    }
    for d in &all {
        if let Some(n) = &d.name {
            if !names.contains(n) {
                names.push(n.clone());
            }
        }
    }
    names
}

/// Sizes the window to cover `d`.
fn fit(window: &WebviewWindow, d: &Display) -> tauri::Result<()> {
    window.set_position(PhysicalPosition::new(d.x, d.y))?;
    window.set_size(PhysicalSize::new(d.width, d.height))
}

fn build(app: &AppHandle, url: tauri::Url, mode: DashboardMode) -> tauri::Result<WebviewWindow> {
    let full = mode == DashboardMode::Fullscreen;
    let handle = app.clone();
    let me = label(mode);
    let mut builder = WebviewWindowBuilder::new(app, me, WebviewUrl::External(url));
    if full {
        // Never takes focus from the app in front; the hover button closes it.
        builder = builder.focusable(false).initialization_script(CLOSE_SCRIPT);
    }
    builder
        .title("Detent dashboard")
        .decorations(!full)
        .resizable(!full)
        .always_on_top(full)
        .visible_on_all_workspaces(full)
        .skip_taskbar(true)
        // A window is shown without taking focus; a click focuses it.
        .focused(false)
        .shadow(!full)
        .visible(false)
        .on_navigation(move |url| {
            if url.host_str() != Some(CLOSE_HOST) {
                return true;
            }
            let h = handle.clone();
            let _ = handle.run_on_main_thread(move || {
                if let Some(w) = h.get_webview_window(me) {
                    let _ = w.hide();
                }
            });
            false
        })
        .build()
}

/// Shows `url` on `display` in `mode`, or hides the window if it's already
/// showing that page. Runs on the main thread. Returns whether it's shown.
fn toggle(
    app: &AppHandle,
    parsed: tauri::Url,
    display: Option<&str>,
    mode: DashboardMode,
    close_without_device: bool,
) -> tauri::Result<bool> {
    *WANTED.lock().unwrap_or_else(|e| e.into_inner()) = Shown {
        display: display.map(str::to_owned),
        mode,
        close_without_device,
    };
    // The other mode's window, if it's up, gives way.
    let other = match mode {
        DashboardMode::Fullscreen => WINDOWED_WINDOW,
        DashboardMode::Window => FULLSCREEN_WINDOW,
    };
    if let Some(w) = app.get_webview_window(other) {
        w.hide()?;
    }
    let existing = app.get_webview_window(label(mode));
    if let Some(w) = &existing {
        let same_page = w.url().map(|u| u == parsed).unwrap_or(false);
        if w.is_visible()? && same_page {
            w.hide()?;
            return Ok(false);
        }
    }
    let window = match existing {
        Some(w) => {
            if w.url().map(|u| u != parsed).unwrap_or(true) {
                w.navigate(parsed)?;
            }
            w
        }
        None => build(app, parsed, mode)?,
    };
    let (all, main) = displays_now(app);
    if let Some(d) = displays::choose(&all, display, main) {
        fit(&window, &displays::placement(d, mode))?;
    }
    window.show()?;
    Ok(true)
}

/// Runs Open dashboard on the main thread (windows live there) and every
/// other action on the wrapped executor.
pub struct AppExecutor {
    pub inner: std::sync::Arc<dyn Executor>,
    pub app: AppHandle,
}

impl Executor for AppExecutor {
    fn run(&self, action: &Action) -> Outcome {
        let Action::OpenDashboard {
            url,
            display,
            mode,
            close_without_device,
        } = action
        else {
            return self.inner.run(action);
        };
        action.check_args().map_err(ActionError::Invalid)?;
        let parsed = url
            .parse::<tauri::Url>()
            .map_err(|e| ActionError::Window(format!("bad address {url}: {e}")))?;
        let (tx, rx) = mpsc::channel();
        let (app, display, mode, close) = (
            self.app.clone(),
            display.clone(),
            *mode,
            *close_without_device,
        );
        let posted = self.app.run_on_main_thread(move || {
            let result =
                toggle(&app, parsed, display.as_deref(), mode, close).map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        if posted.is_err() {
            return Err(ActionError::NeedsTheApp);
        }
        match rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(shown)) => Ok(Done::Dashboard { shown }),
            Ok(Err(e)) => Err(ActionError::Window(e)),
            Err(_) => Err(ActionError::Window("no answer from the main thread".into())),
        }
    }
}

/// Keeps a showing dashboard on a connected display: when its display is
/// unplugged, macOS can leave the window sized for a screen that's gone, so
/// it's refitted to the display it should be on now (the named one if
/// connected, else the main one).
pub fn watch(app: AppHandle) {
    let spawned = std::thread::Builder::new()
        .name("detent-dashboard".into())
        .spawn(move || loop {
            std::thread::sleep(WATCH_EVERY);
            let handle = app.clone();
            let (name, mode) = {
                let w = WANTED.lock().unwrap_or_else(|e| e.into_inner());
                (w.display.clone(), w.mode)
            };
            // A window is the user's to move and size; only full screen is kept.
            if mode != DashboardMode::Fullscreen {
                continue;
            }
            let _ = app.run_on_main_thread(move || {
                let Some(w) = handle.get_webview_window(FULLSCREEN_WINDOW) else {
                    return;
                };
                if !w.is_visible().unwrap_or(false) {
                    return;
                }
                let (all, main) = displays_now(&handle);
                let Some(target) = displays::choose(&all, name.as_deref(), main) else {
                    return;
                };
                let here = (w.outer_position().ok(), w.outer_size().ok());
                let there = (
                    Some(PhysicalPosition::new(target.x, target.y)),
                    Some(PhysicalSize::new(target.width, target.height)),
                );
                if here != there {
                    let _ = fit(&w, target);
                }
            });
        });
    if let Err(e) = spawned {
        eprintln!("detent: no dashboard watcher: {e}");
    }
}

/// How often the cursor is checked over a full screen dashboard.
const HOVER_EVERY: Duration = Duration::from_millis(150);

/// Shows the full screen dashboard's close button while the cursor moves
/// over it. The page can't see the mouse itself (Detent is a background
/// app), so Detent polls the cursor and asks the page by script; nothing
/// flows from the page to Detent.
pub fn watch_hover(app: AppHandle) {
    let last = std::sync::Arc::new(Mutex::new(None::<(i32, i32)>));
    let spawned = std::thread::Builder::new()
        .name("detent-dashboard-hover".into())
        .spawn(move || loop {
            std::thread::sleep(HOVER_EVERY);
            if WANTED.lock().unwrap_or_else(|e| e.into_inner()).mode != DashboardMode::Fullscreen {
                continue;
            }
            let (handle, last) = (app.clone(), last.clone());
            let _ = app.run_on_main_thread(move || {
                let Some(w) = handle.get_webview_window(FULLSCREEN_WINDOW) else {
                    return;
                };
                if !w.is_visible().unwrap_or(false) {
                    return;
                }
                let (Ok(c), Ok(pos), Ok(size)) =
                    (handle.cursor_position(), w.outer_position(), w.outer_size())
                else {
                    return;
                };
                let here = (c.x.round() as i32, c.y.round() as i32);
                let inside = here.0 >= pos.x
                    && here.1 >= pos.y
                    && here.0 < pos.x + size.width as i32
                    && here.1 < pos.y + size.height as i32;
                let mut last = last.lock().unwrap_or_else(|e| e.into_inner());
                let moved = last.is_some_and(|l| l != here);
                *last = Some(here);
                if inside && moved {
                    let _ = w.eval("window.__detentShowClose && window.__detentShowClose()");
                }
            });
        });
    if let Err(e) = spawned {
        eprintln!("detent: no dashboard hover watcher: {e}");
    }
}

/// No paired device is connected: hide the dashboard if the action that
/// showed it asked for that (D-090), so unplugging always dismisses it.
pub fn no_device(app: &AppHandle) {
    if !WANTED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .close_without_device
    {
        return;
    }
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        for me in [FULLSCREEN_WINDOW, WINDOWED_WINDOW] {
            if let Some(w) = handle.get_webview_window(me) {
                if w.is_visible().unwrap_or(false) {
                    let _ = w.hide();
                }
            }
        }
    });
}
