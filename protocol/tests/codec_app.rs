//! App to device codec: HELLO_ACK, MENU, STATUS, REQUEST_HELLO, TEST_INJECT,
//! text validation, and every rejection rule in `docs/protocol.md`.

use detent_protocol::codec::{
    decode_app, encode_app, AppMessage, ButtonAction, DecodeError, EncodeError, InjectEvent,
    Injected, Label, StatusText, TextError, MAX_INJECT_EVENTS, MAX_LABEL_LEN, MAX_STATUS_LEN,
};
use detent_protocol::{msg, PROTOCOL_VERSION, REPORT_LEN};

fn raw(msg_type: u8, seq: u8, payload: &[u8]) -> [u8; REPORT_LEN] {
    let mut r = [0u8; REPORT_LEN];
    r[0] = PROTOCOL_VERSION;
    r[1] = msg_type;
    r[2] = seq;
    r[3] = payload.len() as u8;
    r[4..4 + payload.len()].copy_from_slice(payload);
    r
}

fn decoded(msg_type: u8, payload: &[u8]) -> Result<AppMessage, DecodeError> {
    decode_app(&raw(msg_type, 3, payload)).map(|f| f.msg)
}

fn invalid(msg_type: u8, field: &'static str, value: u8) -> DecodeError {
    DecodeError::InvalidField {
        msg_type,
        field,
        value,
    }
}

/// MENU payload: total, index, item, label_len, then a 16 byte label field.
fn menu_payload(total: u8, index: u8, item: u8, label: &[u8]) -> Vec<u8> {
    let mut p = vec![total, index, item, label.len() as u8];
    let mut field = [0u8; 16];
    field[..label.len()].copy_from_slice(label);
    p.extend_from_slice(&field);
    p
}

/// STATUS payload: text_len, then a 20 byte text field.
fn status_payload(text: &[u8]) -> Vec<u8> {
    let mut p = vec![text.len() as u8];
    let mut field = [0u8; 20];
    field[..text.len()].copy_from_slice(text);
    p.extend_from_slice(&field);
    p
}

// --- text types ---

#[test]
fn label_accepts_printable_ascii_up_to_16() {
    assert_eq!(MAX_LABEL_LEN, 16);
    assert_eq!(Label::new("Homepage").unwrap().as_str(), "Homepage");
    assert!(Label::new("abcdefghijklmnop").is_ok());
    assert_eq!(
        Label::new("abcdefghijklmnopq"),
        Err(TextError::TooLong { max: 16, got: 17 })
    );
    assert_eq!(Label::new(""), Err(TextError::Empty));
    assert_eq!(Label::new("caf\u{e9}"), Err(TextError::NotPrintableAscii));
    assert_eq!(Label::new("tab\there"), Err(TextError::NotPrintableAscii));
}

#[test]
fn status_text_allows_empty_and_up_to_20() {
    assert_eq!(MAX_STATUS_LEN, 20);
    assert_eq!(StatusText::new("").unwrap().as_str(), "");
    assert!(StatusText::new("Homepage on display2").is_ok());
    assert_eq!(
        StatusText::new("Homepage on display 2"),
        Err(TextError::TooLong { max: 20, got: 21 })
    );
    assert_eq!(StatusText::new("\u{1}"), Err(TextError::NotPrintableAscii));
}

// --- decoding each message ---

#[test]
fn decodes_hello_ack() {
    assert_eq!(
        decoded(msg::HELLO_ACK, &[1]),
        Ok(AppMessage::HelloAck { paired: true })
    );
    assert_eq!(
        decoded(msg::HELLO_ACK, &[0]),
        Ok(AppMessage::HelloAck { paired: false })
    );
}

#[test]
fn decodes_menu_item_and_clear() {
    assert_eq!(
        decoded(msg::MENU, &menu_payload(3, 1, 42, b"Mail")),
        Ok(AppMessage::MenuItem {
            total: 3,
            index: 1,
            item: 42,
            label: Label::new("Mail").unwrap()
        })
    );
    assert_eq!(
        decoded(msg::MENU, &menu_payload(0, 0, 0, b"")),
        Ok(AppMessage::MenuClear)
    );
}

#[test]
fn decodes_status() {
    assert_eq!(
        decoded(msg::STATUS, &status_payload(b"Homepage")),
        Ok(AppMessage::Status {
            text: StatusText::new("Homepage").unwrap()
        })
    );
    assert_eq!(
        decoded(msg::STATUS, &status_payload(b"")),
        Ok(AppMessage::Status {
            text: StatusText::new("").unwrap()
        })
    );
}

#[test]
fn decodes_request_hello() {
    assert_eq!(
        decoded(msg::REQUEST_HELLO, &[]),
        Ok(AppMessage::RequestHello)
    );
}

#[test]
fn decodes_test_inject() {
    let mut p = vec![2];
    p.extend_from_slice(&[msg::BUTTON, 4, 1, 0x10, 0x00]); // button 4 down, 16 ms
    p.extend_from_slice(&[msg::ENCODER, 9, (-3i8) as u8, 0x00, 0x01]); // 256 ms
    assert_eq!(
        decoded(msg::TEST_INJECT, &p),
        Ok(AppMessage::TestInject {
            events: vec![
                InjectEvent {
                    event: Injected::Button {
                        control: 4,
                        action: ButtonAction::Down
                    },
                    delay_ms: 16
                },
                InjectEvent {
                    event: Injected::Encoder {
                        control: 9,
                        delta: -3
                    },
                    delay_ms: 256
                },
            ]
        })
    );
}

// --- rejections ---

#[test]
fn rejects_device_to_app_and_unknown_types() {
    for t in [msg::HELLO, msg::BUTTON, msg::HEARTBEAT, 0x80, 0x85, 0xFF] {
        assert_eq!(
            decode_app(&raw(t, 0, &[])),
            Err(DecodeError::UnknownType { msg_type: t }),
            "type {t:#04x}"
        );
    }
}

#[test]
fn rejects_wrong_payload_sizes() {
    let size = |t, expected, got| DecodeError::PayloadSize {
        msg_type: t,
        expected,
        got,
    };
    assert_eq!(
        decoded(msg::HELLO_ACK, &[]),
        Err(size(msg::HELLO_ACK, 1, 0))
    );
    assert_eq!(
        decoded(msg::REQUEST_HELLO, &[0]),
        Err(size(msg::REQUEST_HELLO, 0, 1))
    );
    assert_eq!(
        decoded(msg::MENU, &[1, 0, 0, 1, b'A']),
        Err(size(msg::MENU, 20, 5))
    );
    assert_eq!(
        decoded(msg::STATUS, &[1, b'A']),
        Err(size(msg::STATUS, 21, 2))
    );
    // TEST_INJECT's size follows its count.
    assert_eq!(
        decoded(msg::TEST_INJECT, &[2, msg::BUTTON, 1, 1, 0, 0]),
        Err(size(msg::TEST_INJECT, 11, 6))
    );
    assert_eq!(
        decoded(msg::TEST_INJECT, &[]),
        Err(size(msg::TEST_INJECT, 1, 0))
    );
}

#[test]
fn rejects_bad_hello_ack() {
    assert_eq!(
        decoded(msg::HELLO_ACK, &[2]),
        Err(invalid(msg::HELLO_ACK, "paired", 2))
    );
}

#[test]
fn rejects_bad_menu_fields() {
    let m = msg::MENU;
    assert_eq!(
        decoded(m, &menu_payload(2, 2, 1, b"A")),
        Err(invalid(m, "index", 2))
    );
    assert_eq!(
        decoded(m, &menu_payload(2, 0, 1, b"")),
        Err(invalid(m, "label_len", 0))
    );
    let mut p = menu_payload(2, 0, 1, b"A");
    p[3] = 17;
    assert_eq!(decoded(m, &p), Err(invalid(m, "label_len", 17)));
    assert_eq!(
        decoded(m, &menu_payload(2, 0, 1, b"\x07")),
        Err(invalid(m, "label", 0x07))
    );
    // Padding inside the 16 byte label field must be zero.
    let mut p = menu_payload(2, 0, 1, b"A");
    p[4 + 5] = b'Z';
    assert_eq!(decoded(m, &p), Err(DecodeError::TrailingBytes));
    // A clear carries nothing else.
    assert_eq!(
        decoded(m, &menu_payload(0, 0, 5, b"")),
        Err(invalid(m, "item", 5))
    );
    assert_eq!(
        decoded(m, &menu_payload(0, 1, 0, b"")),
        Err(invalid(m, "index", 1))
    );
    assert_eq!(
        decoded(m, &menu_payload(0, 0, 0, b"A")),
        Err(invalid(m, "label_len", 1))
    );
}

#[test]
fn rejects_bad_status_fields() {
    let s = msg::STATUS;
    let mut p = status_payload(b"ok");
    p[0] = 21;
    assert_eq!(decoded(s, &p), Err(invalid(s, "text_len", 21)));
    assert_eq!(
        decoded(s, &status_payload(&[b'o', 0xC3])),
        Err(invalid(s, "text", 0xC3))
    );
    let mut p = status_payload(b"ok");
    p[1 + 19] = b'!';
    assert_eq!(decoded(s, &p), Err(DecodeError::TrailingBytes));
}

#[test]
fn rejects_bad_test_inject() {
    let t = msg::TEST_INJECT;
    assert_eq!(decoded(t, &[0]), Err(invalid(t, "count", 0)));
    let mut p = vec![(MAX_INJECT_EVENTS + 1) as u8];
    p.resize(1 + 5 * MAX_INJECT_EVENTS, 0);
    assert_eq!(
        decoded(t, &p),
        Err(DecodeError::TooManyEvents {
            count: (MAX_INJECT_EVENTS + 1) as u8
        })
    );
    assert_eq!(
        decoded(t, &[1, msg::HELLO, 1, 1, 0, 0]),
        Err(invalid(t, "event_type", msg::HELLO))
    );
    assert_eq!(
        decoded(t, &[1, msg::BUTTON, 1, 9, 0, 0]),
        Err(invalid(t, "action", 9))
    );
    assert_eq!(
        decoded(t, &[1, msg::ENCODER, 1, 0, 0, 0]),
        Err(invalid(t, "delta", 0))
    );
}

// --- encoding ---

#[test]
fn encodes_to_the_documented_bytes() {
    let r = encode_app(
        5,
        &AppMessage::MenuItem {
            total: 3,
            index: 1,
            item: 42,
            label: Label::new("Mail").unwrap(),
        },
    )
    .unwrap();
    assert_eq!(r, raw(msg::MENU, 5, &menu_payload(3, 1, 42, b"Mail")));
    assert_eq!(
        encode_app(0, &AppMessage::RequestHello).unwrap(),
        raw(msg::REQUEST_HELLO, 0, &[])
    );
    assert_eq!(
        encode_app(1, &AppMessage::HelloAck { paired: true }).unwrap(),
        raw(msg::HELLO_ACK, 1, &[1])
    );
}

#[test]
fn encodes_max_inject_events() {
    let events: Vec<_> = (0..MAX_INJECT_EVENTS as u8)
        .map(|i| InjectEvent {
            event: Injected::Encoder {
                control: i,
                delta: 1,
            },
            delay_ms: 20,
        })
        .collect();
    let m = AppMessage::TestInject { events };
    let r = encode_app(0, &m).unwrap();
    assert_eq!(r[3] as usize, 1 + 5 * MAX_INJECT_EVENTS);
    assert_eq!(decode_app(&r).unwrap().msg, m);
}

#[test]
fn refuses_to_encode_invalid_messages() {
    let label = Label::new("A").unwrap();
    assert_eq!(
        encode_app(
            0,
            &AppMessage::MenuItem {
                total: 2,
                index: 2,
                item: 0,
                label: label.clone()
            }
        ),
        Err(EncodeError::MenuIndex { index: 2, total: 2 })
    );
    assert_eq!(
        encode_app(
            0,
            &AppMessage::MenuItem {
                total: 0,
                index: 0,
                item: 0,
                label
            }
        ),
        Err(EncodeError::MenuIndex { index: 0, total: 0 })
    );
    assert_eq!(
        encode_app(0, &AppMessage::TestInject { events: vec![] }),
        Err(EncodeError::NoEvents)
    );
    let too_many = vec![
        InjectEvent {
            event: Injected::Button {
                control: 1,
                action: ButtonAction::Up
            },
            delay_ms: 0
        };
        MAX_INJECT_EVENTS + 1
    ];
    assert_eq!(
        encode_app(0, &AppMessage::TestInject { events: too_many }),
        Err(EncodeError::TooManyEvents {
            count: MAX_INJECT_EVENTS + 1
        })
    );
    let zero = vec![InjectEvent {
        event: Injected::Encoder {
            control: 1,
            delta: 0,
        },
        delay_ms: 0,
    }];
    assert_eq!(
        encode_app(0, &AppMessage::TestInject { events: zero }),
        Err(EncodeError::ZeroDelta)
    );
}
