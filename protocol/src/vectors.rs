//! Generates `firmware/test/vectors.h`: golden reports from this crate's
//! codec, so the firmware's C++ codec is tested byte for byte against Rust
//! (D-035). Run `scripts/gen-header.sh`; a test fails if the file drifts.
//!
//! Three kinds of vector:
//! - pack: device to app messages the firmware must build exactly;
//! - parse: app to device messages the firmware must decode;
//! - reject: malformed app to device reports, each with the error kind the
//!   Rust decoder gives, which the firmware must match.

use std::fmt::Write;

use crate::codec::{
    decode_app, encode_app, encode_device, AppMessage, ButtonAction, Control, ControlKind,
    DecodeError, DeviceMessage, EncoderMode, FirmwareVersion, Hello, InjectEvent, Injected, Label,
    StatusText, MAX_CONTROLS,
};
use crate::{msg, MAX_PAYLOAD_LEN, PROTOCOL_VERSION, REPORT_LEN};

/// Device to app samples the firmware packs. The C++ tests call the pack
/// functions with the same arguments; any mismatch fails byte for byte.
pub fn pack_cases() -> Vec<(&'static str, DeviceMessage, u8)> {
    use ControlKind::{Button, Encoder};
    vec![
        (
            "HELLO",
            DeviceMessage::Hello(Hello {
                firmware: FirmwareVersion {
                    major: 0,
                    minor: 1,
                    patch: 2,
                },
                encoder_mode: EncoderMode::Mapped,
                controls: vec![
                    Control {
                        id: 1,
                        kind: Button,
                        push: None,
                    },
                    Control {
                        id: 2,
                        kind: Button,
                        push: None,
                    },
                    Control {
                        id: 9,
                        kind: Encoder,
                        push: None,
                    },
                ],
            }),
            0,
        ),
        (
            "HELLO_EMPTY",
            DeviceMessage::Hello(Hello {
                firmware: FirmwareVersion {
                    major: 1,
                    minor: 0,
                    patch: 0,
                },
                encoder_mode: EncoderMode::Volume,
                controls: vec![],
            }),
            255,
        ),
        (
            "HELLO_MAX",
            DeviceMessage::Hello(Hello {
                firmware: FirmwareVersion {
                    major: 2,
                    minor: 3,
                    patch: 4,
                },
                encoder_mode: EncoderMode::Menu,
                // id i, Button when i is even, Encoder when odd.
                controls: (0..MAX_CONTROLS as u8)
                    .map(|id| Control {
                        id,
                        kind: if id % 2 == 0 { Button } else { Encoder },
                        push: None,
                    })
                    .collect(),
            }),
            1,
        ),
        (
            // v2 (D-094): encoder 5's push switch is button 6.
            "HELLO_PUSH",
            DeviceMessage::Hello(Hello {
                firmware: FirmwareVersion {
                    major: 0,
                    minor: 2,
                    patch: 0,
                },
                encoder_mode: EncoderMode::Volume,
                controls: vec![
                    Control::button(1),
                    Control::encoder(5, Some(6)),
                    Control::button(6),
                    Control::encoder(7, None),
                ],
            }),
            2,
        ),
        (
            "BUTTON_LONG_PRESS",
            DeviceMessage::Button {
                control: 4,
                action: ButtonAction::LongPress,
            },
            9,
        ),
        (
            "ENCODER_NEGATIVE",
            DeviceMessage::Encoder {
                control: 9,
                delta: -3,
            },
            10,
        ),
        ("MENU_SELECT", DeviceMessage::MenuSelect { item: 42 }, 11),
        (
            "HEARTBEAT",
            DeviceMessage::Heartbeat {
                uptime_s: 0x0102_0304,
                encoder_mode: EncoderMode::Menu,
            },
            12,
        ),
    ]
}

/// App to device samples the firmware parses.
pub fn parse_cases() -> Vec<(&'static str, AppMessage, u8)> {
    let label = |s| Label::new(s).expect("sample label is valid");
    let text = |s| StatusText::new(s).expect("sample text is valid");
    vec![
        ("HELLO_ACK", AppMessage::HelloAck { paired: true }, 1),
        (
            "MENU_ITEM",
            AppMessage::MenuItem {
                total: 3,
                index: 1,
                item: 42,
                label: label("Mail"),
            },
            2,
        ),
        ("MENU_CLEAR", AppMessage::MenuClear, 3),
        (
            "STATUS",
            AppMessage::Status {
                text: text("Homepage"),
            },
            4,
        ),
        ("STATUS_EMPTY", AppMessage::Status { text: text("") }, 5),
        ("REQUEST_HELLO", AppMessage::RequestHello, 6),
        (
            "TEST_INJECT",
            AppMessage::TestInject {
                events: vec![
                    InjectEvent {
                        event: Injected::Button {
                            control: 4,
                            action: ButtonAction::Down,
                        },
                        delay_ms: 16,
                    },
                    InjectEvent {
                        event: Injected::Encoder {
                            control: 9,
                            delta: -3,
                        },
                        delay_ms: 256,
                    },
                ],
            },
            7,
        ),
    ]
}

/// A report built by hand, for malformed cases the encoder won't produce.
fn raw(msg_type: u8, payload: &[u8]) -> [u8; REPORT_LEN] {
    let mut r = [0u8; REPORT_LEN];
    r[0] = PROTOCOL_VERSION;
    r[1] = msg_type;
    r[3] = payload.len() as u8;
    r[4..4 + payload.len()].copy_from_slice(payload);
    r
}

/// MENU payload with a 16 byte label field.
fn menu(total: u8, index: u8, item: u8, label: &[u8]) -> Vec<u8> {
    let mut p = vec![total, index, item, label.len() as u8];
    p.resize(4 + 16, 0);
    p[4..4 + label.len()].copy_from_slice(label);
    p
}

/// STATUS payload with a 20 byte text field.
fn status(text: &[u8]) -> Vec<u8> {
    let mut p = vec![text.len() as u8];
    p.resize(1 + 20, 0);
    p[1..1 + text.len()].copy_from_slice(text);
    p
}

/// Malformed app to device reports, one per rejection rule.
pub fn reject_cases() -> Vec<(&'static str, [u8; REPORT_LEN])> {
    let mut v = Vec::new();
    let mut r = raw(msg::REQUEST_HELLO, &[]);
    r[0] = 9;
    v.push(("bad_version", r));
    let mut r = raw(msg::HELLO_ACK, &[1]);
    r[3] = (MAX_PAYLOAD_LEN + 1) as u8;
    v.push(("len_over_max", r));
    let mut r = raw(msg::HELLO_ACK, &[1]);
    r[20] = 0xAA;
    v.push(("trailing_byte", r));
    v.push(("unknown_type", raw(0x85, &[])));
    v.push(("device_to_app_type", raw(msg::HELLO, &[0, 0, 0, 0, 0])));
    v.push(("hello_ack_empty", raw(msg::HELLO_ACK, &[])));
    v.push(("hello_ack_bad_value", raw(msg::HELLO_ACK, &[2])));
    v.push(("request_hello_payload", raw(msg::REQUEST_HELLO, &[0])));
    v.push(("menu_short", raw(msg::MENU, &[1, 0, 0, 1, b'A'])));
    v.push(("menu_index_ge_total", raw(msg::MENU, &menu(2, 2, 1, b"A"))));
    v.push(("menu_label_empty", raw(msg::MENU, &menu(2, 0, 1, b""))));
    let mut p = menu(2, 0, 1, b"A");
    p[3] = 17;
    v.push(("menu_label_len_17", raw(msg::MENU, &p)));
    v.push((
        "menu_label_control_char",
        raw(msg::MENU, &menu(2, 0, 1, b"\x07")),
    ));
    let mut p = menu(2, 0, 1, b"A");
    p[9] = b'Z';
    v.push(("menu_label_padding", raw(msg::MENU, &p)));
    v.push(("menu_clear_with_item", raw(msg::MENU, &menu(0, 0, 5, b""))));
    v.push(("menu_clear_with_index", raw(msg::MENU, &menu(0, 1, 0, b""))));
    let mut p = status(b"ok");
    p[0] = 21;
    v.push(("status_len_21", raw(msg::STATUS, &p)));
    v.push(("status_non_ascii", raw(msg::STATUS, &status(&[b'o', 0xC3]))));
    let mut p = status(b"ok");
    p[20] = b'!';
    v.push(("status_padding", raw(msg::STATUS, &p)));
    v.push(("inject_empty", raw(msg::TEST_INJECT, &[])));
    v.push(("inject_count_zero", raw(msg::TEST_INJECT, &[0])));
    let mut p = vec![12u8];
    p.resize(1 + 5 * 11, 0);
    v.push(("inject_count_12", raw(msg::TEST_INJECT, &p)));
    v.push((
        "inject_size_mismatch",
        raw(msg::TEST_INJECT, &[2, msg::BUTTON, 1, 1, 0, 0]),
    ));
    v.push((
        "inject_bad_event_type",
        raw(msg::TEST_INJECT, &[1, msg::HELLO, 1, 1, 0, 0]),
    ));
    v.push((
        "inject_bad_action",
        raw(msg::TEST_INJECT, &[1, msg::BUTTON, 1, 9, 0, 0]),
    ));
    v.push((
        "inject_zero_delta",
        raw(msg::TEST_INJECT, &[1, msg::ENCODER, 1, 0, 0, 0]),
    ));
    v
}

/// The `DecodeError` variant name, which the C++ `parse_error_name` returns.
pub fn error_kind(e: &DecodeError) -> &'static str {
    match e {
        DecodeError::ReportLength { .. } => "ReportLength",
        DecodeError::Version { .. } => "Version",
        DecodeError::PayloadLength { .. } => "PayloadLength",
        DecodeError::TrailingBytes => "TrailingBytes",
        DecodeError::UnknownType { .. } => "UnknownType",
        DecodeError::PayloadSize { .. } => "PayloadSize",
        DecodeError::InvalidField { .. } => "InvalidField",
        DecodeError::TooManyControls { .. } => "TooManyControls",
        DecodeError::DuplicateControl { .. } => "DuplicateControl",
        DecodeError::BadPush { .. } => "BadPush",
        DecodeError::TooManyEvents { .. } => "TooManyEvents",
    }
}

fn bytes(r: &[u8; REPORT_LEN]) -> String {
    r.chunks(16)
        .map(|c| {
            let line: Vec<String> = c.iter().map(|b| format!("0x{b:02X}")).collect();
            format!("    {}", line.join(", "))
        })
        .collect::<Vec<_>>()
        .join(",\n")
}

/// The full text of `vectors.h`. Panics if a sample doesn't encode or a
/// reject case decodes; both are bugs in this file, caught by `cargo test`.
pub fn c_vectors() -> String {
    let mut h = String::new();
    let _ = writeln!(
        h,
        "// GENERATED by scripts/gen-header.sh from the protocol crate's codec. Do not edit.\n\
         // Golden reports for the firmware's native tests (D-035).\n\n\
         #ifndef DETENT_VECTORS_H\n#define DETENT_VECTORS_H\n\n\
         #include <stdint.h>\n\n#include \"detent_protocol.h\"\n\n\
         typedef struct {{\n  const char* name;\n  const char* error;\n  uint8_t bytes[DETENT_REPORT_LEN];\n}} DetentRejectVector;\n"
    );

    let _ = writeln!(h, "// Device to app: the firmware must pack these exactly.");
    for (name, m, seq) in pack_cases() {
        let r = encode_device(seq, &m).expect("pack sample encodes");
        let _ = writeln!(h, "// seq {seq}: {m:?}");
        let _ = writeln!(
            h,
            "static const uint8_t DETENT_VEC_PACK_{name}[DETENT_REPORT_LEN] = {{\n{}\n}};\n",
            bytes(&r)
        );
    }

    let _ = writeln!(
        h,
        "// App to device: the firmware must parse these to the listed fields."
    );
    for (name, m, seq) in parse_cases() {
        let r = encode_app(seq, &m).expect("parse sample encodes");
        let _ = writeln!(h, "// seq {seq}: {m:?}");
        let _ = writeln!(
            h,
            "static const uint8_t DETENT_VEC_PARSE_{name}[DETENT_REPORT_LEN] = {{\n{}\n}};\n",
            bytes(&r)
        );
    }

    let rejects = reject_cases();
    let _ = writeln!(
        h,
        "// App to device, malformed: the firmware must reject each with the Rust error kind."
    );
    let _ = writeln!(h, "#define DETENT_VEC_REJECT_COUNT {}", rejects.len());
    let _ = writeln!(
        h,
        "static const DetentRejectVector DETENT_VEC_REJECTS[DETENT_VEC_REJECT_COUNT] = {{"
    );
    for (name, r) in &rejects {
        let err = decode_app(r).expect_err("reject case must not decode");
        let _ = writeln!(
            h,
            "  {{\"{name}\", \"{}\", {{\n{}\n  }}}},",
            error_kind(&err),
            bytes(r)
        );
    }
    let _ = writeln!(h, "}};\n\n#endif  // DETENT_VECTORS_H");
    h
}
