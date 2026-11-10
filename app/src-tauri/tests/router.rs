//! Router: one event plus the frontmost app resolves to at most one mapping.

use detent_lib::router::{trigger_of, Router};
use detent_lib::store::{parse, Action, Trigger};
use detent_protocol::codec::{ButtonAction, DeviceMessage, EncoderMode, FirmwareVersion, Hello};

const SERIAL: &str = "E072A1FC4EDC";

/// A config with the given mappings (as JSON array items) on one device.
fn router(mappings: &str) -> Router {
    let json = format!(
        r#"{{ "version": 1,
              "devices": [{{ "name": "Desk", "vid": "0x303A", "pid": "0x8FDE", "serial": "{SERIAL}" }},
                          {{ "name": "Other", "vid": "0x303A", "pid": "0x8FDE", "serial": "OTHER" }}],
              "mappings": [{mappings}] }}"#
    );
    Router::new(&parse(&json).expect("test config is valid"))
}

fn m(control: u8, trigger: &str, scope: &str, action: &str) -> String {
    format!(
        r#"{{ "device": "{SERIAL}", "control": {control}, "trigger": "{trigger}", "scope": {scope}, "action": {action} }}"#
    )
}

const EVERYWHERE: &str = r#""everywhere""#;
const SAFARI: &str = r#"{ "app": "com.apple.Safari" }"#;
const MAIL: &str = r#"{ "type": "open_app", "bundle_id": "com.apple.mail" }"#;
const MUTE: &str = r#"{ "type": "mute_toggle" }"#;
const UP: &str = r#"{ "type": "volume_up" }"#;

fn open_mail() -> Action {
    Action::OpenApp {
        bundle_id: "com.apple.mail".into(),
    }
}

#[test]
fn app_scope_beats_everywhere_and_everywhere_covers_other_apps() {
    // acceptance.md scenario: "App scope beats Everywhere; Everywhere used with no app match"
    let r = router(&[m(1, "press", EVERYWHERE, MAIL), m(1, "press", SAFARI, MUTE)].join(","));
    let got = |front| {
        r.resolve(SERIAL, 1, Trigger::Press, front)
            .map(|m| m.action.clone())
    };
    assert_eq!(got(Some("com.apple.Safari")), Some(Action::MuteToggle));
    assert_eq!(got(Some("com.apple.finder")), Some(open_mail()));
    assert_eq!(got(None), Some(open_mail()));
}

/// (serial, control, trigger, frontmost app, expected action)
type Case<'a> = (&'a str, u8, Trigger, Option<&'a str>, Option<Action>);

#[test]
fn resolution_table() {
    let r = router(
        &[
            m(1, "press", EVERYWHERE, MAIL),
            m(2, "press", SAFARI, MUTE),
            m(2, "long_press", EVERYWHERE, UP),
            m(5, "clockwise", EVERYWHERE, UP),
        ]
        .join(","),
    );
    let cases: &[Case] = &[
        (
            SERIAL,
            1,
            Trigger::Press,
            Some("com.apple.Safari"),
            Some(open_mail()),
        ), // no app mapping: everywhere
        (SERIAL, 1, Trigger::LongPress, None, None), // trigger not mapped
        (SERIAL, 1, Trigger::Clockwise, None, None),
        (
            SERIAL,
            2,
            Trigger::Press,
            Some("com.apple.Safari"),
            Some(Action::MuteToggle),
        ),
        (SERIAL, 2, Trigger::Press, Some("com.apple.finder"), None), // app only, wrong app
        (SERIAL, 2, Trigger::Press, None, None),                     // app only, no frontmost
        (
            SERIAL,
            2,
            Trigger::LongPress,
            Some("com.apple.Safari"),
            Some(Action::VolumeUp { step: 1.0 / 16.0 }),
        ),
        (
            SERIAL,
            5,
            Trigger::Clockwise,
            None,
            Some(Action::VolumeUp { step: 1.0 / 16.0 }),
        ),
        (SERIAL, 5, Trigger::Counterclockwise, None, None),
        (SERIAL, 9, Trigger::Press, None, None), // unmapped control
        ("OTHER", 1, Trigger::Press, None, None), // mappings belong to their device
        ("NOPE", 1, Trigger::Press, None, None), // unknown device
    ];
    for (i, (serial, control, trigger, front, expected)) in cases.iter().enumerate() {
        let got = r
            .resolve(serial, *control, *trigger, *front)
            .map(|m| m.action.clone());
        assert_eq!(
            &got, expected,
            "case {i}: {serial} {control} {trigger:?} in {front:?}"
        );
    }
}

#[test]
fn bundle_ids_match_case_insensitively() {
    let r = router(&m(1, "press", r#"{ "app": "com.apple.safari" }"#, MUTE));
    assert_eq!(
        r.resolve(SERIAL, 1, Trigger::Press, Some("com.apple.Safari"))
            .map(|m| &m.action),
        Some(&Action::MuteToggle)
    );
}

#[test]
fn different_apps_each_get_their_own_mapping() {
    let r = router(
        &[
            m(3, "press", SAFARI, MUTE),
            m(3, "press", r#"{ "app": "com.plasticity.app" }"#, UP),
        ]
        .join(","),
    );
    let got = |front| {
        r.resolve(SERIAL, 3, Trigger::Press, Some(front))
            .map(|m| m.action.clone())
    };
    assert_eq!(got("com.apple.Safari"), Some(Action::MuteToggle));
    assert_eq!(
        got("com.plasticity.app"),
        Some(Action::VolumeUp { step: 1.0 / 16.0 })
    );
}

#[test]
fn example_config_routes_as_documented() {
    let config = parse(include_str!("../examples/config.example.json")).unwrap();
    let r = Router::new(&config);
    assert_eq!(
        r.resolve(SERIAL, 1, Trigger::Press, Some("com.apple.Safari"))
            .map(|m| &m.action),
        Some(&Action::MuteToggle)
    );
    assert_eq!(
        r.resolve(SERIAL, 1, Trigger::Press, Some("com.apple.finder"))
            .map(|m| &m.action),
        Some(&open_mail())
    );
}

#[test]
fn an_empty_config_routes_nothing() {
    let r = Router::new(&parse(r#"{ "version": 1 }"#).unwrap());
    assert!(r.resolve(SERIAL, 1, Trigger::Press, None).is_none());
}

// --- protocol messages to triggers (D-054) ---

#[test]
fn buttons_trigger_on_press_and_long_press_never_release() {
    let b = |action| DeviceMessage::Button { control: 4, action };
    assert_eq!(
        trigger_of(&b(ButtonAction::Down)),
        Some((4, Trigger::Press, 1))
    );
    assert_eq!(
        trigger_of(&b(ButtonAction::LongPress)),
        Some((4, Trigger::LongPress, 1))
    );
    assert_eq!(trigger_of(&b(ButtonAction::Up)), None);
}

#[test]
fn encoder_turns_run_once_per_detent() {
    let e = |delta| DeviceMessage::Encoder { control: 5, delta };
    assert_eq!(trigger_of(&e(1)), Some((5, Trigger::Clockwise, 1)));
    assert_eq!(trigger_of(&e(3)), Some((5, Trigger::Clockwise, 3)));
    assert_eq!(trigger_of(&e(-2)), Some((5, Trigger::Counterclockwise, 2)));
    assert_eq!(
        trigger_of(&e(-128)),
        Some((5, Trigger::Counterclockwise, 128))
    );
}

#[test]
fn other_messages_trigger_nothing() {
    let hello = DeviceMessage::Hello(Hello {
        firmware: FirmwareVersion {
            major: 0,
            minor: 1,
            patch: 0,
        },
        encoder_mode: EncoderMode::Volume,
        controls: vec![],
    });
    assert_eq!(trigger_of(&hello), None);
    assert_eq!(
        trigger_of(&DeviceMessage::Heartbeat {
            uptime_s: 1,
            encoder_mode: EncoderMode::Volume
        }),
        None
    );
    assert_eq!(trigger_of(&DeviceMessage::MenuSelect { item: 1 }), None);
}
