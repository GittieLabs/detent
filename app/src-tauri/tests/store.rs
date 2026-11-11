//! Config schema v1: parsing, validation, and error messages that say where.

use detent_lib::store::{self, Config, DashboardMode};
use detent_lib::store::{
    load, parse, Action, ConfigError, Scope, Trigger, DEFAULT_APPLESCRIPT_TIMEOUT_MS,
    DEFAULT_VOLUME_STEP,
};

const EXAMPLE: &str = include_str!("../examples/config.example.json");

/// A valid config with one device and one mapping; tests change one thing.
fn base(mapping_extra: &str) -> String {
    format!(
        r#"{{
  "version": 1,
  "devices": [{{ "name": "Desk", "vid": "0x303A", "pid": "0x8FDE", "serial": "E072A1FC4EDC" }}],
  "mappings": [{{ "device": "E072A1FC4EDC", "control": 1, "trigger": "press", "scope": "everywhere",
                 "action": {{ "type": "open_app", "bundle_id": "com.apple.mail" }} {mapping_extra} }}]
}}"#
    )
}

/// Replaces `from` with `to` once, failing loudly if `from` isn't there, so a
/// typo in a test can't quietly test the unmodified config.
fn edit(json: &str, from: &str, to: &str) -> String {
    assert!(
        json.contains(from),
        "test bug: {from:?} not found in the config"
    );
    json.replacen(from, to, 1)
}

fn err(json: &str) -> String {
    match parse(json) {
        Ok(c) => panic!("expected an error, parsed {c:?}"),
        Err(e) => e.to_string(),
    }
}

fn assert_err(json: &str, expected: &str) {
    let e = err(json);
    assert!(
        e.contains(expected),
        "error {e:?} should contain {expected:?}"
    );
}

// --- valid configs ---

#[test]
fn example_config_parses() {
    let c = parse(EXAMPLE).expect("the example is valid");
    assert_eq!(c.version, 1);
    assert!(!c.devices.is_empty());
    assert!(!c.mappings.is_empty());
}

#[test]
fn minimal_config_has_empty_lists() {
    let c = parse(r#"{ "version": 1 }"#).unwrap();
    assert!(c.devices.is_empty() && c.ignored.is_empty() && c.mappings.is_empty());
}

#[test]
fn parses_every_field() {
    let json = r#"{
      "version": 1,
      "devices": [{ "name": "Desk", "vid": "0x303A", "pid": "0x8FDE", "serial": "E072A1FC4EDC" }],
      "ignored": [{ "vid": "0x303A", "pid": "0x8FDE", "serial": "AAAAAAAAAAAA" }],
      "mappings": [
        { "device": "E072A1FC4EDC", "control": 1, "trigger": "press", "scope": "everywhere",
          "action": { "type": "open_app", "bundle_id": "com.apple.mail" } },
        { "device": "E072A1FC4EDC", "control": 1, "trigger": "press", "scope": { "app": "com.apple.Safari" },
          "action": { "type": "mute_toggle" } },
        { "device": "E072A1FC4EDC", "control": 2, "trigger": "long_press", "scope": "everywhere",
          "action": { "type": "applescript", "source": "beep" } },
        { "device": "E072A1FC4EDC", "control": 5, "trigger": "clockwise", "scope": "everywhere",
          "action": { "type": "volume_up" } },
        { "device": "E072A1FC4EDC", "control": 5, "trigger": "counterclockwise", "scope": "everywhere",
          "action": { "type": "volume_down", "step": 0.125 } },
        { "device": "E072A1FC4EDC", "control": 3, "trigger": "press", "scope": "everywhere",
          "action": { "type": "volume_set", "level": 0.25 } },
        { "device": "E072A1FC4EDC", "control": 4, "trigger": "press", "scope": "everywhere",
          "action": { "type": "applescript", "source": "beep", "timeout_ms": 2000 } }
      ]
    }"#;
    let c = parse(json).unwrap();
    let d = &c.devices[0];
    assert_eq!(
        (d.vid, d.pid, d.serial.as_str(), d.name.as_str()),
        (0x303A, 0x8FDE, "E072A1FC4EDC", "Desk")
    );
    assert_eq!(c.ignored[0].serial, "AAAAAAAAAAAA");
    let m = &c.mappings;
    assert_eq!(m[0].trigger, Trigger::Press);
    assert_eq!(m[0].scope, Scope::Everywhere);
    assert_eq!(
        m[0].action,
        Action::OpenApp {
            bundle_id: "com.apple.mail".into()
        }
    );
    assert_eq!(m[1].scope, Scope::App("com.apple.Safari".into()));
    assert_eq!(m[1].action, Action::MuteToggle);
    assert_eq!(m[2].trigger, Trigger::LongPress);
    assert_eq!(
        m[2].action,
        Action::AppleScript {
            source: "beep".into(),
            timeout_ms: DEFAULT_APPLESCRIPT_TIMEOUT_MS
        }
    );
    assert_eq!(m[3].trigger, Trigger::Clockwise);
    assert_eq!(
        m[3].action,
        Action::VolumeUp {
            step: DEFAULT_VOLUME_STEP
        }
    );
    assert_eq!(m[4].trigger, Trigger::Counterclockwise);
    assert_eq!(m[4].action, Action::VolumeDown { step: 0.125 });
    assert_eq!(m[5].action, Action::VolumeSet { level: 0.25 });
    assert_eq!(
        m[6].action,
        Action::AppleScript {
            source: "beep".into(),
            timeout_ms: 2000
        }
    );
}

#[test]
fn hex_ids_are_case_insensitive_and_accept_numbers() {
    let c = parse(&edit(&base(""), r#""vid": "0x303A""#, r#""vid": "0x303a""#)).unwrap();
    assert_eq!(c.devices[0].vid, 0x303A);
    let c = parse(&edit(&base(""), r#""pid": "0x8FDE""#, r#""pid": 36830"#)).unwrap();
    assert_eq!(c.devices[0].pid, 0x8FDE);
}

// --- syntax and structure ---

#[test]
fn malformed_json_reports_line_and_column() {
    let e = err("{\n  \"version\": 1,\n  \"devices\": [,]\n}");
    assert!(e.contains("line 3"), "{e}");
}

#[test]
fn version_is_required_and_must_be_1() {
    assert_err(r#"{ "devices": [] }"#, "missing field `version`");
    assert_err(
        r#"{ "version": 2 }"#,
        "unsupported config version 2 (this Detent reads version 1)",
    );
}

#[test]
fn unknown_fields_are_rejected_with_their_path() {
    assert_err(
        r#"{ "version": 1, "colour": "red" }"#,
        "unknown field `colour`",
    );
    assert_err(&base(r#", "colour": "red""#), "mappings[0]");
    assert_err(&base(r#", "colour": "red""#), "unknown field `colour`");
}

#[test]
fn unknown_trigger_and_action_types_name_the_field() {
    let e = err(&edit(
        &base(""),
        r#""trigger": "press""#,
        r#""trigger": "double_tap""#,
    ));
    assert!(
        e.contains("mappings[0].trigger") && e.contains("double_tap"),
        "{e}"
    );
    let e = err(&edit(
        &base(""),
        r#""type": "open_app""#,
        r#""type": "launch""#,
    ));
    assert!(
        e.contains("mappings[0].action") && e.contains("launch"),
        "{e}"
    );
}

// --- devices ---

#[test]
fn bad_ids_are_rejected() {
    assert_err(&edit(&base(""), "\"0x303A\"", "\"0xZZ\""), "devices[0].vid");
    assert_err(
        &edit(&base(""), "\"0x303A\"", "\"0x1FFFF\""),
        "devices[0].vid",
    );
    assert_err(&edit(&base(""), "\"0x8FDE\"", "\"8FDE\""), "devices[0].pid");
}

#[test]
fn device_fields_must_be_non_empty_and_serials_unique() {
    assert_err(
        &edit(&base(""), r#""name": "Desk""#, r#""name": " ""#),
        "devices[0].name: must not be empty",
    );
    let json = edit(
        &base(""),
        r#""serial": "E072A1FC4EDC" }"#,
        r#""serial": "" }"#,
    );
    assert_err(&json, "devices[0].serial: must not be empty");
    let dup = edit(
        &base(""),
        r#""serial": "E072A1FC4EDC" }]"#,
        r#""serial": "E072A1FC4EDC" }, { "name": "Two", "vid": "0x303A", "pid": "0x8FDE", "serial": "E072A1FC4EDC" }]"#,
    );
    assert_err(
        &dup,
        "devices[1].serial: E072A1FC4EDC is already listed in devices[0]",
    );
}

// --- mappings ---

#[test]
fn mappings_must_name_a_listed_device() {
    assert_err(
        &edit(
            &base(""),
            r#""device": "E072A1FC4EDC""#,
            r#""device": "NOPE""#,
        ),
        "mappings[0].device: NOPE isn't in devices",
    );
}

#[test]
fn app_scope_needs_a_bundle_id() {
    assert_err(
        &edit(
            &base(""),
            r#""scope": "everywhere""#,
            r#""scope": { "app": "" }"#,
        ),
        "mappings[0].scope.app: must be a bundle ID",
    );
    assert_err(
        &edit(
            &base(""),
            r#""scope": "everywhere""#,
            r#""scope": { "app": "Safari" }"#,
        ),
        "mappings[0].scope.app: must be a bundle ID",
    );
}

#[test]
fn duplicate_mappings_are_ambiguous() {
    // Written out in full: building this by string replacement once matched
    // nothing and silently tested a valid config.
    let config = |second_scope: &str| {
        format!(
            r#"{{
  "version": 1,
  "devices": [{{ "name": "Desk", "vid": "0x303A", "pid": "0x8FDE", "serial": "E072A1FC4EDC" }}],
  "mappings": [
    {{ "device": "E072A1FC4EDC", "control": 1, "trigger": "press", "scope": "everywhere",
       "action": {{ "type": "open_app", "bundle_id": "com.apple.mail" }} }},
    {{ "device": "E072A1FC4EDC", "control": 1, "trigger": "press", "scope": {second_scope},
       "action": {{ "type": "mute_toggle" }} }}
  ]
}}"#
        )
    };
    assert_err(
        &config(r#""everywhere""#),
        "mappings[1]: same device, control, trigger, and scope as mappings[0]",
    );
    // A different scope is not a duplicate: app scope overrides Everywhere.
    let ok = parse(&config(r#"{ "app": "com.apple.Safari" }"#)).unwrap();
    assert_eq!(ok.mappings.len(), 2);
}

#[test]
fn control_must_fit_a_byte() {
    assert_err(
        &edit(&base(""), r#""control": 1"#, r#""control": 300"#),
        "mappings[0].control",
    );
}

// --- actions ---

#[test]
fn action_values_are_checked() {
    let act = |a: &str| {
        edit(
            &base(""),
            r#"{ "type": "open_app", "bundle_id": "com.apple.mail" }"#,
            a,
        )
    };
    assert_err(
        &act(r#"{ "type": "open_app", "bundle_id": "Mail" }"#),
        "mappings[0].action.bundle_id: must be a bundle ID",
    );
    assert_err(
        &act(r#"{ "type": "volume_set", "level": 1.5 }"#),
        "mappings[0].action.level: must be from 0 to 1",
    );
    assert_err(
        &act(r#"{ "type": "volume_up", "step": 0 }"#),
        "mappings[0].action.step: must be more than 0 and at most 1",
    );
    assert_err(
        &act(r#"{ "type": "volume_down", "step": 2 }"#),
        "mappings[0].action.step: must be more than 0 and at most 1",
    );
    assert_err(&act(r#"{ "type": "volume_set" }"#), "missing field `level`");
    assert_err(
        &act(r#"{ "type": "applescript", "source": "  " }"#),
        "mappings[0].action.source: must not be empty",
    );
    assert_err(
        &act(r#"{ "type": "applescript", "source": "beep", "timeout_ms": 10 }"#),
        "mappings[0].action.timeout_ms: must be from 100 to 60000",
    );
}

// --- loading files ---

fn temp_file(name: &str, contents: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("detent-store-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, contents).unwrap();
    path
}

#[test]
fn load_reads_a_file() {
    let path = temp_file("ok.json", &base(""));
    assert_eq!(load(&path).unwrap().mappings.len(), 1);
}

#[test]
fn load_reports_a_missing_file_and_bad_contents_with_the_path() {
    let missing = std::env::temp_dir().join("detent-store-test-no-such-file.json");
    let e = load(&missing).unwrap_err();
    assert!(matches!(e, ConfigError::Missing(_)), "{e:?}");
    assert!(e.to_string().contains("no config at"), "{e}");
    let bad = temp_file("bad.json", r#"{ "version": 9 }"#);
    let e = load(&bad).unwrap_err().to_string();
    assert!(
        e.contains("bad.json") && e.contains("unsupported config version 9"),
        "{e}"
    );
}

// Saving (M3): canonical JSON, written atomically, validated first.

fn temp_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("detent-store-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn example() -> Config {
    store::parse(include_str!("../examples/config.example.json")).expect("example parses")
}

#[test]
fn saved_text_parses_back_to_the_same_config() {
    let config = example();
    let text = store::to_json(&config).expect("valid config serializes");
    assert_eq!(store::parse(&text).expect("saved text parses"), config);
}

#[test]
fn saved_text_is_canonical_and_readable() {
    let text = store::to_json(&example()).unwrap();
    assert!(
        text.starts_with("{\n  \"version\": 1,\n  \"devices\": ["),
        "{text}"
    );
    assert!(
        text.contains("\"vid\": \"0x303A\""),
        "USB IDs stay hex: {text}"
    );
    assert!(text.contains("\"pid\": \"0x8FDE\""));
    assert!(text.contains("\"scope\": \"everywhere\""));
    assert!(text.contains("\"app\": \"com.apple.Safari\""));
    assert!(text.contains("\"type\": \"mute_toggle\""));
    // Defaults are written out, so the file says what will happen.
    assert!(text.contains("\"step\": 0.0625"), "{text}");
    assert!(text.contains("\"timeout_ms\": 5000"));
    assert!(text.ends_with("}\n"));
    // Saving twice gives the same bytes.
    assert_eq!(store::to_json(&store::parse(&text).unwrap()).unwrap(), text);
}

#[test]
fn save_writes_the_file_creating_its_folder() {
    let dir = temp_dir("create");
    let path = dir.join("nested/config.json");
    store::save(&path, &example()).expect("saves");
    assert_eq!(store::load(&path).unwrap(), example());
    let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(
        leftovers,
        vec![std::ffi::OsString::from("config.json")],
        "no temp file left"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_replaces_an_existing_file() {
    let dir = temp_dir("replace");
    let path = dir.join("config.json");
    store::save(&path, &example()).unwrap();
    let mut smaller = example();
    smaller.mappings.truncate(1);
    store::save(&path, &smaller).unwrap();
    assert_eq!(store::load(&path).unwrap(), smaller);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_invalid_config_is_refused_and_the_file_is_untouched() {
    let dir = temp_dir("invalid");
    let path = dir.join("config.json");
    store::save(&path, &example()).unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut bad = example();
    bad.mappings[0].device = "NOT-LISTED".into();
    match store::save(&path, &bad) {
        Err(store::ConfigError::Invalid { path, .. }) => assert_eq!(path, "mappings[0].device"),
        other => panic!("{other:?}"),
    }
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_write_leaves_the_old_file_intact() {
    let dir = temp_dir("fail");
    let path = dir.join("config.json");
    store::save(&path, &example()).unwrap();
    let before = std::fs::read(&path).unwrap();
    // Block the temp file's name with a directory, so the write fails.
    std::fs::create_dir_all(dir.join("config.json.tmp")).unwrap();
    let mut changed = example();
    changed.mappings.clear();
    assert!(matches!(
        store::save(&path, &changed),
        Err(store::ConfigError::Io { .. })
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn paused_defaults_off_and_is_saved() {
    let c = parse(r#"{ "version": 1 }"#).unwrap();
    assert!(!c.paused);
    let c = parse(r#"{ "version": 1, "paused": true }"#).unwrap();
    assert!(c.paused);
    let text = store::to_json(&c).unwrap();
    assert!(text.contains("\"paused\": true"), "{text}");
    assert_eq!(parse(&text).unwrap(), c);
    // Written out when off too, like every default (D-069).
    let text = store::to_json(&example()).unwrap();
    assert!(text.contains("\"paused\": false"), "{text}");
}

#[test]
fn send_shortcut_parses_validates_and_saves() {
    let json = r#"{ "version": 1,
      "devices": [{ "name": "Desk", "vid": "0x303A", "pid": "0x8FDE", "serial": "E072A1FC4EDC" }],
      "mappings": [{ "device": "E072A1FC4EDC", "control": 3, "trigger": "press",
                     "scope": { "app": "com.electron.plasticity" },
                     "action": { "type": "send_shortcut", "keys": "e" } }] }"#;
    let c = parse(json).unwrap();
    assert_eq!(
        c.mappings[0].action,
        Action::SendShortcut { keys: "e".into() }
    );
    let text = store::to_json(&c).unwrap();
    assert!(text.contains("\"type\": \"send_shortcut\""), "{text}");
    assert_eq!(parse(&text).unwrap(), c);

    assert_err(
        &json.replace(r#""keys": "e""#, r#""keys": "cmd+banana""#),
        "mappings[0].action.keys",
    );
    assert_err(
        &json.replace(r#""keys": "e""#, r#""keys": """#),
        "mappings[0].action.keys",
    );
    assert_err(
        &json.replace(r#""keys": "e""#, r#""keys": "e", "extra": 1"#),
        "extra",
    );
}

// Device menu items (M6): app owned OLED menu entries per unit.

const MENU_CONFIG: &str = r#"{ "version": 1,
  "devices": [{ "name": "Desk", "vid": "0x303A", "pid": "0x8FDE", "serial": "E072A1FC4EDC",
    "menu": [
      { "label": "Mail", "action": { "type": "open_app", "bundle_id": "com.apple.mail" } },
      { "label": "Mute", "action": { "type": "mute_toggle" } }
    ] }] }"#;

#[test]
fn device_menu_items_parse_and_save() {
    let c = parse(MENU_CONFIG).unwrap();
    let menu = &c.devices[0].menu;
    assert_eq!(menu.len(), 2);
    assert_eq!(menu[1].label, "Mute");
    assert_eq!(menu[1].action, Action::MuteToggle);
    let text = store::to_json(&c).unwrap();
    assert_eq!(parse(&text).unwrap(), c);
    // A unit without a menu reads as an empty one, and is written out empty.
    let plain = parse(r#"{ "version": 1, "devices": [{ "name": "D", "vid": "0x303A", "pid": "0x8FDE", "serial": "S" }] }"#).unwrap();
    assert!(plain.devices[0].menu.is_empty());
    assert!(store::to_json(&plain).unwrap().contains("\"menu\": []"));
}

#[test]
fn bad_device_menu_items_name_the_field() {
    assert_err(
        &MENU_CONFIG.replace(r#""label": "Mute""#, r#""label": """#),
        "devices[0].menu[1].label",
    );
    assert_err(
        &MENU_CONFIG.replace(r#""label": "Mute""#, r#""label": "A label far too long""#),
        "devices[0].menu[1].label",
    );
    assert_err(
        &MENU_CONFIG.replace(r#""label": "Mute""#, r#""label": "Café""#),
        "devices[0].menu[1].label",
    );
    assert_err(
        &MENU_CONFIG.replace(r#""bundle_id": "com.apple.mail""#, r#""bundle_id": "Mail""#),
        "devices[0].menu[0].action.bundle_id",
    );
    let nine: Vec<String> = (1..=9)
        .map(|i| format!(r#"{{ "label": "Item {i}", "action": {{ "type": "mute_toggle" }} }}"#))
        .collect();
    let too_many = format!(
        r#"{{ "version": 1, "devices": [{{ "name": "D", "vid": "0x303A", "pid": "0x8FDE",
             "serial": "S", "menu": [{}] }}] }}"#,
        nine.join(",")
    );
    assert_err(&too_many, "devices[0].menu");
}

// Open dashboard (M6, D-088).

fn dashboard_config(action: &str) -> String {
    format!(
        r#"{{ "version": 1,
          "devices": [{{ "name": "D", "vid": "0x303A", "pid": "0x8FDE", "serial": "S" }}],
          "mappings": [{{ "device": "S", "control": 1, "trigger": "press", "scope": "everywhere",
                          "action": {action} }}] }}"#
    )
}

#[test]
fn open_dashboard_defaults_to_the_documentation_site() {
    let c = parse(&dashboard_config(r#"{ "type": "open_dashboard" }"#)).unwrap();
    assert_eq!(
        c.mappings[0].action,
        Action::OpenDashboard {
            url: "https://gittielabs.github.io/detent/".into(),
            display: None,
            mode: DashboardMode::Fullscreen,
            close_without_device: true,
        }
    );
    let c = parse(&dashboard_config(
        r#"{ "type": "open_dashboard", "url": "http://homepage.local:3000", "display": "Studio Display", "mode": "window", "close_without_device": false }"#,
    ))
    .unwrap();
    assert_eq!(
        c.mappings[0].action,
        Action::OpenDashboard {
            url: "http://homepage.local:3000".into(),
            display: Some("Studio Display".into()),
            mode: DashboardMode::Window,
            close_without_device: false,
        }
    );
    let text = store::to_json(&c).unwrap();
    assert_eq!(parse(&text).unwrap(), c);
}

#[test]
fn open_dashboard_needs_a_web_url_and_a_real_display_name() {
    // Serde reports a bad value inside a tagged action at the action's path,
    // but names the value and the choices.
    assert_err(
        &dashboard_config(r#"{ "type": "open_dashboard", "mode": "kiosk" }"#),
        "unknown variant `kiosk`, expected `fullscreen` or `window`",
    );
    for bad in [
        r#""url": "file:///etc/passwd""#,
        r#""url": "github.com""#,
        r#""url": "https://a b""#,
    ] {
        assert_err(
            &dashboard_config(&format!(r#"{{ "type": "open_dashboard", {bad} }}"#)),
            "mappings[0].action.url",
        );
    }
    assert_err(
        &dashboard_config(r#"{ "type": "open_dashboard", "display": " " }"#),
        "mappings[0].action.display",
    );
}

#[test]
fn a_broken_config_is_moved_aside_not_deleted() {
    let dir = temp_dir("move-aside");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.json");
    std::fs::write(&path, "{ not json").unwrap();
    let aside = store::move_aside(&path, 1_700_000_000).unwrap();
    assert_eq!(aside, dir.join("config.json.bad-1700000000"));
    assert!(!path.exists());
    assert_eq!(std::fs::read_to_string(&aside).unwrap(), "{ not json");

    // A second one in the same second doesn't overwrite the first.
    std::fs::write(&path, "{ also broken").unwrap();
    let second = store::move_aside(&path, 1_700_000_000).unwrap();
    assert_eq!(second, dir.join("config.json.bad-1700000000-2"));
    assert_eq!(std::fs::read_to_string(&aside).unwrap(), "{ not json");
    assert_eq!(std::fs::read_to_string(&second).unwrap(), "{ also broken");
}

#[test]
fn moving_aside_a_missing_config_fails() {
    let dir = temp_dir("move-aside-missing");
    assert!(store::move_aside(&dir.join("config.json"), 1).is_err());
}

#[test]
fn the_ui_offers_the_same_default_dashboard_url() {
    let api = include_str!("../../ui/api.ts");
    let want = format!(
        "export const DEFAULT_DASHBOARD_URL = \"{}\";",
        store::DEFAULT_DASHBOARD_URL
    );
    assert!(api.contains(&want), "app/ui/api.ts should have: {want}");
}
