//! App state for the UI (built from the config and engine records) and the
//! config edits behind the Tauri commands. Pure: no Tauri, no hardware.

use detent_lib::app_state::{self, AppState, Candidate, EditError, UnknownUnit};
use detent_lib::devices::{DeviceEvent, DeviceId, Lost};
use detent_lib::engine::Record;
use detent_lib::store::{self, Config, UnitRef};
use detent_protocol::codec::{Control, ControlKind, EncoderMode, FirmwareVersion, Hello};
use detent_protocol::ids::{PID, VID};

const PAIRED: &str = "E072A1FC4EDC";
const OTHER: &str = "AABBCCDDEEFF";

fn config() -> Config {
    store::parse(&format!(
        r#"{{ "version": 1,
          "devices": [{{ "name": "Desk controller", "vid": "0x303A", "pid": "0x8FDE", "serial": "{PAIRED}" }}],
          "mappings": [
            {{ "device": "{PAIRED}", "control": 1, "trigger": "press", "scope": "everywhere",
               "action": {{ "type": "mute_toggle" }} }},
            {{ "device": "{PAIRED}", "control": 2, "trigger": "press", "scope": "everywhere",
               "action": {{ "type": "volume_up" }} }}
          ] }}"#
    ))
    .unwrap()
}

fn id(serial: &str) -> DeviceId {
    DeviceId {
        vid: VID,
        pid: PID,
        serial: serial.into(),
    }
}

fn hello() -> Hello {
    Hello {
        firmware: FirmwareVersion {
            major: 0,
            minor: 1,
            patch: 2,
        },
        encoder_mode: EncoderMode::Volume,
        controls: vec![
            Control {
                id: 1,
                kind: ControlKind::Button,
            },
            Control {
                id: 2,
                kind: ControlKind::Button,
            },
            Control {
                id: 5,
                kind: ControlKind::Encoder,
            },
        ],
    }
}

fn dev(e: DeviceEvent) -> Record {
    Record::Device(e)
}

fn connected(serial: &str) -> Record {
    dev(DeviceEvent::Connected {
        id: id(serial),
        product: Some("Detent Dev".into()),
    })
}

// State from the config and records.

#[test]
fn a_fresh_state_lists_paired_units_as_not_connected() {
    let s = AppState::new(&config());
    assert_eq!(s.paired.len(), 1);
    let u = &s.paired[0];
    assert_eq!(
        (u.name.as_str(), u.serial.as_str()),
        ("Desk controller", PAIRED)
    );
    assert!(!u.connected);
    assert_eq!((u.model.as_deref(), u.firmware.as_deref()), (None, None));
    assert!(s.candidates.is_empty() && s.unknown.is_empty() && s.ignored.is_empty());
}

#[test]
fn connecting_and_hello_fill_in_the_live_details() {
    let mut s = AppState::new(&config());
    assert!(s.apply(&connected(PAIRED)));
    assert!(s.apply(&dev(DeviceEvent::Hello {
        id: id(PAIRED),
        hello: hello()
    })));
    let u = &s.paired[0];
    assert!(u.connected);
    assert_eq!(
        u.model.as_deref(),
        Some("Dev"),
        "product string without `Detent `"
    );
    assert_eq!(u.firmware.as_deref(), Some("0.1.2"));
    assert_eq!((u.buttons, u.encoders), (2, 1));

    assert!(s.apply(&dev(DeviceEvent::Disconnected {
        id: id(PAIRED),
        why: Lost::Detached
    })));
    let u = &s.paired[0];
    assert!(!u.connected);
    assert_eq!(
        u.firmware.as_deref(),
        Some("0.1.2"),
        "last known details stay"
    );
}

#[test]
fn records_that_change_nothing_say_so() {
    let mut s = AppState::new(&config());
    s.apply(&connected(PAIRED));
    assert!(!s.apply(&dev(DeviceEvent::Input {
        id: id(PAIRED),
        message: detent_protocol::codec::DeviceMessage::MenuSelect { item: 1 }
    })));
    assert!(!s.apply(&Record::Reloaded {
        devices: 1,
        mappings: 2
    }));
}

#[test]
fn unknown_units_come_and_go() {
    let mut s = AppState::new(&config());
    let unknown = DeviceEvent::Unknown {
        vid: VID,
        pid: PID,
        serial: Some(OTHER.into()),
    };
    assert!(s.apply(&dev(unknown)));
    assert_eq!(
        s.unknown,
        vec![UnknownUnit {
            vid: VID,
            pid: PID,
            serial: Some(OTHER.into())
        }]
    );
    assert!(s.apply(&dev(DeviceEvent::UnknownGone {
        vid: VID,
        pid: PID,
        serial: Some(OTHER.into())
    })));
    assert!(s.unknown.is_empty());
}

#[test]
fn candidates_are_found_described_and_picked() {
    let mut s = AppState::new(&config());
    s.set_identifying(true);
    s.apply(&dev(DeviceEvent::CandidateFound {
        id: id(OTHER),
        product: Some("Detent Dev".into()),
    }));
    s.apply(&dev(DeviceEvent::CandidateHello {
        id: id(OTHER),
        hello: hello(),
    }));
    assert_eq!(
        s.candidates,
        vec![Candidate {
            vid: VID,
            pid: PID,
            serial: OTHER.into(),
            model: Some("Dev".into()),
            firmware: Some("0.1.2".into()),
            buttons: 2,
            encoders: 1,
            picked: false,
            suggested_name: "Detent Dev".into(),
        }]
    );
    s.apply(&dev(DeviceEvent::CandidatePressed(id(OTHER))));
    assert!(s.candidates[0].picked);
    s.apply(&dev(DeviceEvent::CandidateGone(id(OTHER))));
    assert!(s.candidates.is_empty());
}

#[test]
fn only_the_latest_pressed_candidate_is_picked() {
    let mut s = AppState::new(&config());
    s.set_identifying(true);
    let second = "112233445566";
    for serial in [OTHER, second] {
        s.apply(&dev(DeviceEvent::CandidateFound {
            id: id(serial),
            product: None,
        }));
    }
    s.apply(&dev(DeviceEvent::CandidatePressed(id(OTHER))));
    s.apply(&dev(DeviceEvent::CandidatePressed(id(second))));
    let picked: Vec<_> = s.candidates.iter().filter(|c| c.picked).collect();
    assert_eq!(picked.len(), 1);
    assert_eq!(picked[0].serial, second);
}

#[test]
fn ending_identification_clears_candidates() {
    let mut s = AppState::new(&config());
    s.set_identifying(true);
    s.apply(&dev(DeviceEvent::CandidateFound {
        id: id(OTHER),
        product: None,
    }));
    s.set_identifying(false);
    assert!(!s.identifying);
    assert!(s.candidates.is_empty());
}

#[test]
fn a_new_config_keeps_live_details_and_drops_what_left() {
    let mut s = AppState::new(&config());
    s.apply(&connected(PAIRED));
    s.apply(&dev(DeviceEvent::Unknown {
        vid: VID,
        pid: PID,
        serial: Some(OTHER.into()),
    }));
    // OTHER gets paired and PAIRED renamed.
    let mut c = app_state::pair(&config(), VID, PID, OTHER, "Second").unwrap();
    c = app_state::rename(&c, PAIRED, "Desk").unwrap();
    s.set_config(&c);
    let names: Vec<_> = s.paired.iter().map(|u| u.name.as_str()).collect();
    assert_eq!(names, ["Desk", "Second"]);
    assert!(s.paired[0].connected, "still connected after a rename");
    assert!(s.unknown.is_empty(), "a paired unit is no longer unknown");
}

#[test]
fn a_problem_is_shown_until_cleared() {
    let mut s = AppState::new(&config());
    s.problem = Some("config not loaded".into());
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["problem"], "config not loaded");
    assert_eq!(json["paired"][0]["serial"], PAIRED, "serializes for the UI");
}

// Config edits.

#[test]
fn pairing_adds_the_unit_and_takes_it_off_the_ignore_list() {
    let mut c = config();
    c.ignored.push(UnitRef {
        vid: VID,
        pid: PID,
        serial: OTHER.into(),
    });
    let c = app_state::pair(&c, VID, PID, OTHER, "  Second  ").unwrap();
    assert_eq!(c.devices.len(), 2);
    assert_eq!(c.devices[1].name, "Second", "trimmed");
    assert!(c.ignored.is_empty());
    assert!(store::to_json(&c).is_ok(), "still a valid config");
}

#[test]
fn pairing_refuses_bad_names_and_duplicates() {
    assert_eq!(
        app_state::pair(&config(), VID, PID, OTHER, "   "),
        Err(EditError::EmptyName)
    );
    assert_eq!(
        app_state::pair(&config(), VID, PID, &PAIRED.to_lowercase(), "Again"),
        Err(EditError::AlreadyPaired(PAIRED.to_lowercase()))
    );
}

#[test]
fn renaming_changes_only_the_name() {
    let c = app_state::rename(&config(), PAIRED, "Desk").unwrap();
    assert_eq!(c.devices[0].name, "Desk");
    assert_eq!(c.mappings, config().mappings);
    assert_eq!(
        app_state::rename(&config(), OTHER, "x"),
        Err(EditError::NotPaired(OTHER.into()))
    );
    assert_eq!(
        app_state::rename(&config(), PAIRED, ""),
        Err(EditError::EmptyName)
    );
}

#[test]
fn unpairing_removes_the_unit_and_its_mappings() {
    let (c, removed) = app_state::unpair(&config(), PAIRED).unwrap();
    assert!(c.devices.is_empty());
    assert!(c.mappings.is_empty());
    assert_eq!(removed, 2);
    assert!(store::to_json(&c).is_ok());
    assert_eq!(app_state::mappings_of(&config(), PAIRED), 2);
    assert_eq!(
        app_state::unpair(&config(), OTHER).map(|(_, n)| n),
        Err(EditError::NotPaired(OTHER.into()))
    );
}

#[test]
fn ignoring_and_unignoring() {
    let c = app_state::ignore(&config(), VID, PID, OTHER).unwrap();
    assert_eq!(c.ignored.len(), 1);
    assert_eq!(
        app_state::ignore(&c, VID, PID, OTHER),
        Err(EditError::AlreadyIgnored(OTHER.into()))
    );
    assert_eq!(
        app_state::ignore(&config(), VID, PID, PAIRED),
        Err(EditError::AlreadyPaired(PAIRED.into())),
        "unpair first"
    );
    let c = app_state::unignore(&c, OTHER).unwrap();
    assert!(c.ignored.is_empty());
    assert_eq!(
        app_state::unignore(&c, OTHER),
        Err(EditError::NotIgnored(OTHER.into()))
    );
}

#[test]
fn edit_errors_read_well() {
    assert_eq!(EditError::EmptyName.to_string(), "the name can't be empty");
    assert_eq!(
        EditError::AlreadyPaired(OTHER.into()).to_string(),
        format!("{OTHER} is already paired")
    );
}

fn found(s: &mut AppState, serial: &str, product: Option<&str>) {
    s.apply(&dev(DeviceEvent::CandidateFound {
        id: id(serial),
        product: product.map(Into::into),
    }));
}

#[test]
fn a_candidates_name_is_suggested_from_its_model() {
    let mut s = AppState::new(&config());
    s.set_identifying(true);
    found(&mut s, OTHER, Some("Detent Dev"));
    found(&mut s, "112233445566", None);
    let names: Vec<_> = s
        .candidates
        .iter()
        .map(|c| c.suggested_name.as_str())
        .collect();
    assert_eq!(names, ["Detent Dev", "Detent"]);
}

#[test]
fn a_suggested_name_never_repeats_a_paired_one() {
    let c = app_state::rename(&config(), PAIRED, "detent dev").unwrap();
    let c = app_state::pair(&c, VID, PID, "112233445566", "Detent Dev 2").unwrap();
    let mut s = AppState::new(&c);
    s.set_identifying(true);
    found(&mut s, OTHER, Some("Detent Dev"));
    assert_eq!(s.candidates[0].suggested_name, "Detent Dev 3");

    // Renaming the paired units frees the name, and the suggestion follows.
    let c = app_state::rename(&c, PAIRED, "Desk").unwrap();
    let c = app_state::rename(&c, "112233445566", "Shelf").unwrap();
    s.set_config(&c);
    assert_eq!(s.candidates[0].suggested_name, "Detent Dev");
}

#[test]
fn pausing_is_a_config_edit_the_state_follows() {
    let mut s = AppState::new(&config());
    assert!(!s.paused);
    let c = app_state::set_paused(&config(), true);
    assert!(c.paused);
    assert_eq!(c.mappings, config().mappings, "mappings untouched");
    s.set_config(&c);
    assert!(s.paused);
    s.set_config(&app_state::set_paused(&c, false));
    assert!(!s.paused);
}

#[test]
fn the_dry_run_flag_follows_the_engine() {
    let mut s = AppState::new(&config());
    assert!(!s.dry_run);
    assert!(s.apply(&Record::DryRun { on: true }));
    assert!(s.dry_run);
    assert!(!s.apply(&Record::DryRun { on: true }), "no change, no push");
    s.apply(&Record::DryRun { on: false });
    assert!(!s.dry_run);
}

#[test]
fn a_resolution_becomes_an_input_view_for_the_test_screen() {
    use detent_lib::store::{Action, Trigger};
    let resolved = Record::Resolved {
        id: id(PAIRED),
        control: 2,
        trigger: Trigger::Press,
        frontmost: Some("com.apple.Safari".into()),
        action: Some(Action::MuteToggle),
        count: 1,
        paused: false,
        dry_run: true,
    };
    let view = app_state::input_view(&resolved).expect("a view");
    assert_eq!(view.serial, PAIRED);
    assert_eq!(view.control, 2);
    assert_eq!(view.trigger, Trigger::Press);
    assert_eq!(view.frontmost.as_deref(), Some("com.apple.Safari"));
    assert_eq!(view.action.as_deref(), Some("mute_toggle"));
    assert!(view.dry_run && !view.paused);
    let json = serde_json::to_value(&view).unwrap();
    assert_eq!(json["trigger"], "press");

    let mut unmapped = resolved.clone();
    if let Record::Resolved { action, .. } = &mut unmapped {
        *action = None;
    }
    assert_eq!(app_state::input_view(&unmapped).unwrap().action, None);
    assert_eq!(app_state::input_view(&Record::DryRun { on: true }), None);
}
