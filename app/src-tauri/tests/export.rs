//! Export logs (M9, D-100): a zip of the logs, a redacted config and facts
//! about the app, safe to attach to a public GitHub issue.

use std::path::{Path, PathBuf};
use std::process::Command;

use detent_lib::app_state::AppState;
use detent_lib::export;
use detent_lib::store::{self, Action, Config, DEFAULT_DASHBOARD_URL};

const SCRIPT: &str = "tell application \"Mail\" to send secret message to \"boss@example.com\"";

fn config() -> Config {
    store::parse(&format!(
        r#"{{ "version": 1,
          "devices": [{{ "name": "Keith's desk", "vid": "0x303A", "pid": "0x8FDE", "serial": "E072A1FC4EDC",
            "menu": [{{ "label": "Private thing", "action": {{ "type": "mute_toggle" }} }}] }}],
          "mappings": [
            {{ "device": "E072A1FC4EDC", "control": 1, "trigger": "press", "scope": "everywhere",
               "action": {{ "type": "applescript", "source": {} }} }},
            {{ "device": "E072A1FC4EDC", "control": 2, "trigger": "press", "scope": {{ "app": "com.apple.Safari" }},
               "action": {{ "type": "open_dashboard", "url": "https://home.internal:8443/secret", "display": "LG Ultra Keith" }} }},
            {{ "device": "E072A1FC4EDC", "control": 3, "trigger": "press", "scope": "everywhere",
               "action": {{ "type": "open_dashboard" }} }}
          ] }}"#,
        serde_json::to_string(SCRIPT).unwrap()
    ))
    .expect("test config parses")
}

#[test]
fn the_config_keeps_its_shape_but_not_its_words() {
    let (red, secrets) = export::redact_config(&config());
    let json = store::to_json(&red).expect("the redacted config is still valid");
    for gone in [
        "Keith's desk",
        "Private thing",
        "boss@example.com",
        "home.internal",
        "LG Ultra Keith",
    ] {
        assert!(!json.contains(gone), "{gone} leaked:\n{json}");
    }
    assert_eq!(red.devices[0].name, "Unit 1");
    assert_eq!(red.devices[0].menu[0].label, "Item 1");
    match &red.mappings[0].action {
        Action::AppleScript { source, .. } => {
            assert_eq!(
                source,
                &format!("-- redacted ({} characters)", SCRIPT.len())
            )
        }
        a => panic!("{a:?}"),
    }
    match &red.mappings[1].action {
        Action::OpenDashboard { url, display, .. } => {
            assert_eq!(url, export::REDACTED_URL);
            assert_eq!(display.as_deref(), Some("Display 1"));
        }
        a => panic!("{a:?}"),
    }
    // The project's own default page isn't private.
    match &red.mappings[2].action {
        Action::OpenDashboard { url, .. } => assert_eq!(url, DEFAULT_DASHBOARD_URL),
        a => panic!("{a:?}"),
    }
    // What says how Detent is used stays: serials, apps, controls.
    assert!(json.contains("E072A1FC4EDC") && json.contains("com.apple.Safari"));
    for s in [
        "Keith's desk",
        "Private thing",
        "https://home.internal:8443/secret",
        "LG Ultra Keith",
    ] {
        assert!(secrets.iter().any(|x| x == s), "{s} not in {secrets:?}");
    }
    assert!(secrets.iter().any(|x| x == SCRIPT));
}

#[test]
fn log_text_loses_every_secret_the_config_names() {
    let (_, secrets) = export::redact_config(&config());
    let log = format!(
        "1.000 resolved device Keith's desk button 1 press -> applescript {:?} ...\n\
         1.100 ran open_dashboard https://home.internal:8443/secret on LG Ultra Keith\n",
        SCRIPT.lines().next().unwrap()
    );
    let out = export::redact_text(&log, &secrets);
    for gone in [
        "Keith's desk",
        "boss@example.com",
        "home.internal",
        "LG Ultra Keith",
    ] {
        assert!(!out.contains(gone), "{gone} leaked: {out}");
    }
    assert!(
        out.contains("button 1 press") && out.contains("[redacted]"),
        "{out}"
    );
}

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("detent-export-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn zip_names(zip: &Path) -> Vec<String> {
    let out = Command::new("/usr/bin/unzip")
        .arg("-Z1")
        .arg(zip)
        .output()
        .unwrap();
    let mut names: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.ends_with('/')) // the folder itself
        .map(str::to_owned)
        .collect();
    names.sort();
    names
}

fn unzip_text(zip: &Path, name: &str) -> String {
    let out = Command::new("/usr/bin/unzip")
        .arg("-p")
        .arg(zip)
        .arg(name)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
#[cfg(target_os = "macos")]
fn the_zip_holds_both_logs_the_config_and_the_facts_all_redacted() {
    let dir = temp("zip");
    let logs = dir.join("logs");
    std::fs::create_dir_all(&logs).unwrap();
    std::fs::write(
        logs.join("detent.log"),
        "2.0 device Keith's desk connected\n",
    )
    .unwrap();
    std::fs::write(
        logs.join("detent.log.1"),
        "1.0 ran open_dashboard https://home.internal:8443/secret\n",
    )
    .unwrap();
    let out = dir.join("Desktop");
    std::fs::create_dir_all(&out).unwrap();

    let state = AppState::new(&config());
    let info = export::info("0.1.0", "macOS 27.0", &state);
    let zip = export::export(
        &out,
        "2026-11-11 14.30.00",
        &[logs.join("detent.log"), logs.join("detent.log.1")],
        Some(&config()),
        &info,
    )
    .expect("export works");

    assert_eq!(zip, out.join("Detent logs 2026-11-11 14.30.00.zip"));
    let prefix = "Detent logs 2026-11-11 14.30.00/";
    assert_eq!(
        zip_names(&zip),
        ["config.json", "detent.log", "detent.log.1", "info.txt"]
            .iter()
            .map(|n| format!("{prefix}{n}"))
            .collect::<Vec<_>>()
    );
    let all: String = ["config.json", "detent.log", "detent.log.1", "info.txt"]
        .iter()
        .map(|n| unzip_text(&zip, &format!("{prefix}{n}")))
        .collect();
    for gone in [
        "Keith's desk",
        "boss@example.com",
        "home.internal",
        "LG Ultra Keith",
        "Private thing",
    ] {
        assert!(!all.contains(gone), "{gone} leaked:\n{all}");
    }
    let info = unzip_text(&zip, &format!("{prefix}info.txt"));
    assert!(
        info.contains("Detent 0.1.0") && info.contains("macOS 27.0"),
        "{info}"
    );
    assert!(info.contains("E072A1FC4EDC"), "{info}");
    // No staging folder is left behind on the Desktop.
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[cfg(target_os = "macos")]
fn missing_logs_and_an_unloaded_config_are_noted_not_fatal() {
    let dir = temp("missing");
    let state = AppState::new(&config());
    let zip = export::export(
        &dir,
        "x",
        &[dir.join("nope.log")],
        None,
        &export::info("0.1.0", "macOS", &state),
    )
    .unwrap();
    let names = zip_names(&zip);
    assert!(names.iter().any(|n| n.ends_with("config.txt")), "{names:?}");
    assert!(!names.iter().any(|n| n.ends_with("nope.log")), "{names:?}");
    let note = unzip_text(&zip, "Detent logs x/config.txt");
    assert!(note.contains("didn't load"), "{note}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[cfg(target_os = "macos")]
fn a_second_export_in_the_same_second_gets_its_own_name() {
    let dir = temp("twice");
    let state = AppState::new(&config());
    let info = export::info("0.1.0", "macOS", &state);
    let a = export::export(&dir, "s", &[], Some(&config()), &info).unwrap();
    let b = export::export(&dir, "s", &[], Some(&config()), &info).unwrap();
    assert_ne!(a, b);
    assert_eq!(b, dir.join("Detent logs s 2.zip"));
}
