//! Devices: attach, pairing handshake, events, heartbeat, detach, and the
//! allowlist, against MockBus and MockTransport. Time is passed in, so the
//! heartbeat timeout is tested exactly and instantly.

use std::time::{Duration, Instant};

use detent_lib::devices::{DeviceEvent, DeviceId, Devices, Lost, Options};
use detent_lib::store::{DeviceEntry, UnitRef};
use detent_lib::transport::{MockBus, MockHid};
use detent_protocol::codec::{
    decode_app, AppMessage, ButtonAction, Control, ControlKind, DeviceMessage, EncoderMode,
    FirmwareVersion, Hello,
};
use detent_protocol::ids::{PID, VID};
use detent_protocol::VENDOR_REPORT_ID;

const PAIRED: &str = "E072A1FC4EDC";
const OTHER: &str = "AABBCCDDEEFF";
const SECOND: &str = "112233445566";

fn opts() -> Options {
    Options {
        read_wait: Duration::from_millis(1),
        ..Options::default()
    }
}

fn entry(serial: &str) -> DeviceEntry {
    DeviceEntry {
        name: format!("unit {serial}"),
        vid: VID,
        pid: PID,
        serial: serial.into(),
    }
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
            patch: 0,
        },
        encoder_mode: EncoderMode::Mapped,
        controls: vec![
            Control {
                id: 1,
                kind: ControlKind::Button,
            },
            Control {
                id: 5,
                kind: ControlKind::Encoder,
            },
        ],
    }
}

fn press(control: u8) -> DeviceMessage {
    DeviceMessage::Button {
        control,
        action: ButtonAction::Down,
    }
}

fn heartbeat(uptime_s: u32) -> DeviceMessage {
    DeviceMessage::Heartbeat {
        uptime_s,
        encoder_mode: EncoderMode::Mapped,
    }
}

/// App messages the device received, decoded.
fn sent(hid: &MockHid) -> Vec<AppMessage> {
    hid.writes()
        .iter()
        .map(|w| {
            assert_eq!(w[0], VENDOR_REPORT_ID);
            decode_app(&w[1..]).expect("app sent a valid report").msg
        })
        .collect()
}

struct Rig {
    bus: MockBus,
    devices: Devices<MockBus>,
    t0: Instant,
}

impl Rig {
    fn new(allow: &[&str], ignore: &[&str]) -> Self {
        let bus = MockBus::default();
        let allow: Vec<_> = allow.iter().map(|s| entry(s)).collect();
        let ignore: Vec<_> = ignore
            .iter()
            .map(|s| UnitRef {
                vid: VID,
                pid: PID,
                serial: (*s).into(),
            })
            .collect();
        let devices = Devices::new(bus.clone(), &allow, &ignore, opts());
        Self {
            bus,
            devices,
            t0: Instant::now(),
        }
    }

    fn at(&mut self, secs: f64) -> Vec<DeviceEvent> {
        self.devices.step(self.t0 + Duration::from_secs_f64(secs))
    }
}

#[test]
fn an_allowlisted_unit_opens_requests_hello_and_is_acked() {
    let mut r = Rig::new(&[PAIRED], &[]);
    let hid = r.bus.attach(VID, PID, Some(PAIRED));
    assert_eq!(r.at(0.0), vec![DeviceEvent::Connected(id(PAIRED))]);
    assert_eq!(sent(&hid), vec![AppMessage::RequestHello]);

    hid.push_device(0, &DeviceMessage::Hello(hello()));
    assert_eq!(
        r.at(0.1),
        vec![DeviceEvent::Hello {
            id: id(PAIRED),
            hello: hello()
        }]
    );
    assert_eq!(
        sent(&hid),
        vec![
            AppMessage::RequestHello,
            AppMessage::HelloAck { paired: true }
        ]
    );
    assert_eq!(r.devices.connected(), vec![&id(PAIRED)]);
    assert_eq!(r.devices.hello(&id(PAIRED)), Some(&hello()));
}

#[test]
fn input_is_tagged_with_the_device_and_heartbeats_stay_internal() {
    let mut r = Rig::new(&[PAIRED], &[]);
    let hid = r.bus.attach(VID, PID, Some(PAIRED));
    r.at(0.0);
    hid.push_device(0, &DeviceMessage::Hello(hello()));
    hid.push_device(1, &press(1));
    hid.push_device(2, &heartbeat(5));
    hid.push_device(
        3,
        &DeviceMessage::Encoder {
            control: 5,
            delta: -2,
        },
    );
    let events = r.at(0.1);
    assert_eq!(
        events,
        vec![
            DeviceEvent::Hello {
                id: id(PAIRED),
                hello: hello()
            },
            DeviceEvent::Input {
                id: id(PAIRED),
                message: press(1)
            },
            DeviceEvent::Input {
                id: id(PAIRED),
                message: DeviceMessage::Encoder {
                    control: 5,
                    delta: -2
                }
            },
        ]
    );
}

#[test]
fn a_second_hello_is_acked_again_and_reported() {
    // The device sends HELLO again after leaving Setup.
    let mut r = Rig::new(&[PAIRED], &[]);
    let hid = r.bus.attach(VID, PID, Some(PAIRED));
    r.at(0.0);
    hid.push_device(0, &DeviceMessage::Hello(hello()));
    r.at(0.1);
    let mut h2 = hello();
    h2.encoder_mode = EncoderMode::Volume;
    hid.push_device(1, &DeviceMessage::Hello(h2.clone()));
    assert_eq!(
        r.at(0.2),
        vec![DeviceEvent::Hello {
            id: id(PAIRED),
            hello: h2.clone()
        }]
    );
    assert_eq!(r.devices.hello(&id(PAIRED)), Some(&h2));
    let acks = sent(&hid)
        .into_iter()
        .filter(|m| *m == AppMessage::HelloAck { paired: true })
        .count();
    assert_eq!(acks, 2);
}

#[test]
fn garbage_is_rejected_and_the_unit_stays_up() {
    let mut r = Rig::new(&[PAIRED], &[]);
    let hid = r.bus.attach(VID, PID, Some(PAIRED));
    r.at(0.0);
    let mut bad = vec![VENDOR_REPORT_ID, 9]; // version 9
    bad.resize(64, 0);
    hid.push_raw(bad);
    hid.push_raw(vec![VENDOR_REPORT_ID, 1, 2]); // short
    hid.push_device(0, &press(1));
    let events = r.at(0.1);
    assert_eq!(events.len(), 3, "{events:?}");
    assert!(matches!(&events[0], DeviceEvent::Rejected { id: i, .. } if *i == id(PAIRED)));
    assert!(matches!(&events[1], DeviceEvent::Rejected { .. }));
    assert_eq!(
        events[2],
        DeviceEvent::Input {
            id: id(PAIRED),
            message: press(1)
        }
    );
    assert_eq!(r.devices.connected(), vec![&id(PAIRED)]);
}

#[test]
fn heartbeats_keep_a_unit_up_and_6_s_of_silence_drops_it() {
    let mut r = Rig::new(&[PAIRED], &[]);
    let hid = r.bus.attach(VID, PID, Some(PAIRED));
    r.at(0.0);
    // Heartbeats every 2 s through t = 10 s.
    for (i, t) in [2.0, 4.0, 6.0, 8.0, 10.0].into_iter().enumerate() {
        hid.push_device(i as u8, &heartbeat(t as u32));
        assert_eq!(r.at(t), vec![], "t = {t}");
    }
    // Silent since t = 10: still up just under 6 s later, down at 6 s.
    assert_eq!(r.at(15.9), vec![]);
    assert_eq!(
        r.at(16.0),
        vec![DeviceEvent::Disconnected {
            id: id(PAIRED),
            why: Lost::HeartbeatTimeout
        }]
    );
    assert!(r.devices.connected().is_empty());
}

#[test]
fn garbage_does_not_count_as_a_heartbeat() {
    let mut r = Rig::new(&[PAIRED], &[]);
    let hid = r.bus.attach(VID, PID, Some(PAIRED));
    r.at(0.0);
    let mut bad = vec![VENDOR_REPORT_ID, 9];
    bad.resize(64, 0);
    hid.push_raw(bad);
    r.at(5.0);
    let events = r.at(6.0);
    assert!(events.contains(&DeviceEvent::Disconnected {
        id: id(PAIRED),
        why: Lost::HeartbeatTimeout
    }));
}

#[test]
fn a_unit_dropped_for_silence_is_reopened_by_the_next_scan() {
    let mut r = Rig::new(&[PAIRED], &[]);
    let hid = r.bus.attach(VID, PID, Some(PAIRED));
    r.at(0.0);
    r.at(6.0); // dropped
    assert_eq!(r.at(7.0), vec![DeviceEvent::Connected(id(PAIRED))]);
    assert_eq!(
        sent(&hid),
        vec![AppMessage::RequestHello, AppMessage::RequestHello]
    );
}

#[test]
fn unplug_disconnects_and_replug_reconnects() {
    let mut r = Rig::new(&[PAIRED], &[]);
    let hid = r.bus.attach(VID, PID, Some(PAIRED));
    r.at(0.0);
    hid.push_device(0, &DeviceMessage::Hello(hello()));
    r.at(0.1);

    r.bus.detach(PAIRED);
    let events = r.at(0.2);
    assert!(
        matches!(
            events.as_slice(),
            [DeviceEvent::Disconnected { id: i, why: Lost::Link(_) }] if *i == id(PAIRED)
        ),
        "{events:?}"
    );
    assert_eq!(r.at(1.5), vec![], "nothing attached, nothing to open");

    let hid2 = r.bus.attach(VID, PID, Some(PAIRED));
    assert_eq!(r.at(2.5), vec![DeviceEvent::Connected(id(PAIRED))]);
    assert_eq!(sent(&hid2), vec![AppMessage::RequestHello]);
    hid2.push_device(0, &press(1));
    assert_eq!(
        r.at(2.6),
        vec![DeviceEvent::Input {
            id: id(PAIRED),
            message: press(1)
        }]
    );
}

#[test]
fn a_unit_gone_from_the_list_is_closed_even_if_reads_never_failed() {
    let mut r = Rig::new(&[PAIRED], &[]);
    r.bus.attach(VID, PID, Some(PAIRED));
    r.at(0.0);
    r.bus.remove_quietly(PAIRED);
    assert_eq!(
        r.at(1.0),
        vec![DeviceEvent::Disconnected {
            id: id(PAIRED),
            why: Lost::Detached
        }]
    );
}

#[test]
fn the_list_is_polled_once_a_second() {
    let mut r = Rig::new(&[PAIRED], &[]);
    r.at(0.0);
    let hid = r.bus.attach(VID, PID, Some(PAIRED));
    assert_eq!(r.at(0.5), vec![], "not scanned yet");
    assert_eq!(r.at(1.0), vec![DeviceEvent::Connected(id(PAIRED))]);
    assert_eq!(sent(&hid), vec![AppMessage::RequestHello]);
}

#[test]
fn an_unknown_unit_is_reported_once_per_attach_and_never_opened() {
    let mut r = Rig::new(&[PAIRED], &[]);
    let hid = r.bus.attach(VID, PID, Some(OTHER));
    hid.push_device(0, &press(1));
    let unknown = DeviceEvent::Unknown {
        vid: VID,
        pid: PID,
        serial: Some(OTHER.into()),
    };
    assert_eq!(r.at(0.0), vec![unknown.clone()]);
    assert_eq!(r.at(1.0), vec![]);
    assert_eq!(r.at(2.0), vec![]);
    assert_eq!(r.bus.opens(), 0, "never opened");
    assert!(hid.writes().is_empty(), "never written");

    r.bus.detach(OTHER);
    r.at(3.0);
    r.bus.attach(VID, PID, Some(OTHER));
    assert_eq!(r.at(4.0), vec![unknown], "a new attach is news again");
    assert_eq!(r.bus.opens(), 0);
}

#[test]
fn an_ignored_unit_stays_ignored() {
    let mut r = Rig::new(&[PAIRED], &[OTHER]);
    let hid = r.bus.attach(VID, PID, Some(OTHER));
    hid.push_device(0, &press(1));
    for t in 0..4 {
        assert_eq!(r.at(t as f64), vec![]);
    }
    assert_eq!(r.bus.opens(), 0);
}

#[test]
fn ignore_wins_over_the_allowlist() {
    let mut r = Rig::new(&[PAIRED], &[PAIRED]);
    r.bus.attach(VID, PID, Some(PAIRED));
    assert_eq!(r.at(0.0), vec![]);
    assert_eq!(r.bus.opens(), 0);
}

#[test]
fn other_vendors_and_serialless_entries_are_not_news() {
    let mut r = Rig::new(&[PAIRED], &[]);
    // Some other product's vendor page collection, and a Detent with no serial.
    r.bus.attach(0x046D, 0xC52B, Some("X"));
    r.bus.attach(VID, PID, None);
    assert_eq!(
        r.at(0.0),
        vec![DeviceEvent::Unknown {
            vid: VID,
            pid: PID,
            serial: None
        }]
    );
    assert_eq!(r.bus.opens(), 0);
}

#[test]
fn serials_match_case_insensitively() {
    let mut r = Rig::new(&[&PAIRED.to_ascii_lowercase()], &[]);
    r.bus.attach(VID, PID, Some(PAIRED));
    assert_eq!(r.at(0.0), vec![DeviceEvent::Connected(id(PAIRED))]);
}

#[test]
fn a_different_pid_with_the_same_serial_is_not_the_paired_unit() {
    let mut r = Rig::new(&[PAIRED], &[]);
    r.bus.attach(VID, PID - 1, Some(PAIRED));
    r.at(0.0);
    assert_eq!(r.bus.opens(), 0);
}

#[test]
fn two_paired_units_run_side_by_side() {
    let mut r = Rig::new(&[PAIRED, SECOND], &[]);
    let a = r.bus.attach(VID, PID, Some(PAIRED));
    let b = r.bus.attach(VID, PID, Some(SECOND));
    let mut connected = r.at(0.0);
    connected.sort_by_key(|e| format!("{e:?}"));
    assert_eq!(connected.len(), 2);
    a.push_device(0, &press(1));
    b.push_device(0, &press(2));
    let events = r.at(0.1);
    assert!(events.contains(&DeviceEvent::Input {
        id: id(PAIRED),
        message: press(1)
    }));
    assert!(events.contains(&DeviceEvent::Input {
        id: id(SECOND),
        message: press(2)
    }));
}

#[test]
fn a_failed_open_is_reported_once_and_retried() {
    let mut r = Rig::new(&[PAIRED], &[]);
    r.bus.attach(VID, PID, Some(PAIRED));
    r.bus.fail_opens(true);
    let first = r.at(0.0);
    assert!(
        matches!(first.as_slice(), [DeviceEvent::OpenFailed { id: i, .. }] if *i == id(PAIRED)),
        "{first:?}"
    );
    assert_eq!(r.at(1.0), vec![], "reported once");
    r.bus.fail_opens(false);
    assert_eq!(r.at(2.0), vec![DeviceEvent::Connected(id(PAIRED))]);
}

#[test]
fn a_failed_write_drops_the_unit() {
    let mut r = Rig::new(&[PAIRED], &[]);
    let hid = r.bus.attach(VID, PID, Some(PAIRED));
    r.at(0.0);
    hid.limit_writes_to(3);
    hid.push_device(0, &DeviceMessage::Hello(hello()));
    let events = r.at(0.1);
    assert!(
        matches!(
            events.last(),
            Some(DeviceEvent::Disconnected {
                why: Lost::Link(_),
                ..
            })
        ),
        "{events:?}"
    );
}

/// HIL: the devices module over hidapi with the real board allowlisted. It
/// connects, gets a HELLO (and acks it), and stays connected on heartbeats.
/// Needs the board plugged in, any firmware: `just hil-app`.
#[cfg(target_os = "macos")]
#[test]
#[ignore = "needs the board: just hil-app"]
fn the_board_pairs_and_stays_up_on_heartbeats() {
    use detent_lib::transport::{Bus, Hid};
    let mut hid = Hid::new().expect("hidapi starts");
    let serial = hid
        .scan()
        .expect("enumerates")
        .into_iter()
        .find(|c| c.vid == VID && c.pid == PID)
        .and_then(|c| c.serial)
        .expect("a Detent board with a serial is attached");
    let mut devices = Devices::new(hid, &[entry(&serial)], &[], Options::default());
    let start = Instant::now();
    let mut log = Vec::new();
    while start.elapsed() < Duration::from_secs(7) {
        for e in devices.step(Instant::now()) {
            println!("{:>6.3}s  {e:?}", start.elapsed().as_secs_f64());
            log.push(e);
        }
    }
    assert_eq!(log.first(), Some(&DeviceEvent::Connected(id(&serial))));
    assert!(
        log.iter().any(|e| matches!(e, DeviceEvent::Hello { .. })),
        "no HELLO"
    );
    assert!(
        !log.iter()
            .any(|e| matches!(e, DeviceEvent::Disconnected { .. })),
        "dropped while heartbeats should have kept it up"
    );
    assert_eq!(devices.connected(), vec![&id(&serial)]);
}

#[test]
fn after_sleep_every_unit_is_closed_and_reopened_at_once() {
    let mut r = Rig::new(&[PAIRED, SECOND], &[]);
    let a = r.bus.attach(VID, PID, Some(PAIRED));
    r.bus.attach(VID, PID, Some(SECOND));
    r.at(0.0);
    a.push_device(0, &DeviceMessage::Hello(hello()));
    r.at(0.1);
    assert_eq!(r.bus.opens(), 2);

    // Woke at t = 0.3, well before the next scheduled scan at t = 1.
    r.devices.woke();
    let events = r.at(0.3);
    let lost: Vec<_> = events
        .iter()
        .filter(|e| {
            matches!(
                e,
                DeviceEvent::Disconnected {
                    why: Lost::Slept,
                    ..
                }
            )
        })
        .collect();
    let back: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, DeviceEvent::Connected(_)))
        .collect();
    assert_eq!((lost.len(), back.len()), (2, 2), "{events:?}");
    assert_eq!(r.bus.opens(), 4, "both reopened in the same step");
    assert_eq!(r.devices.hello(&id(PAIRED)), None, "HELLO asked for again");
    assert_eq!(
        sent(&a).last(),
        Some(&AppMessage::RequestHello),
        "the reopened unit is asked for its HELLO"
    );
}

#[test]
fn waking_with_nothing_open_just_scans() {
    let mut r = Rig::new(&[PAIRED], &[]);
    r.at(0.0);
    r.bus.attach(VID, PID, Some(PAIRED));
    r.devices.woke();
    assert_eq!(r.at(0.2), vec![DeviceEvent::Connected(id(PAIRED))]);
}
