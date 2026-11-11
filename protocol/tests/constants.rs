//! The crate's constants must match `docs/protocol.md`. If one of these fails,
//! either the crate or the doc drifted; fix both in one commit with a version
//! bump (CLAUDE.md: protocol changes are atomic).

use detent_protocol::{ids, msg, HEADER_LEN, MAX_PAYLOAD_LEN, PROTOCOL_VERSION, REPORT_LEN};
use detent_protocol::{CONSUMER_REPORT_ID, USAGE, USAGE_PAGE, VENDOR_REPORT_ID};

#[test]
fn version_is_two() {
    assert_eq!(PROTOCOL_VERSION, 2);
}

#[test]
fn report_layout_adds_up() {
    // One HID interface shared with Consumer Control (D-036): 64 byte
    // endpoint, minus the report ID byte.
    assert_eq!(REPORT_LEN, 63);
    assert_eq!(HEADER_LEN, 4);
    assert_eq!(MAX_PAYLOAD_LEN, 59);
    assert_eq!(HEADER_LEN + MAX_PAYLOAD_LEN, REPORT_LEN);
}

#[test]
fn vendor_hid_usage() {
    assert_eq!(USAGE_PAGE, 0xFF00);
    assert_eq!(USAGE, 0x01);
}

#[test]
fn report_ids_match_the_arduino_core() {
    // Fixed by Arduino-ESP32's USBHID report ID enum; firmware asserts the same.
    assert_eq!(VENDOR_REPORT_ID, 6);
    assert_eq!(CONSUMER_REPORT_ID, 4);
}

#[test]
fn message_types_match_the_doc() {
    let expected = [
        (0x01, "HELLO"),
        (0x02, "BUTTON"),
        (0x03, "ENCODER"),
        (0x04, "MENU_SELECT"),
        (0x05, "HEARTBEAT"),
        (0x81, "HELLO_ACK"),
        (0x82, "MENU"),
        (0x83, "STATUS"),
        (0x84, "REQUEST_HELLO"),
        (0xF0, "TEST_INJECT"),
    ];
    assert_eq!(msg::ALL, expected);
    assert_eq!(msg::HELLO, 0x01);
    assert_eq!(msg::HEARTBEAT, 0x05);
    assert_eq!(msg::REQUEST_HELLO, 0x84);
    assert_eq!(msg::TEST_INJECT, 0xF0);
}

#[test]
fn message_types_are_unique() {
    for (i, (a, _)) in msg::ALL.iter().enumerate() {
        for (b, _) in &msg::ALL[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

#[test]
fn each_type_sits_in_its_direction_range() {
    let device = [
        msg::HELLO,
        msg::BUTTON,
        msg::ENCODER,
        msg::MENU_SELECT,
        msg::HEARTBEAT,
    ];
    let app = [
        msg::HELLO_ACK,
        msg::MENU,
        msg::STATUS,
        msg::REQUEST_HELLO,
        msg::TEST_INJECT,
    ];
    for t in device {
        assert!(msg::is_device_to_app(t), "{t:#04x}");
        assert!(!msg::is_app_to_device(t), "{t:#04x}");
    }
    for t in app {
        assert!(msg::is_app_to_device(t), "{t:#04x}");
        assert!(!msg::is_device_to_app(t), "{t:#04x}");
    }
    // 0x00 belongs to neither direction.
    assert!(!msg::is_device_to_app(0x00) && !msg::is_app_to_device(0x00));
}

#[test]
fn name_lookup() {
    assert_eq!(msg::name(msg::BUTTON), Some("BUTTON"));
    assert_eq!(msg::name(0x7F), None);
}

#[test]
fn vid_is_espressif() {
    assert_eq!(ids::VID, 0x303A);
}

#[test]
fn placeholder_pid_is_in_range_and_not_a_default() {
    assert!((0x8000..=0x8FFF).contains(&ids::PID));
    // TinyUSB and ESP-IDF defaults collide with stock firmware (architecture.md).
    for default in [0x0002, 0x1001, 0x4001, 0x4002, 0x4003, 0x4004] {
        assert_ne!(ids::PID, default);
    }
}
