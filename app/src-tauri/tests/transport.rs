//! Transport: framing over a raw HID link, tested through MockTransport, which
//! runs the same framing code as UsbHid. The last test is HIL (ignored by
//! default): `just hil-app` with the board plugged in.

use std::thread;
use std::time::{Duration, Instant};

use detent_lib::transport::{DeviceTransport, MockTransport, TransportError};
use detent_protocol::codec::{
    decode_app, decode_device, encode_app, encode_device, AppMessage, ButtonAction, DeviceMessage,
};
use detent_protocol::{CONSUMER_REPORT_ID, REPORT_LEN, VENDOR_REPORT_ID};

const SHORT: Duration = Duration::from_millis(30);

fn button(control: u8) -> DeviceMessage {
    DeviceMessage::Button {
        control,
        action: ButtonAction::Down,
    }
}

#[test]
fn vendor_reports_arrive_without_their_report_id() {
    let (mut t, hid) = MockTransport::new();
    hid.push_device(7, &button(2));
    let report = t.read(SHORT).unwrap().expect("a report");
    assert_eq!(report.len(), REPORT_LEN);
    let frame = decode_device(&report).unwrap();
    assert_eq!((frame.seq, frame.msg), (7, button(2)));
}

#[test]
fn writes_carry_report_id_6_and_63_bytes() {
    let (mut t, hid) = MockTransport::new();
    let report = encode_app(3, &AppMessage::RequestHello).unwrap();
    t.write(&report).unwrap();
    let writes = hid.writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].len(), 1 + REPORT_LEN);
    assert_eq!(writes[0][0], VENDOR_REPORT_ID);
    let frame = decode_app(&writes[0][1..]).unwrap();
    assert_eq!((frame.seq, frame.msg), (3, AppMessage::RequestHello));
}

#[test]
fn foreign_report_ids_are_dropped() {
    let (mut t, hid) = MockTransport::new();
    // Consumer Control (volume up) and an unknown ID, then a vendor report.
    hid.push_raw(vec![CONSUMER_REPORT_ID, 0xE9, 0x00]);
    hid.push_raw(vec![1; 1 + REPORT_LEN]);
    hid.push_device(0, &button(1));
    hid.push_raw(vec![CONSUMER_REPORT_ID, 0x00, 0x00]);
    let first = t.read(SHORT).unwrap().expect("the vendor report");
    assert_eq!(decode_device(&first).unwrap().msg, button(1));
    assert_eq!(t.read(SHORT).unwrap(), None, "only foreign reports remain");
}

#[test]
fn odd_length_vendor_reports_pass_through_for_the_codec_to_reject() {
    let (mut t, hid) = MockTransport::new();
    hid.push_raw(vec![VENDOR_REPORT_ID, 1, 2, 3]);
    let report = t.read(SHORT).unwrap().expect("passed through");
    assert_eq!(report, vec![1, 2, 3]);
    assert!(decode_device(&report).is_err());
}

#[test]
fn a_quiet_link_times_out_with_none() {
    let (mut t, _hid) = MockTransport::new();
    let started = Instant::now();
    assert_eq!(t.read(Duration::from_millis(50)).unwrap(), None);
    let waited = started.elapsed();
    assert!(waited >= Duration::from_millis(45), "{waited:?}");
    assert!(waited < Duration::from_millis(500), "{waited:?}");
}

#[test]
fn a_waiting_read_wakes_for_a_report_pushed_later() {
    let (mut t, hid) = MockTransport::new();
    let pusher = thread::spawn(move || {
        thread::sleep(Duration::from_millis(30));
        hid.push_device(1, &button(4));
        hid
    });
    let started = Instant::now();
    let report = t.read(Duration::from_secs(5)).unwrap().expect("pushed");
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(decode_device(&report).unwrap().msg, button(4));
    pusher.join().unwrap();
}

#[test]
fn unplug_surfaces_as_disconnected_and_stays_that_way() {
    let (mut t, hid) = MockTransport::new();
    hid.push_device(0, &button(1));
    assert!(t.read(SHORT).unwrap().is_some());
    hid.unplug();
    assert!(matches!(
        t.read(SHORT),
        Err(TransportError::Disconnected(_))
    ));
    assert!(matches!(
        t.read(SHORT),
        Err(TransportError::Disconnected(_))
    ));
    let report = encode_app(0, &AppMessage::RequestHello).unwrap();
    assert!(matches!(
        t.write(&report),
        Err(TransportError::Disconnected(_))
    ));
}

#[test]
fn unplug_wakes_a_waiting_read() {
    let (mut t, hid) = MockTransport::new();
    let unplugger = thread::spawn(move || {
        thread::sleep(Duration::from_millis(30));
        hid.unplug();
    });
    let started = Instant::now();
    assert!(matches!(
        t.read(Duration::from_secs(5)),
        Err(TransportError::Disconnected(_))
    ));
    assert!(started.elapsed() < Duration::from_secs(1));
    unplugger.join().unwrap();
}

#[test]
fn a_short_write_is_an_error() {
    let (mut t, hid) = MockTransport::new();
    hid.limit_writes_to(10);
    let report = encode_app(0, &AppMessage::RequestHello).unwrap();
    assert_eq!(
        t.write(&report),
        Err(TransportError::ShortWrite {
            wrote: 10,
            expected: 1 + REPORT_LEN
        })
    );
}

#[test]
fn transports_move_to_another_thread() {
    fn assert_send<T: Send>() {}
    assert_send::<MockTransport>();
    let (t, hid) = MockTransport::new();
    let mut boxed: Box<dyn DeviceTransport> = Box::new(t);
    hid.push_device(0, &encode_check());
    assert!(boxed.read(SHORT).unwrap().is_some());
}

fn encode_check() -> DeviceMessage {
    let msg = button(9);
    encode_device(0, &msg).expect("encodes");
    msg
}

/// HIL: UsbHid opens the real board's vendor channel (non exclusive), asks
/// for a HELLO, and reads one. Needs the board plugged in, any firmware.
#[cfg(target_os = "macos")]
#[test]
#[ignore = "needs the board: just hil-app"]
fn usb_hid_reads_a_hello_from_the_board() {
    use detent_lib::transport::Hid;
    let mut hid = Hid::new().expect("hidapi starts");
    let channels = hid.channels().expect("enumerates");
    let ch = channels
        .iter()
        .find(|c| c.vid == detent_protocol::ids::VID && c.pid == detent_protocol::ids::PID)
        .unwrap_or_else(|| panic!("no Detent vendor channel among {channels:?}"));
    println!("opening {ch:?}");
    let mut t = hid.open(ch).expect("opens");
    t.write(&encode_app(0, &AppMessage::RequestHello).unwrap())
        .expect("REQUEST_HELLO sent");
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if let Some(report) = t.read(Duration::from_millis(200)).expect("reads") {
            if let Ok(frame) = decode_device(&report) {
                if let DeviceMessage::Hello(h) = frame.msg {
                    println!("HELLO: {h:?}");
                    return;
                }
            }
        }
    }
    panic!("no HELLO within 3 s");
}
