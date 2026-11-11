//! Every config shape Detent has written or documented must keep loading
//! (D-008, D-103), like `protocol/tests/frozen.rs` for the wire format.
//!
//! Schema 2 (D-109) is the first real migration: every fixture is schema 1.
//!
//! The files in `tests/fixtures/configs/` are frozen: never edit or delete
//! one. A schema change that breaks one needs a `store::MIGRATIONS` step
//! and a `SCHEMA_VERSION` bump, and the fixture stays as it was. Add a new
//! fixture whenever a milestone changes what Detent writes.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use detent_lib::store::{self, ConfigError, Migration, SCHEMA_VERSION};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/configs")
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures().join(name)).expect("fixture exists")
}

/// Every frozen fixture, by file name.
const FROZEN: &[&str] = &[
    "m2-example.json",
    "m2-hand-written.json",
    "m3-first-save.json",
    "m7-keith-redacted.json",
];

#[test]
fn every_fixture_is_listed() {
    let mut on_disk: Vec<String> = std::fs::read_dir(fixtures())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    on_disk.sort();
    assert_eq!(on_disk, FROZEN, "list each new fixture in FROZEN");
}

#[test]
fn every_frozen_config_still_loads_and_saves_back_the_same() {
    for name in FROZEN {
        let config = store::parse(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let saved = store::to_json(&config).unwrap_or_else(|e| panic!("{name}: {e}"));
        let again = store::parse(&saved).unwrap_or_else(|e| panic!("{name} saved: {e}"));
        assert_eq!(again, config, "{name}");
    }
}

#[test]
fn the_frozen_configs_keep_what_they_said() {
    let m2 = store::parse(&fixture("m2-hand-written.json")).unwrap();
    assert_eq!(m2.devices.len(), 1);
    assert_eq!(m2.mappings.len(), 5);
    assert!(!m2.paused, "Pause defaults off when the field is missing");

    let m3 = store::parse(&fixture("m3-first-save.json")).unwrap();
    assert_eq!(m3.devices[0].name, "Detent Dev");
    assert!(m3.mappings.is_empty());

    let m7 = store::parse(&fixture("m7-keith-redacted.json")).unwrap();
    assert_eq!(m7.devices.len(), 3);
    assert_eq!(m7.mappings.len(), 17);
    assert_eq!(m7.devices[0].menu.len(), 2);
}

// The migration machinery, with made up steps: schema 1 has no earlier
// version to migrate from yet.

/// A pretend v0 that called `mappings` `bindings`.
fn v0_to_v1(mut v: Value) -> Result<Value, String> {
    let o = v.as_object_mut().ok_or("not an object")?;
    if let Some(b) = o.remove("bindings") {
        o.insert("mappings".into(), b);
    }
    Ok(v)
}

const STEPS: &[Migration] = &[Migration {
    from: 0,
    run: v0_to_v1,
}];

#[test]
fn an_older_version_is_migrated_step_by_step() {
    let v0 = json!({ "version": 0, "bindings": [] });
    let (v1, from) = store::migrate(v0, STEPS, 1).unwrap();
    assert_eq!(from, Some(0));
    assert_eq!(v1, json!({ "version": 1, "mappings": [] }));
}

#[test]
fn the_current_version_is_left_alone() {
    let v1 = json!({ "version": 1, "mappings": [] });
    let (same, from) = store::migrate(v1.clone(), STEPS, 1).unwrap();
    assert_eq!((same, from), (v1, None));
}

#[test]
fn a_version_with_no_path_forward_is_refused() {
    let err = store::migrate(json!({ "version": 0 }), &[], 1).unwrap_err();
    assert!(matches!(err, ConfigError::UnsupportedVersion(0)), "{err}");
    let err = store::migrate(json!({ "version": 2 }), STEPS, 1).unwrap_err();
    assert!(matches!(err, ConfigError::UnsupportedVersion(2)), "{err}");
}

#[test]
fn a_failing_step_names_its_versions() {
    fn broken(_: Value) -> Result<Value, String> {
        Err("no bindings".into())
    }
    let steps = [Migration {
        from: 0,
        run: broken,
    }];
    let err = store::migrate(json!({ "version": 0 }), &steps, 1).unwrap_err();
    assert_eq!(
        err.to_string(),
        "couldn't migrate the config from version 0 to 1: no bindings"
    );
}

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("detent-migrate-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap();
}

#[test]
fn loading_a_migrated_config_keeps_the_old_file_as_a_backup() {
    let dir = temp("backup");
    let path = dir.join("config.json");
    let old = r#"{ "version": 0, "bindings": [] }"#;
    write(&path, old);
    let config = store::load_with(&path, STEPS, 1).unwrap();
    assert!(config.mappings.is_empty());
    let backup = dir.join("config.json.v0.bak");
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), old);
    // The file itself changes only when Detent next saves.
    assert_eq!(std::fs::read_to_string(&path).unwrap(), old);

    // A second launch before any save keeps the first backup.
    write(&backup, "first backup");
    store::load_with(&path, STEPS, 1).unwrap();
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), "first backup");
}

#[test]
fn a_current_config_makes_no_backup() {
    let dir = temp("no-backup");
    let path = dir.join("config.json");
    let current = store::to_json(&store::parse(&fixture("m3-first-save.json")).unwrap()).unwrap();
    write(&path, &current);
    store::load(&path).unwrap();
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
}

#[test]
fn a_future_version_is_refused_and_left_untouched() {
    let dir = temp("future");
    let path = dir.join("config.json");
    let future = format!(r#"{{ "version": {} }}"#, SCHEMA_VERSION + 1);
    write(&path, &future);
    let err = store::load(&path).unwrap_err();
    assert!(
        err.to_string().contains("unsupported config version"),
        "{err}"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), future);
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
}

// Schema 2 (D-109): the old default dashboard page becomes the docs site.

const REPO: &str = "https://github.com/gittielabs/detent";

fn dashboard_url(a: &detent_lib::store::Action) -> &str {
    match a {
        detent_lib::store::Action::OpenDashboard { url, .. } => url,
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_old_default_dashboard_page_moves_to_the_docs_site() {
    let v1 = format!(
        r#"{{ "version": 1,
          "devices": [{{ "name": "D", "vid": "0x303A", "pid": "0x8FDE", "serial": "S",
            "menu": [{{ "label": "Docs", "action": {{ "type": "open_dashboard", "url": "{REPO}" }} }}] }}],
          "mappings": [
            {{ "device": "S", "control": 1, "trigger": "press", "scope": "everywhere",
               "action": {{ "type": "open_dashboard", "url": "{REPO}", "mode": "window" }} }},
            {{ "device": "S", "control": 2, "trigger": "press", "scope": "everywhere",
               "action": {{ "type": "open_dashboard", "url": "{REPO}/issues" }} }},
            {{ "device": "S", "control": 3, "trigger": "press", "scope": "everywhere",
               "action": {{ "type": "open_app", "bundle_id": "com.apple.mail" }} }}
          ] }}"#
    );
    let c = store::parse(&v1).unwrap();
    assert_eq!(c.version, 2);
    let docs = store::DEFAULT_DASHBOARD_URL;
    assert_eq!(dashboard_url(&c.mappings[0].action), docs);
    assert_eq!(dashboard_url(&c.devices[0].menu[0].action), docs);
    // Anything else, even another page of the repo, is the user's choice.
    assert_eq!(
        dashboard_url(&c.mappings[1].action),
        format!("{REPO}/issues")
    );
    // Other fields of the action are kept.
    assert!(matches!(
        &c.mappings[0].action,
        detent_lib::store::Action::OpenDashboard {
            mode: detent_lib::store::DashboardMode::Window,
            ..
        }
    ));
}

#[test]
fn loading_a_v1_config_keeps_it_as_a_backup() {
    let dir = temp("v1-backup");
    let path = dir.join("config.json");
    let text = fixture("m7-keith-redacted.json");
    write(&path, &text);
    let c = store::load(&path).unwrap();
    assert_eq!(c.version, 2);
    assert_eq!(
        std::fs::read_to_string(dir.join("config.json.v1.bak")).unwrap(),
        text
    );
}
