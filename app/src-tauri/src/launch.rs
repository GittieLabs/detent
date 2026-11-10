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
use std::sync::{Arc, Mutex};

use detent_lib::actions::{Executor, RecordingExecutor};
use detent_lib::app_state::AppState;
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
}

pub fn parse_args(args: &[String]) -> Launch {
    let mut launch = Launch::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dry-run" => launch.dry_run = true,
            "--settings" => launch.settings = true,
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

/// Everything the app shares between the engine's sink, the tray, and the
/// commands. Managed by Tauri.
pub struct Shared {
    /// Where the config lives, `None` without a home directory.
    pub path: Option<PathBuf>,
    /// The config as last loaded or saved.
    pub config: Mutex<Config>,
    /// False when the file on disk didn't load: saving would overwrite what
    /// the user wrote, so commands refuse until it's fixed (D-070).
    pub writable: bool,
    pub state: Arc<Mutex<AppState>>,
    pub engine: Option<EngineHandle>,
    pub app: AppHandle,
    pub tray: tray::Tray,
    pub dry_run: bool,
    /// Detent's log, shared with the engine's sink.
    pub log: Option<Arc<FileLog>>,
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
    state: Arc<Mutex<AppState>>,
    app: AppHandle,
    tray: tray::Tray,
    dry_run: bool,
}

impl Sink for AppSink {
    fn record(&self, record: &Record) {
        let line = record.to_string();
        eprintln!("detent: {line}");
        if let Some(log) = &self.log {
            log.line(&line);
        }
        if let Some(view) = detent_lib::app_state::input_view(record) {
            let _ = self.app.emit(INPUT_EVENT, view);
        }
        if let Some(view) = detent_lib::app_state::result_view(record) {
            let _ = self.app.emit(RESULT_EVENT, view);
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.apply(record) {
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

fn executor(dry_run: bool) -> Arc<dyn Executor> {
    #[cfg(target_os = "macos")]
    if !dry_run {
        return Arc::new(detent_lib::actions::SystemExecutor);
    }
    let _ = dry_run;
    Arc::new(RecordingExecutor::default())
}

/// An empty v1 config: what a first launch starts from.
pub fn empty_config() -> Config {
    Config {
        version: 1,
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
    initial.problem = problem;
    initial.set_accessibility(accessibility_granted());
    let state = Arc::new(Mutex::new(initial));
    watch_accessibility(app.clone(), tray.clone(), state.clone(), launch.dry_run);

    let engine = match Hid::new() {
        Ok(bus) => {
            let sink = Arc::new(AppSink {
                log: log.clone(),
                state: state.clone(),
                app: app.clone(),
                tray: tray.clone(),
                dry_run: launch.dry_run,
            });
            Some(
                Engine::new(
                    bus,
                    &config,
                    executor(launch.dry_run),
                    Arc::new(frontmost),
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
    let shared = Shared {
        path,
        config: Mutex::new(config),
        writable,
        state,
        engine,
        app,
        tray,
        dry_run: launch.dry_run,
        log,
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
        };
        assert_eq!(parse_args(&args("--config /tmp/c.json --dry-run")), want);
        assert_eq!(
            parse_args(&args("-psn_0_12345 --dry-run --config /tmp/c.json")),
            want
        );
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
