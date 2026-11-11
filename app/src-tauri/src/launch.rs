//! Launch options and the engine's hookup to the tray and log.
//!
//!   detent [--dry-run] [--config PATH] [--settings]
//!
//! `--dry-run` resolves and logs every press but runs nothing (until M3's
//! Test screen has its toggle, D-064). `--config` reads another config file
//! instead of `~/Library/Application Support/Detent/config.json`.
//! `--settings` opens the settings window at launch. Unknown
//! arguments are ignored: macOS and Tauri may pass their own.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use detent_lib::actions::{Executor, RecordingExecutor};
use detent_lib::app_state::{AppState, DebugLog};
use detent_lib::devices::Options;
use detent_lib::engine::{default_log_path, Engine, EngineHandle, FileLog, Record, Sink};
use detent_lib::store::{self, Config, ConfigError};
use detent_lib::transport::Hid;
use tauri::{AppHandle, Emitter};

use crate::tray;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Launch {
    pub dry_run: bool,
    pub config: Option<PathBuf>,
    pub settings: bool,
    /// `--debug`: open the settings window with Debug on (M9).
    pub debug: bool,
}

pub fn parse_args(args: &[String]) -> Launch {
    let mut launch = Launch::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dry-run" => launch.dry_run = true,
            "--settings" => launch.settings = true,
            "--debug" => {
                launch.debug = true;
                launch.settings = true;
            }
            "--config" => launch.config = it.next().map(PathBuf::from),
            _ => {}
        }
    }
    launch
}

/// The event the settings window listens to; its payload is the whole
/// [`AppState`].
pub const STATE_EVENT: &str = "state";
/// The event carrying each resolved input to the Test screen.
pub const INPUT_EVENT: &str = "input";
/// The event carrying each action's result to the Test screen.
pub const RESULT_EVENT: &str = "result";
/// The event carrying each debug log entry while Debug is on (M9).
pub const DEBUG_EVENT: &str = "debug";

/// Debug's on switch and its log, shared by the sink and the commands.
#[derive(Clone, Default)]
pub struct Debug {
    pub on: Arc<AtomicBool>,
    pub log: Arc<Mutex<DebugLog>>,
}

/// Everything the app shares between the engine's sink, the tray, and the
/// commands. Managed by Tauri.
pub struct Shared {
    /// Where the config lives, `None` without a home directory.
    pub path: Option<PathBuf>,
    /// The config as last loaded or saved.
    pub config: Mutex<Config>,
    /// False when the file on disk didn't load: saving would overwrite what
    /// the user wrote, so commands refuse until it's fixed (D-070).
    pub writable: AtomicBool,
    pub state: Arc<Mutex<AppState>>,
    pub engine: Option<EngineHandle>,
    pub app: AppHandle,
    pub tray: tray::Tray,
    pub dry_run: bool,
    /// Detent's log, shared with the engine's sink.
    pub log: Option<Arc<FileLog>>,
    pub debug: Debug,
}

impl Shared {
    /// Writes a line to Detent's log (and stderr).
    pub fn say(&self, line: &str) {
        eprintln!("detent: {line}");
        if let Some(log) = &self.log {
            log.line(line);
        }
    }

    /// Pushes the state to the settings window and the tray.
    pub fn publish(&self, state: &AppState) {
        publish(&self.app, &self.tray, state, self.dry_run);
    }
}

fn accessibility_granted() -> bool {
    #[cfg(target_os = "macos")]
    return detent_lib::macos::accessibility_trusted();
    #[cfg(not(target_os = "macos"))]
    return false;
}

/// How often the Accessibility grant is rechecked, so the banner clears soon
/// after it's turned on in System Settings, with no relaunch.
const ACCESSIBILITY_EVERY: std::time::Duration = std::time::Duration::from_secs(2);

/// Rechecks the grant in the background and publishes when it changes. The
/// check is one cheap call that never prompts.
fn watch_accessibility(
    app: AppHandle,
    tray: tray::Tray,
    state: Arc<Mutex<AppState>>,
    dry_run: bool,
) {
    let spawned = std::thread::Builder::new()
        .name("detent-accessibility".into())
        .spawn(move || loop {
            std::thread::sleep(ACCESSIBILITY_EVERY);
            let granted = accessibility_granted();
            let changed = {
                let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                s.set_accessibility(granted).then(|| s.clone())
            };
            if let Some(s) = changed {
                publish(&app, &tray, &s, dry_run);
            }
        });
    if let Err(e) = spawned {
        eprintln!("detent: no accessibility watcher: {e}");
    }
}

fn publish(app: &AppHandle, tray: &tray::Tray, state: &AppState, dry_run: bool) {
    let _ = app.emit(STATE_EVENT, state);
    tray.show(&tray::view(state, dry_run));
}

/// Logs every record, folds it into the app state, and publishes changes.
struct AppSink {
    log: Option<Arc<FileLog>>,
    overlay: crate::hud::Overlay,
    state: Arc<Mutex<AppState>>,
    app: AppHandle,
    tray: tray::Tray,
    dry_run: bool,
    debug: Debug,
}

impl Sink for AppSink {
    fn record(&self, record: &Record) {
        let report = matches!(
            record,
            Record::Device(detent_lib::devices::DeviceEvent::Report { .. })
        );
        // Raw reports go to Debug only: every heartbeat would flood the log.
        if !report {
            let line = record.to_string();
            eprintln!("detent: {line}");
            if let Some(log) = &self.log {
                log.line(&line);
            }
        }
        if self.debug.on.load(std::sync::atomic::Ordering::Relaxed) {
            let at_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64);
            let entry = detent_lib::app_state::debug_entry(record, at_ms);
            self.debug
                .log
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(entry.clone());
            let _ = self.app.emit(DEBUG_EVENT, entry);
        }
        if let Some(view) = detent_lib::app_state::input_view(record) {
            let _ = self.app.emit(INPUT_EVENT, view);
        }
        if let Some(view) = detent_lib::app_state::result_view(record) {
            let _ = self.app.emit(RESULT_EVENT, view);
        }
        if let Some(hud) = detent_lib::app_state::hud_of(record) {
            self.overlay.show(hud);
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.apply(record) {
            if !state.any_connected() {
                crate::dashboard::no_device(&self.app);
            }
            publish(&self.app, &self.tray, &state, self.dry_run);
        }
    }
}

fn frontmost() -> Option<String> {
    #[cfg(target_os = "macos")]
    return detent_lib::macos::frontmost_bundle_id();
    #[cfg(not(target_os = "macos"))]
    return None;
}

fn executor(dry_run: bool, app: &AppHandle) -> Arc<dyn Executor> {
    #[cfg(target_os = "macos")]
    if !dry_run {
        // Open dashboard needs a window, so the app wraps the system executor.
        return Arc::new(crate::dashboard::AppExecutor {
            inner: Arc::new(detent_lib::actions::SystemExecutor),
            app: app.clone(),
        });
    }
    let _ = (dry_run, app);
    Arc::new(RecordingExecutor::default())
}

/// Debug builds only: `DETENT_CRASH_TEST=engine` panics the engine on its
/// first input, and `DETENT_CRASH_TEST=actions` panics the first action, so
/// acceptance can check crash recovery (M9 step 3). Release builds ignore it.
mod crash_test {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use detent_lib::actions::{Executor, Outcome};
    use detent_lib::engine::Frontmost;
    use detent_lib::store::Action;

    fn wants(what: &str) -> bool {
        cfg!(debug_assertions) && std::env::var("DETENT_CRASH_TEST").is_ok_and(|v| v == what)
    }

    pub fn frontmost(inner: Frontmost) -> Frontmost {
        if !wants("engine") {
            return inner;
        }
        let armed = AtomicBool::new(true);
        Arc::new(move || {
            if armed.swap(false, Ordering::Relaxed) {
                panic!("DETENT_CRASH_TEST=engine");
            }
            inner()
        })
    }

    struct Once {
        inner: Arc<dyn Executor>,
        armed: AtomicBool,
    }

    impl Executor for Once {
        fn run(&self, action: &Action) -> Outcome {
            if self.armed.swap(false, Ordering::Relaxed) {
                panic!("DETENT_CRASH_TEST=actions");
            }
            self.inner.run(action)
        }
    }

    pub fn executor(inner: Arc<dyn Executor>) -> Arc<dyn Executor> {
        if !wants("actions") {
            return inner;
        }
        Arc::new(Once {
            inner,
            armed: AtomicBool::new(true),
        })
    }
}

/// An empty v1 config: what a first launch starts from.
pub fn empty_config() -> Config {
    Config {
        version: 2,
        devices: Vec::new(),
        ignored: Vec::new(),
        mappings: Vec::new(),
        paused: false,
    }
}

/// Loads the config and starts the engine. A missing file is a first launch:
/// Detent starts empty and the first save creates it. A file that doesn't
/// load is left alone, shown as a problem, and never overwritten (D-070).
/// The app keeps running either way.
pub fn start(launch: &Launch, app: AppHandle, tray: tray::Tray) -> Shared {
    let log = default_log_path().and_then(|p| match FileLog::open(&p) {
        Ok(log) => Some(Arc::new(log)),
        Err(e) => {
            eprintln!("detent: no log file at {}: {e}", p.display());
            None
        }
    });
    if let Some(log) = &log {
        detent_lib::engine::install_panic_hook(log.clone());
    }
    let say = |line: &str| {
        eprintln!("detent: {line}");
        if let Some(log) = &log {
            log.line(line);
        }
    };
    say(&format!(
        "starting{}",
        if launch.dry_run {
            " in dry run: actions are logged, not run"
        } else {
            ""
        }
    ));

    let path = launch.config.clone().or_else(store::default_path);
    let (config, writable, problem) = match &path {
        None => (
            empty_config(),
            false,
            Some("no home directory for the config".to_owned()),
        ),
        Some(p) => match store::load(p) {
            Ok(c) => {
                say(&format!(
                    "config {}: {} device(s), {} mapping(s)",
                    p.display(),
                    c.devices.len(),
                    c.mappings.len()
                ));
                (c, true, None)
            }
            Err(ConfigError::Missing(_)) => {
                say(&format!("no config at {} yet: starting empty", p.display()));
                (empty_config(), true, None)
            }
            Err(e) => (
                empty_config(),
                false,
                Some(format!("config not loaded: {e}")),
            ),
        },
    };
    if let Some(p) = &problem {
        say(p);
    }
    let mut initial = AppState::new(&config);
    initial.config_broken = !writable && path.is_some();
    initial.problem = problem;
    initial.set_accessibility(accessibility_granted());
    initial.debug = launch.debug;
    let debug = Debug::default();
    debug
        .on
        .store(launch.debug, std::sync::atomic::Ordering::Relaxed);
    let state = Arc::new(Mutex::new(initial));
    watch_accessibility(app.clone(), tray.clone(), state.clone(), launch.dry_run);
    crate::dashboard::watch(app.clone());
    crate::dashboard::watch_hover(app.clone());

    let engine = match Hid::new() {
        Ok(bus) => {
            let sink = Arc::new(AppSink {
                log: log.clone(),
                overlay: crate::hud::Overlay::new(app.clone()),
                state: state.clone(),
                app: app.clone(),
                tray: tray.clone(),
                dry_run: launch.dry_run,
                debug: debug.clone(),
            });
            Some(
                Engine::new(
                    bus,
                    &config,
                    crash_test::executor(executor(launch.dry_run, &app)),
                    crash_test::frontmost(Arc::new(frontmost)),
                    sink,
                    Options::default(),
                )
                .spawn(),
            )
        }
        Err(e) => {
            say(&format!("USB HID unavailable: {e}"));
            let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
            s.problem = Some(format!("USB HID unavailable: {e}"));
            None
        }
    };
    if let (true, Some(engine)) = (launch.debug, &engine) {
        engine.set_raw(true);
    }
    let shared = Shared {
        path,
        config: Mutex::new(config),
        writable: AtomicBool::new(writable),
        state,
        engine,
        app,
        tray,
        dry_run: launch.dry_run,
        log,
        debug,
    };
    let snapshot = shared
        .state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    shared.publish(&snapshot);
    shared
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn no_args_means_real_actions_and_the_default_config() {
        assert_eq!(parse_args(&[]), Launch::default());
    }

    #[test]
    fn flags_parse_in_any_order_and_unknown_ones_are_ignored() {
        let want = Launch {
            dry_run: true,
            config: Some("/tmp/c.json".into()),
            settings: false,
            debug: false,
        };
        assert_eq!(parse_args(&args("--config /tmp/c.json --dry-run")), want);
        assert_eq!(
            parse_args(&args("-psn_0_12345 --dry-run --config /tmp/c.json")),
            want
        );
    }

    #[test]
    fn debug_opens_the_window_with_debug_on() {
        let l = parse_args(&args("--debug"));
        assert!(l.debug && l.settings);
        assert!(!parse_args(&args("--settings")).debug);
    }

    #[test]
    fn settings_opens_the_window_at_launch() {
        assert!(parse_args(&args("--settings")).settings);
        assert!(!parse_args(&args("--dry-run")).settings);
    }

    #[test]
    fn a_trailing_config_flag_without_a_path_is_ignored() {
        assert_eq!(parse_args(&args("--config")).config, None);
    }
}
