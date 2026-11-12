//! Protocol v3 is frozen (docs/protocol.md). These are the exact bytes of every
//! sample message, plus the transport constants, the message type table, and
//! each reject verdict. v1 was frozen on 2026-11-07; v2 (2026-11-11, D-094)
//! changed byte 0 of every report to 2 and added HELLO kind 3, an encoder
//! entry carrying its push switch (sample HELLO_PUSH). Every other v1 byte is
//! unchanged. v3 (2026-11-12, D-112) changed byte 0 to 3 and nothing else on
//! the wire: what changed is how a host recognizes a unit (Identity).
//!
//! If this test fails, the wire format changed. That needs a PROTOCOL_VERSION
//! bump and a new section here, not an edit to these bytes (CLAUDE.md:
//! protocol changes are atomic).

use detent_protocol::codec::{decode_app, encode_app, encode_device};
use detent_protocol::vectors::{error_kind, pack_cases, parse_cases, reject_cases};
use detent_protocol::{
    msg, CONSUMER_REPORT_ID, HEADER_LEN, MAX_PAYLOAD_LEN, PROTOCOL_VERSION, REPORT_LEN, USAGE,
    USAGE_PAGE, VENDOR_REPORT_ID,
};

/// The leading bytes of a report; everything after is zero.
fn report(prefix: &[u8]) -> [u8; REPORT_LEN] {
    let mut r = [0u8; REPORT_LEN];
    r[..prefix.len()].copy_from_slice(prefix);
    r
}

const V3_DEVICE_TO_APP: &[(&str, &[u8])] = &[
    (
        "HELLO",
        &[
            0x03, 0x01, 0x00, 0x0B, 0x00, 0x01, 0x02, 0x02, 0x03, 0x01, 0x01, 0x02, 0x01, 0x09,
            0x02,
        ],
    ),
    ("HELLO_EMPTY", &[0x03, 0x01, 0xFF, 0x05, 0x01]),
    (
        "HELLO_MAX",
        &[
            0x03, 0x01, 0x01, 0x3B, 0x02, 0x03, 0x04, 0x01, 0x1B, 0x00, 0x01, 0x01, 0x02, 0x02,
            0x01, 0x03, 0x02, 0x04, 0x01, 0x05, 0x02, 0x06, 0x01, 0x07, 0x02, 0x08, 0x01, 0x09,
            0x02, 0x0A, 0x01, 0x0B, 0x02, 0x0C, 0x01, 0x0D, 0x02, 0x0E, 0x01, 0x0F, 0x02, 0x10,
            0x01, 0x11, 0x02, 0x12, 0x01, 0x13, 0x02, 0x14, 0x01, 0x15, 0x02, 0x16, 0x01, 0x17,
            0x02, 0x18, 0x01, 0x19, 0x02, 0x1A, 0x01,
        ],
    ),
    (
        "HELLO_PUSH",
        &[
            0x03, 0x01, 0x02, 0x0E, 0x00, 0x02, 0x00, 0x00, 0x04, 0x01, 0x01, 0x05, 0x03, 0x06,
            0x06, 0x01, 0x07, 0x02,
        ],
    ),
    ("BUTTON_LONG_PRESS", &[0x03, 0x02, 0x09, 0x02, 0x04, 0x03]),
    ("ENCODER_NEGATIVE", &[0x03, 0x03, 0x0A, 0x02, 0x09, 0xFD]),
    ("MENU_SELECT", &[0x03, 0x04, 0x0B, 0x01, 0x2A]),
    (
        "HEARTBEAT",
        &[0x03, 0x05, 0x0C, 0x05, 0x04, 0x03, 0x02, 0x01, 0x01],
    ),
];

const V3_APP_TO_DEVICE: &[(&str, &[u8])] = &[
    ("HELLO_ACK", &[0x03, 0x81, 0x01, 0x01, 0x01]),
    (
        "MENU_ITEM",
        &[
            0x03, 0x82, 0x02, 0x14, 0x03, 0x01, 0x2A, 0x04, 0x4D, 0x61, 0x69, 0x6C,
        ],
    ),
    ("MENU_CLEAR", &[0x03, 0x82, 0x03, 0x14]),
    (
        "STATUS",
        &[
            0x03, 0x83, 0x04, 0x15, 0x08, 0x48, 0x6F, 0x6D, 0x65, 0x70, 0x61, 0x67, 0x65,
        ],
    ),
    ("STATUS_EMPTY", &[0x03, 0x83, 0x05, 0x15]),
    ("REQUEST_HELLO", &[0x03, 0x84, 0x06]),
    (
        "TEST_INJECT",
        &[
            0x03, 0xF0, 0x07, 0x0B, 0x02, 0x02, 0x04, 0x01, 0x10, 0x00, 0x03, 0x09, 0xFD, 0x00,
            0x01,
        ],
    ),
];

const V3_REJECTS: &[(&str, &str)] = &[
    ("bad_version", "Version"),
    ("len_over_max", "PayloadLength"),
    ("trailing_byte", "TrailingBytes"),
    ("unknown_type", "UnknownType"),
    ("device_to_app_type", "UnknownType"),
    ("hello_ack_empty", "PayloadSize"),
    ("hello_ack_bad_value", "InvalidField"),
    ("request_hello_payload", "PayloadSize"),
    ("menu_short", "PayloadSize"),
    ("menu_index_ge_total", "InvalidField"),
    ("menu_label_empty", "InvalidField"),
    ("menu_label_len_17", "InvalidField"),
    ("menu_label_control_char", "InvalidField"),
    ("menu_label_padding", "TrailingBytes"),
    ("menu_clear_with_item", "InvalidField"),
    ("menu_clear_with_index", "InvalidField"),
    ("status_len_21", "InvalidField"),
    ("status_non_ascii", "InvalidField"),
    ("status_padding", "TrailingBytes"),
    ("inject_empty", "PayloadSize"),
    ("inject_count_zero", "InvalidField"),
    ("inject_count_12", "TooManyEvents"),
    ("inject_size_mismatch", "PayloadSize"),
    ("inject_bad_event_type", "InvalidField"),
    ("inject_bad_action", "InvalidField"),
    ("inject_zero_delta", "InvalidField"),
];

#[test]
fn transport_constants_are_v3() {
    assert_eq!(
        PROTOCOL_VERSION, 3,
        "a new version gets its own frozen section here"
    );
    assert_eq!(REPORT_LEN, 63);
    assert_eq!(HEADER_LEN, 4);
    assert_eq!(MAX_PAYLOAD_LEN, 59);
    assert_eq!(USAGE_PAGE, 0xFF00);
    assert_eq!(USAGE, 0x01);
    assert_eq!(VENDOR_REPORT_ID, 6);
    assert_eq!(CONSUMER_REPORT_ID, 4);
}

#[test]
fn message_type_table_is_v3() {
    let v3: [(u8, &str); 10] = [
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
    assert_eq!(msg::ALL, v3);
}

#[test]
fn device_to_app_bytes_are_v3() {
    let cases = pack_cases();
    assert_eq!(cases.len(), V3_DEVICE_TO_APP.len(), "sample set changed");
    for ((name, m, seq), (frozen_name, bytes)) in cases.iter().zip(V3_DEVICE_TO_APP) {
        assert_eq!(name, frozen_name);
        let got = encode_device(*seq, m).expect("sample encodes");
        assert_eq!(
            got,
            report(bytes),
            "{name}: v3 bytes changed; bump PROTOCOL_VERSION"
        );
    }
}

#[test]
fn app_to_device_bytes_are_v3() {
    let cases = parse_cases();
    assert_eq!(cases.len(), V3_APP_TO_DEVICE.len(), "sample set changed");
    for ((name, m, seq), (frozen_name, bytes)) in cases.iter().zip(V3_APP_TO_DEVICE) {
        assert_eq!(name, frozen_name);
        let got = encode_app(*seq, m).expect("sample encodes");
        assert_eq!(
            got,
            report(bytes),
            "{name}: v3 bytes changed; bump PROTOCOL_VERSION"
        );
    }
}

#[test]
fn reject_verdicts_are_v3() {
    let cases = reject_cases();
    assert_eq!(cases.len(), V3_REJECTS.len(), "reject set changed");
    for ((name, r), (frozen_name, kind)) in cases.iter().zip(V3_REJECTS) {
        assert_eq!(name, frozen_name);
        let err = decode_app(r).expect_err("v3 rejects this");
        assert_eq!(
            error_kind(&err),
            *kind,
            "{name}: v3 verdict changed; bump PROTOCOL_VERSION"
        );
    }
}
