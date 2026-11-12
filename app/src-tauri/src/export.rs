//! Export logs (M9, D-100): a zip on the Desktop with Detent's logs, the
//! config and a few facts about the app, made to be attached to a public
//! GitHub issue. Everything personal the config names (device and menu
//! names, AppleScript text, dashboard URLs and displays) is replaced in the
//! config and in the logs; serials, app IDs and the shape of the mappings
//! stay, since they're what a bug report needs.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::app_state::AppState;
use crate::store::{self, Action, Config, DEFAULT_DASHBOARD_URL};

/// Dashboard URLs that are the project's own and stay: the default, and
/// the repo page (the default before the docs site, D-088; schema 2 moves
/// it to the docs, D-109, but a typed link to it is still public).
const PUBLIC_URLS: &[&str] = &[
    DEFAULT_DASHBOARD_URL,
    "https://github.com/gittielabs/detent",
];

/// What a private dashboard URL becomes.
pub const REDACTED_URL: &str = "https://redacted.invalid/";
/// What a secret becomes in log text.
pub const REDACTED: &str = "[redacted]";

/// The config with its personal words replaced, and those words, for
/// [`redact_text`]. The result is still a valid config.
pub fn redact_config(config: &Config) -> (Config, Vec<String>) {
    let mut secrets = Vec::new();
    let mut out = config.clone();
    let mut displays: Vec<String> = Vec::new();
    for (i, d) in out.devices.iter_mut().enumerate() {
        secrets.push(std::mem::replace(&mut d.name, format!("Unit {}", i + 1)));
        for (j, item) in d.menu.iter_mut().enumerate() {
            secrets.push(std::mem::replace(
                &mut item.label,
                format!("Item {}", j + 1),
            ));
            redact_action(&mut item.action, &mut secrets, &mut displays);
        }
    }
    for m in &mut out.mappings {
        redact_action(&mut m.action, &mut secrets, &mut displays);
    }
    (out, secrets)
}

fn redact_action(action: &mut Action, secrets: &mut Vec<String>, displays: &mut Vec<String>) {
    match action {
        Action::AppleScript { source, .. } => {
            let n = source.chars().count();
            let old = std::mem::replace(source, format!("-- redacted ({n} characters)"));
            // The log shows a script's first line, quoted and escaped.
            for line in old.lines().map(str::trim).filter(|l| !l.is_empty()) {
                let quoted = format!("{line:?}");
                secrets.push(quoted[1..quoted.len() - 1].to_owned());
                secrets.push(line.to_owned());
            }
            secrets.push(old);
        }
        Action::OpenDashboard { url, display, .. } => {
            if !PUBLIC_URLS.contains(&url.as_str()) {
                secrets.push(std::mem::replace(url, REDACTED_URL.to_owned()));
            }
            if let Some(name) = display {
                let n = match displays.iter().position(|d| d == name) {
                    Some(i) => i + 1,
                    None => {
                        displays.push(name.clone());
                        displays.len()
                    }
                };
                secrets.push(std::mem::replace(name, format!("Display {n}")));
            }
        }
        Action::OpenApp { .. }
        | Action::VolumeUp { .. }
        | Action::VolumeDown { .. }
        | Action::VolumeSet { .. }
        | Action::MuteToggle
        | Action::SendShortcut { .. } => {}
    }
}

/// `text` with every secret replaced by [`REDACTED`], longest first so a
/// name inside a longer secret can't leave the rest behind. Secrets under
/// three characters are left: they'd mangle ordinary words.
pub fn redact_text(text: &str, secrets: &[String]) -> String {
    let mut sorted: Vec<&String> = secrets.iter().filter(|s| s.chars().count() >= 3).collect();
    sorted.sort_by_key(|s| std::cmp::Reverse(s.len()));
    sorted.dedup();
    sorted
        .into_iter()
        .fold(text.to_owned(), |t, s| t.replace(s.as_str(), REDACTED))
}

/// Facts about the app for `info.txt`: versions, permissions, recoveries,
/// and each paired unit by number with its serial, model, firmware and the
/// controls its last HELLO reported. No names.
pub fn info(app_version: &str, macos: &str, state: &AppState) -> String {
    let yes = |b: bool| if b { "yes" } else { "no" };
    let mut s = format!(
        "Detent {app_version}\n{macos}\n\nAccessibility granted: {}\nPaused: {}\nRecovered from errors since launch: {}\nProblem: {}\n",
        yes(state.accessibility.granted),
        yes(state.paused),
        state.recovered,
        state.problem.as_deref().unwrap_or("none"),
    );
    for (i, u) in state.paired.iter().enumerate() {
        let controls: Vec<&str> = u.controls.iter().map(|c| c.label.as_str()).collect();
        s.push_str(&format!(
            "\nUnit {}: {:04X}:{:04X} serial {}\n  connected: {}\n  model: {}\n  firmware: {}{}\n  controls: {}\n",
            i + 1,
            u.vid,
            u.pid,
            u.serial,
            yes(u.connected),
            u.model.as_deref().unwrap_or("unknown"),
            u.firmware.as_deref().unwrap_or("unknown"),
            if u.outdated {
                " (older protocol)"
            } else if u.newer {
                " (newer protocol than this Detent)"
            } else {
                ""
            },
            if controls.is_empty() {
                "none reported this session".to_owned()
            } else {
                controls.join(", ")
            },
        ));
    }
    s
}

/// Writes `Detent logs <stamp>.zip` to `out_dir` (` 2`, ` 3`, ... if taken)
/// holding each existing log in `logs`, `config.json` (or `config.txt`
/// saying it didn't load) and `info.txt`, all redacted with the config's
/// secrets. Zipped with `ditto`; the staging folder is removed after.
pub fn export(
    out_dir: &Path,
    stamp: &str,
    logs: &[PathBuf],
    config: Option<&Config>,
    info: &str,
) -> Result<PathBuf, String> {
    let base = format!("Detent logs {stamp}");
    let zip = (1..)
        .map(|n| match n {
            1 => out_dir.join(format!("{base}.zip")),
            n => out_dir.join(format!("{base} {n}.zip")),
        })
        .find(|p| !p.exists())
        .expect("some suffix is free");
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let staging =
        std::env::temp_dir().join(format!("detent-export-{}-{nanos}", std::process::id()));
    let folder = staging.join(&base);
    let made = (|| {
        std::fs::create_dir_all(&folder)
            .map_err(|e| format!("couldn't make {}: {e}", folder.display()))?;
        let (red, secrets) = match config {
            Some(c) => {
                let (red, secrets) = redact_config(c);
                (Some(red), secrets)
            }
            None => (None, Vec::new()),
        };
        let write = |name: &str, text: &str| {
            std::fs::write(folder.join(name), text)
                .map_err(|e| format!("couldn't write {name}: {e}"))
        };
        for log in logs {
            let Ok(text) = std::fs::read_to_string(log) else {
                continue; // no previous log yet, say
            };
            let name = log
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("detent.log");
            write(name, &redact_text(&text, &secrets))?;
        }
        match red {
            Some(red) => write("config.json", &store::to_json(&red).map_err(|e| e.to_string())?)?,
            None => write(
                "config.txt",
                "The config didn't load, so it isn't included: it could hold names and scripts that can't be found to redact.\n",
            )?,
        }
        write("info.txt", &redact_text(info, &secrets))?;
        let status = Command::new("/usr/bin/ditto")
            .args([
                "-c",
                "-k",
                "--norsrc",
                "--noextattr",
                "--noqtn",
                "--keepParent",
            ])
            .arg(&folder)
            .arg(&zip)
            .status()
            .map_err(|e| format!("couldn't run ditto: {e}"))?;
        if status.success() {
            Ok(zip.clone())
        } else {
            Err(format!("ditto couldn't make {} ({status})", zip.display()))
        }
    })();
    let _ = std::fs::remove_dir_all(&staging);
    made
}
