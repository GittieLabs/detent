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

// Mappings (M4): each unit's controls from its HELLO, and mapping edits.

use detent_lib::app_state::ControlView;
use detent_lib::store::{Action, Mapping, Scope, Trigger};

/// The Detent Dev layout: buttons 1 to 4, the encoder 5, its push switch 6.
fn dev_hello(mode: EncoderMode) -> Hello {
    let button = |id| Control {
        id,
        kind: ControlKind::Button,
    };
    Hello {
        encoder_mode: mode,
        controls: vec![
            button(1),
            button(2),
            button(3),
            button(4),
            button(6),
            Control {
                id: 5,
                kind: ControlKind::Encoder,
            },
        ],
        ..hello()
    }
}

fn seen(mode: EncoderMode) -> AppState {
    let mut s = AppState::new(&config());
    s.apply(&connected(PAIRED));
    s.apply(&dev(DeviceEvent::Hello {
        id: id(PAIRED),
        hello: dev_hello(mode),
    }));
    s
}

fn mapping(control: u8, trigger: Trigger, scope: Scope, action: Action) -> Mapping {
    Mapping {
        device: PAIRED.into(),
        control,
        trigger,
        scope,
        action,
    }
}

#[test]
fn a_unit_lists_its_controls_from_hello_with_the_encoder_handled_by_the_device() {
    let s = seen(EncoderMode::Volume);
    let controls = &s.paired[0].controls;
    let summary: Vec<_> = controls
        .iter()
        .map(|c| (c.id, c.label.as_str(), c.handled_by_device))
        .collect();
    assert_eq!(
        summary,
        [
            (1, "Button 1", false),
            (2, "Button 2", false),
            (3, "Button 3", false),
            (4, "Button 4", false),
            (6, "Encoder push", true),
            (5, "Encoder", true),
        ]
    );
    // Mapped mode hands both to the app (mapping them arrives in M6).
    let s = seen(EncoderMode::Mapped);
    assert!(s.paired[0].controls.iter().all(|c| !c.handled_by_device));
    // Not seen this session: no controls to draw.
    assert!(AppState::new(&config()).paired[0].controls.is_empty());
}

#[test]
fn a_unit_lists_its_own_mappings_and_they_follow_the_config() {
    let mut s = seen(EncoderMode::Volume);
    assert_eq!(s.paired[0].mappings, config().mappings);
    let c = app_state::remove_mapping(&config(), PAIRED, 1, Trigger::Press, &Scope::Everywhere)
        .unwrap();
    s.set_config(&c);
    assert_eq!(s.paired[0].mappings.len(), 1);
    assert_eq!(s.paired[0].controls.len(), 6, "controls survive a reload");
}

#[test]
fn setting_a_mapping_adds_or_replaces_and_leaves_app_scoped_ones_alone() {
    let s = seen(EncoderMode::Volume);
    let controls = &s.paired[0].controls;
    let safari = mapping(
        3,
        Trigger::Press,
        Scope::App("com.apple.Safari".into()),
        Action::MuteToggle,
    );
    let mut c = config();
    c.mappings.push(safari.clone());

    // A new control and trigger: added.
    let calc = mapping(
        3,
        Trigger::LongPress,
        Scope::Everywhere,
        Action::OpenApp {
            bundle_id: "com.apple.calculator".into(),
        },
    );
    let c = app_state::set_mapping(&c, controls, calc.clone()).unwrap();
    assert_eq!(c.mappings.len(), 4);
    assert!(c.mappings.contains(&calc));

    // Same control, trigger and scope: replaced in place.
    let quieter = mapping(
        2,
        Trigger::Press,
        Scope::Everywhere,
        Action::VolumeDown { step: 0.0625 },
    );
    let c2 = app_state::set_mapping(&c, controls, quieter.clone()).unwrap();
    assert_eq!(c2.mappings.len(), 4);
    assert_eq!(c2.mappings[1], quieter, "kept its place");

    // An Everywhere edit on control 3 press leaves the Safari one alone.
    let mute = mapping(3, Trigger::Press, Scope::Everywhere, Action::MuteToggle);
    let c3 = app_state::set_mapping(&c2, controls, mute.clone()).unwrap();
    assert!(c3.mappings.contains(&safari) && c3.mappings.contains(&mute));
    assert!(store::to_json(&c3).is_ok(), "still a valid config");
}

#[test]
fn removing_a_mapping_needs_an_exact_match() {
    let c = app_state::remove_mapping(&config(), PAIRED, 2, Trigger::Press, &Scope::Everywhere)
        .unwrap();
    assert_eq!(c.mappings.len(), 1);
    assert_eq!(
        app_state::remove_mapping(&c, PAIRED, 2, Trigger::Press, &Scope::Everywhere),
        Err(EditError::NotMapped)
    );
    assert_eq!(
        app_state::remove_mapping(
            &config(),
            PAIRED,
            1,
            Trigger::Press,
            &Scope::App("com.apple.Safari".into())
        ),
        Err(EditError::NotMapped),
        "scope must match too"
    );
}

#[test]
fn bad_mappings_are_refused_with_a_reason() {
    let s = seen(EncoderMode::Volume);
    let controls = &s.paired[0].controls;
    let set = |m| app_state::set_mapping(&config(), controls, m);
    let mute = |control, trigger| mapping(control, trigger, Scope::Everywhere, Action::MuteToggle);

    assert_eq!(
        set(mute(9, Trigger::Press)),
        Err(EditError::NoSuchControl(9))
    );
    assert_eq!(
        set(mute(6, Trigger::Press)),
        Err(EditError::HandledByDevice(6))
    );
    assert_eq!(
        set(mute(1, Trigger::Clockwise)),
        Err(EditError::WrongTrigger(1))
    );
    let loud = mapping(
        1,
        Trigger::Press,
        Scope::Everywhere,
        Action::VolumeSet { level: 2.0 },
    );
    assert_eq!(
        set(loud).unwrap_err().to_string(),
        "level: must be from 0 to 1"
    );
    let bad_scope = mapping(
        1,
        Trigger::Press,
        Scope::App("Safari".into()),
        Action::MuteToggle,
    );
    assert_eq!(
        set(bad_scope).unwrap_err().to_string(),
        "app: must be a bundle ID such as com.apple.mail"
    );
    let mut other = mute(1, Trigger::Press);
    other.device = OTHER.into();
    assert_eq!(set(other), Err(EditError::NotPaired(OTHER.into())));
    // No controls known (the unit hasn't connected this session).
    let none: &[ControlView] = &[];
    assert_eq!(
        app_state::set_mapping(&config(), none, mute(1, Trigger::Press)),
        Err(EditError::NoSuchControl(1))
    );
}

#[test]
fn mappings_and_controls_cross_to_the_window_as_the_ui_expects() {
    // What `ui/api.ts` sends for `set_mapping`.
    let m: Mapping = serde_json::from_str(
        r#"{ "device": "E072A1FC4EDC", "control": 1, "trigger": "long_press",
             "scope": { "app": "com.apple.Safari" },
             "action": { "type": "volume_set", "level": 0.25 } }"#,
    )
    .unwrap();
    assert_eq!(m.trigger, Trigger::LongPress);
    assert_eq!(m.scope, Scope::App("com.apple.Safari".into()));
    let everywhere: Scope = serde_json::from_str(r#""everywhere""#).unwrap();
    assert_eq!(everywhere, Scope::Everywhere);

    let s = seen(EncoderMode::Volume);
    let json = serde_json::to_value(&s.paired[0].controls[0]).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "id": 1, "type": "button", "label": "Button 1", "handled_by_device": false,
            "triggers": ["press", "long_press"]
        })
    );
}

// Starter template (D-077).

fn just_paired(serial: &str) -> Config {
    app_state::pair(
        &store::parse(r#"{ "version": 1 }"#).unwrap(),
        VID,
        PID,
        serial,
        "New",
    )
    .unwrap()
}

#[test]
fn the_detent_dev_template_is_keiths_setup() {
    let t = detent_lib::models::starter_template(VID, PID);
    let summary: Vec<_> = t
        .iter()
        .map(|m| (m.control, m.trigger, m.scope.clone(), m.action.clone()))
        .collect();
    assert_eq!(
        summary[..4],
        [
            (
                1,
                Trigger::Press,
                Scope::Everywhere,
                Action::OpenApp {
                    bundle_id: "com.apple.mail".into()
                }
            ),
            (
                1,
                Trigger::Press,
                Scope::App("com.apple.Safari".into()),
                Action::MuteToggle
            ),
            (
                2,
                Trigger::Press,
                Scope::Everywhere,
                Action::VolumeUp { step: 0.0625 }
            ),
            (3, Trigger::Press, Scope::Everywhere, Action::MuteToggle),
        ]
    );
    assert!(
        matches!(&summary[4], (4, Trigger::Press, Scope::Everywhere, Action::AppleScript { source, .. })
            if source.contains("reopen") && source.contains("activate")),
        "{:?}",
        summary[4]
    );
    assert_eq!(t.len(), 5);
    assert!(detent_lib::models::starter_template(VID, 0x1234).is_empty());
    // The Pico has no template yet: its pins are still a placeholder (M7).
    assert!(detent_lib::models::starter_template(
        detent_protocol::ids::PICO_VID,
        detent_protocol::ids::PICO_PID
    )
    .is_empty());
}

#[test]
fn applying_the_template_maps_the_new_unit_once() {
    let c = app_state::apply_template(&just_paired(OTHER), OTHER).unwrap();
    assert_eq!(c.mappings.len(), 5);
    assert!(c.mappings.iter().all(|m| m.device == OTHER));
    assert!(store::to_json(&c).is_ok(), "the template passes the loader");
    let again = app_state::apply_template(&c, OTHER).unwrap();
    assert_eq!(again.mappings, c.mappings, "nothing duplicated");
}

#[test]
fn the_template_never_replaces_a_mapping_the_unit_has() {
    let mut c = just_paired(OTHER);
    let mine = Mapping {
        device: OTHER.into(),
        control: 2,
        trigger: Trigger::Press,
        scope: Scope::Everywhere,
        action: Action::MuteToggle,
    };
    c.mappings.push(mine.clone());
    let c = app_state::apply_template(&c, OTHER).unwrap();
    assert_eq!(c.mappings.len(), 5, "4 added, button 2 kept");
    assert!(c.mappings.contains(&mine));
    assert_eq!(
        app_state::apply_template(&c, "000000000000"),
        Err(EditError::NotPaired("000000000000".into()))
    );
}

#[test]
fn a_paired_unit_shows_the_template_it_would_get() {
    let s = AppState::new(&just_paired(OTHER));
    let t = &s.paired[0].template;
    assert_eq!(t.len(), 5);
    assert!(t.iter().all(|m| m.device == OTHER));
    // Once applied there's nothing left to add.
    let s = AppState::new(&app_state::apply_template(&just_paired(OTHER), OTHER).unwrap());
    assert!(s.paired[0].template.is_empty());
}

// The Test screen's results (M4 task 7).

#[test]
fn an_action_result_reads_plainly_for_the_test_screen() {
    use detent_lib::actions::Done;
    use std::time::Duration;
    let ran = |outcome| Record::Ran {
        id: id(PAIRED),
        action: Action::VolumeUp { step: 0.25 },
        outcome,
        waited: Duration::from_millis(1),
        took: Duration::from_millis(9),
    };
    let text = |r: &Record| app_state::result_view(r).map(|v| (v.outcome, v.ran, v.ok));

    assert_eq!(
        text(&ran(Ok(Done::Volume {
            from: 0.13,
            to: 0.38
        }))),
        Some(("was 13%, now 38%".into(), true, true))
    );
    assert_eq!(
        text(&ran(Ok(Done::Recorded))),
        Some(("not run (dry run)".into(), false, true))
    );
    assert_eq!(
        text(&ran(Ok(Done::Opened("com.apple.mail".into())))),
        Some(("opened".into(), true, true))
    );
    assert_eq!(
        text(&ran(Ok(Done::Muted(true)))),
        Some(("muted".into(), true, true))
    );
    assert_eq!(
        text(&ran(Ok(Done::Script("42".into())))),
        Some(("script returned \"42\"".into(), true, true))
    );
    let failed = app_state::result_view(&ran(Err(detent_lib::actions::ActionError::Invalid(
        Action::VolumeUp { step: 0.0 }.check_args().unwrap_err(),
    ))))
    .unwrap();
    assert!(
        !failed.ok && failed.outcome.starts_with("failed: "),
        "{failed:?}"
    );

    let v = app_state::result_view(&ran(Ok(Done::Recorded))).unwrap();
    assert_eq!(v.serial, PAIRED);
    assert_eq!(v.action, "volume_up 25%");
    assert_eq!(app_state::result_view(&Record::DryRun { on: true }), None);
}

// Accessibility banner (M5): shown only when a shortcut mapping needs the
// grant and Detent doesn't have it.

#[test]
fn the_accessibility_banner_shows_only_when_a_shortcut_needs_it() {
    let mut s = AppState::new(&config());
    assert!(!s.accessibility.needed, "no shortcut mapping");
    assert!(!s.accessibility.banner());

    let shortcut = Mapping {
        device: PAIRED.into(),
        control: 1,
        trigger: Trigger::LongPress,
        scope: Scope::Everywhere,
        action: Action::SendShortcut {
            keys: "cmd+shift+e".into(),
        },
    };
    let mut c = config();
    c.mappings.push(shortcut);
    s.set_config(&c);
    assert!(s.accessibility.needed);
    assert!(s.accessibility.banner(), "not granted yet");

    assert!(s.set_accessibility(true), "a change");
    assert!(!s.accessibility.banner());
    assert!(!s.set_accessibility(true), "no change, no push");

    s.set_config(&config());
    s.set_accessibility(false);
    assert!(!s.accessibility.banner(), "no longer needed");
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["accessibility"]["banner"], false);
}

// Volume overlay (D-083): what it shows for a result.

#[test]
fn the_volume_overlay_shows_after_a_volume_action_that_ran() {
    use detent_lib::actions::Done;
    use detent_lib::app_state::{hud_of, Hud};
    use std::time::Duration;
    let ran = |action, outcome| Record::Ran {
        id: id(PAIRED),
        action,
        outcome,
        waited: Duration::ZERO,
        took: Duration::ZERO,
    };
    let up = Action::VolumeUp { step: 0.0625 };
    assert_eq!(
        hud_of(&ran(
            up.clone(),
            Ok(Done::Volume {
                from: 0.31,
                to: 0.375
            })
        )),
        Some(Hud::Level(38))
    );
    assert_eq!(
        hud_of(&ran(Action::MuteToggle, Ok(Done::Muted(true)))),
        Some(Hud::Muted(true))
    );
    // Nothing for a dry run, a failure, or another action.
    assert_eq!(hud_of(&ran(up.clone(), Ok(Done::Recorded))), None);
    assert_eq!(
        hud_of(&ran(
            up,
            Err(detent_lib::actions::ActionError::Keys(
                detent_lib::keys::Refusal::NoEvent
            ))
        )),
        None
    );
    assert_eq!(
        hud_of(&ran(
            Action::OpenApp {
                bundle_id: "com.apple.mail".into()
            },
            Ok(Done::Opened("com.apple.mail".into()))
        )),
        None
    );
    assert_eq!(hud_of(&Record::DryRun { on: true }), None);
}

#[test]
fn the_controls_follow_an_encoder_mode_change() {
    let mut s = seen(EncoderMode::Volume);
    let handled = |s: &AppState| {
        s.paired[0]
            .controls
            .iter()
            .filter(|c| c.handled_by_device)
            .map(|c| c.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(handled(&s), [6, 5]);
    assert!(s.apply(&dev(DeviceEvent::Mode {
        id: id(PAIRED),
        mode: EncoderMode::Mapped
    })));
    assert!(handled(&s).is_empty(), "Mapped: the app maps them");
    s.apply(&dev(DeviceEvent::Mode {
        id: id(PAIRED),
        mode: EncoderMode::Menu,
    }));
    assert_eq!(handled(&s), [6, 5], "Menu: the device has them");
}

// Mapped encoder mappings (M6): each control says which triggers it can fire.

#[test]
fn each_control_lists_the_triggers_it_can_fire() {
    let s = seen(EncoderMode::Mapped);
    let triggers = |id: u8| {
        s.paired[0]
            .controls
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .triggers
            .clone()
    };
    assert_eq!(triggers(1), [Trigger::Press, Trigger::LongPress]);
    assert_eq!(triggers(5), [Trigger::Counterclockwise, Trigger::Clockwise]);
    // The push switch's long press opens the device menu in every mode.
    assert_eq!(triggers(6), [Trigger::Press]);
}

#[test]
fn encoder_mappings_are_taken_in_mapped_mode_only() {
    let turn = |trigger| Mapping {
        device: PAIRED.into(),
        control: 5,
        trigger,
        scope: Scope::Everywhere,
        action: Action::VolumeUp { step: 0.02 },
    };
    let mapped = seen(EncoderMode::Mapped);
    let controls = &mapped.paired[0].controls;
    let c = app_state::set_mapping(&config(), controls, turn(Trigger::Clockwise)).unwrap();
    assert!(c.mappings.contains(&turn(Trigger::Clockwise)));
    assert_eq!(
        app_state::set_mapping(&config(), controls, turn(Trigger::Press)),
        Err(EditError::WrongTrigger(5))
    );
    let mut push = turn(Trigger::LongPress);
    push.control = 6;
    assert_eq!(
        app_state::set_mapping(&config(), controls, push),
        Err(EditError::WrongTrigger(6)),
        "the long press opens the device menu"
    );

    let volume = seen(EncoderMode::Volume);
    assert_eq!(
        app_state::set_mapping(
            &config(),
            &volume.paired[0].controls,
            turn(Trigger::Clockwise)
        ),
        Err(EditError::HandledByDevice(5))
    );
}

// Device menu editor (M6 task 5): edits to a unit's app owned menu items.

use detent_lib::store::MenuItem;

fn menu_item(label: &str) -> MenuItem {
    MenuItem {
        label: label.into(),
        action: Action::MuteToggle,
    }
}

fn labels(c: &Config) -> Vec<String> {
    c.devices[0].menu.iter().map(|m| m.label.clone()).collect()
}

#[test]
fn menu_items_are_added_replaced_moved_and_removed() {
    let c = app_state::set_menu_item(&config(), PAIRED, None, menu_item("Mail")).unwrap();
    let c = app_state::set_menu_item(&c, PAIRED, None, menu_item("Mute")).unwrap();
    assert_eq!(labels(&c), ["Mail", "Mute"]);
    let c = app_state::set_menu_item(&c, PAIRED, Some(0), menu_item("Notes")).unwrap();
    assert_eq!(labels(&c), ["Notes", "Mute"]);
    let c = app_state::move_menu_item(&c, PAIRED, 1, 0).unwrap();
    assert_eq!(labels(&c), ["Mute", "Notes"]);
    let c = app_state::remove_menu_item(&c, PAIRED, 0).unwrap();
    assert_eq!(labels(&c), ["Notes"]);
    assert!(store::to_json(&c).is_ok());
    // The state shows each unit's menu.
    let s = AppState::new(&c);
    assert_eq!(s.paired[0].menu, c.devices[0].menu);
}

#[test]
fn bad_menu_edits_are_refused_with_a_reason() {
    let one = app_state::set_menu_item(&config(), PAIRED, None, menu_item("Mail")).unwrap();
    assert_eq!(
        app_state::set_menu_item(&one, PAIRED, None, menu_item("")),
        Err(EditError::BadLabel)
    );
    assert_eq!(
        app_state::set_menu_item(&one, PAIRED, None, menu_item("A label far too long")),
        Err(EditError::BadLabel)
    );
    assert_eq!(
        app_state::set_menu_item(&one, PAIRED, Some(5), menu_item("X")),
        Err(EditError::NoSuchItem(5))
    );
    assert_eq!(
        app_state::remove_menu_item(&one, PAIRED, 3),
        Err(EditError::NoSuchItem(3))
    );
    assert_eq!(
        app_state::move_menu_item(&one, PAIRED, 0, 4),
        Err(EditError::NoSuchItem(4))
    );
    assert_eq!(
        app_state::set_menu_item(&one, OTHER, None, menu_item("X")),
        Err(EditError::NotPaired(OTHER.into()))
    );
    let mut bad = menu_item("Vol");
    bad.action = Action::VolumeSet { level: 2.0 };
    assert_eq!(
        app_state::set_menu_item(&one, PAIRED, None, bad)
            .unwrap_err()
            .to_string(),
        "level: must be from 0 to 1"
    );
    let mut full = config();
    for i in 0..8 {
        full =
            app_state::set_menu_item(&full, PAIRED, None, menu_item(&format!("Item {i}"))).unwrap();
    }
    assert_eq!(
        app_state::set_menu_item(&full, PAIRED, None, menu_item("Ninth")),
        Err(EditError::MenuFull)
    );
    assert_eq!(
        EditError::BadLabel.to_string(),
        "a menu label is 1 to 16 plain characters (letters, digits, punctuation, spaces)"
    );
}

#[test]
fn any_connected_follows_the_paired_units() {
    let mut s = AppState::new(&config());
    assert!(!s.any_connected());
    s.apply(&connected(PAIRED));
    assert!(s.any_connected());
    s.apply(&dev(DeviceEvent::Disconnected {
        id: id(PAIRED),
        why: Lost::Detached,
    }));
    assert!(
        !s.any_connected(),
        "unplugged: the dashboard may close (D-090)"
    );
}

#[test]
fn a_picos_encoder_push_is_known_from_its_model() {
    use detent_protocol::ids::{PICO_PID, PICO_VID};
    let controls = app_state::controls_of(PICO_VID, PICO_PID, &dev_hello(EncoderMode::Volume));
    let push = controls.iter().find(|c| c.id == 6).unwrap();
    assert_eq!(push.label, "Encoder push");
    assert!(push.handled_by_device);
    // An unknown model: no push switch is assumed, so 6 is a plain button.
    let other = app_state::controls_of(0x1209, 0x0002, &dev_hello(EncoderMode::Volume));
    assert_eq!(other.iter().find(|c| c.id == 6).unwrap().label, "Button 6");
}
