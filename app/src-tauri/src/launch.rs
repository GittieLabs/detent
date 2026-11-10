//! Launch options and the engine's hookup to the tray and log.
//!
//!   detent [--dry-run] [--config PATH]
//!
//! `--dry-run` resolves and logs every press but runs nothing (until M3's
//! Test screen has its toggle, D-064). `--config` reads another config file
//! instead of `~/Library/Application Support/Detent/config.json`. Unknown
//! arguments are ignored: macOS and Tauri may pass their own.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use detent_lib::actions::{Executor, RecordingExecutor};
use detent_lib::devices::{DeviceEvent, Options};
use detent_lib::engine::{default_log_path, Engine, EngineHandle, FileLog, Record, Sink};
use detent_lib::store::{self, Config};
use detent_lib::transport::Hid;
use tauri::menu::MenuItem;
use tauri::Wry;

use crate::tray;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Launch {
    pub dry_run: bool,
    pub config: Option<PathBuf>,
}

pub fn parse_args(args: &[String]) -> Launch {
    let mut launch = Launch::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dry-run" => launch.dry_run = true,
            "--config" => launch.config = it.next().map(PathBuf::from),
            _ => {}
        }
    }
    launch
}

/// Logs every record and keeps the tray's device line current.
struct AppSink {
    log: Option<FileLog>,
    names: HashMap<String, String>,
    connected: Mutex<Vec<String>>,
    status: MenuItem<Wry>,
    dry_run: bool,
}

impl AppSink {
    fn line(&self, line: &str) {
        eprintln!("detent: {line}");
        if let Some(log) = &self.log {
            log.line(line);
        }
    }

    fn show(&self, connected: &[String]) {
        let text = tray::device_label(&tray::DeviceStatus::Running(connected), self.dry_run);
        if let Err(e) = self.status.set_text(text) {
            eprintln!("detent: could not update the tray: {e}");
        }
    }
}

impl Sink for AppSink {
    fn record(&self, record: &Record) {
        self.line(&record.to_string());
        let changed = match record {
            Record::Device(DeviceEvent::Connected(id)) => {
                let name = self
                    .names
                    .get(&id.serial)
                    .cloned()
                    .unwrap_or(id.serial.clone());
                let mut c = self.connected.lock().unwrap_or_else(|e| e.into_inner());
                c.push(name);
                Some(c.clone())
            }
            Record::Device(DeviceEvent::Disconnected { id, .. }) => {
                let name = self
                    .names
                    .get(&id.serial)
                    .cloned()
                    .unwrap_or(id.serial.clone());
                let mut c = self.connected.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(i) = c.iter().position(|n| *n == name) {
                    c.remove(i);
                }
                Some(c.clone())
            }
            _ => None,
        };
        if let Some(c) = changed {
            self.show(&c);
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

/// Loads the config and starts the engine. Problems are logged and shown in
/// the tray's device line; the app keeps running either way.
pub fn start(launch: &Launch, status: MenuItem<Wry>) -> Option<EngineHandle> {
    let log = default_log_path().and_then(|p| match FileLog::open(&p) {
        Ok(log) => Some(log),
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
    let fail = |problem: tray::DeviceStatus, line: String| {
        say(&line);
        let _ = status.set_text(tray::device_label(&problem, launch.dry_run));
        None
    };

    say(&format!(
        "starting{}",
        if launch.dry_run {
            " in dry run: actions are logged, not run"
        } else {
            ""
        }
    ));
    let Some(path) = launch.config.clone().or_else(store::default_path) else {
        return fail(
            tray::DeviceStatus::ConfigProblem,
            "no home directory for the config".into(),
        );
    };
    let config: Config = match store::load(&path) {
        Ok(c) => c,
        Err(e) => {
            return fail(
                tray::DeviceStatus::ConfigProblem,
                format!("config not loaded: {e}"),
            )
        }
    };
    say(&format!(
        "config {}: {} device(s), {} mapping(s)",
        path.display(),
        config.devices.len(),
        config.mappings.len()
    ));
    let bus = match Hid::new() {
        Ok(b) => b,
        Err(e) => {
            return fail(
                tray::DeviceStatus::UsbUnavailable,
                format!("USB HID unavailable: {e}"),
            )
        }
    };
    let sink = Arc::new(AppSink {
        log,
        names: config
            .devices
            .iter()
            .map(|d| (d.serial.clone(), d.name.clone()))
            .collect(),
        connected: Mutex::new(Vec::new()),
        status,
        dry_run: launch.dry_run,
    });
    sink.show(&[]);
    let engine = Engine::new(
        bus,
        &config,
        executor(launch.dry_run),
        Arc::new(frontmost),
        sink,
        Options::default(),
    );
    Some(engine.spawn())
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
        };
        assert_eq!(parse_args(&args("--config /tmp/c.json --dry-run")), want);
        assert_eq!(
            parse_args(&args("-psn_0_12345 --dry-run --config /tmp/c.json")),
            want
        );
    }

    #[test]
    fn a_trailing_config_flag_without_a_path_is_ignored() {
        assert_eq!(parse_args(&args("--config")).config, None);
    }
}
