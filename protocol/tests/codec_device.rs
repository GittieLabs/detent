//! Device to app codec: decode and encode HELLO, BUTTON, ENCODER, MENU_SELECT,
//! HEARTBEAT, and every rejection rule in `docs/protocol.md`.

use detent_protocol::codec::{
    decode_device, encode_device, ButtonAction, Control, ControlKind, DecodeError, DeviceMessage,
    EncodeError, EncoderMode, FirmwareVersion, Frame, Hello, MAX_CONTROLS,
};
use detent_protocol::{msg, MAX_PAYLOAD_LEN, PROTOCOL_VERSION, REPORT_LEN};

/// Builds a raw 63 byte report by hand, independent of the encoder.
fn raw(msg_type: u8, seq: u8, payload: &[u8]) -> [u8; REPORT_LEN] {
    let mut r = [0u8; REPORT_LEN];
    r[0] = PROTOCOL_VERSION;
    r[1] = msg_type;
    r[2] = seq;
    r[3] = payload.len() as u8;
    r[4..4 + payload.len()].copy_from_slice(payload);
    r
}

fn decoded(msg_type: u8, payload: &[u8]) -> Result<DeviceMessage, DecodeError> {
    decode_device(&raw(msg_type, 7, payload)).map(|f| f.msg)
}

fn sample_hello() -> Hello {
    Hello {
        firmware: FirmwareVersion {
            major: 0,
            minor: 1,
            patch: 2,
        },
        encoder_mode: EncoderMode::Mapped,
        controls: vec![
            Control {
                id: 1,
                kind: ControlKind::Button,
                push: None,
            },
            Control {
                id: 2,
                kind: ControlKind::Button,
                push: None,
            },
            Control {
                id: 9,
                kind: ControlKind::Encoder,
                push: None,
            },
        ],
    }
}

// --- decoding each message ---

#[test]
fn decodes_hello() {
    let payload = [0, 1, 2, 2, 3, 1, 1, 2, 1, 9, 2];
    assert_eq!(
        decoded(msg::HELLO, &payload),
        Ok(DeviceMessage::Hello(sample_hello()))
    );
}

#[test]
fn decodes_hello_with_no_controls() {
    let got = decoded(msg::HELLO, &[1, 0, 0, 0, 0]).unwrap();
    let DeviceMessage::Hello(h) = got else {
        panic!("{got:?}")
    };
    assert_eq!(h.encoder_mode, EncoderMode::Volume);
    assert!(h.controls.is_empty());
}

#[test]
fn decodes_button_actions() {
    for (byte, action) in [
        (1, ButtonAction::Down),
        (2, ButtonAction::Up),
        (3, ButtonAction::LongPress),
    ] {
        assert_eq!(
            decoded(msg::BUTTON, &[4, byte]),
            Ok(DeviceMessage::Button { control: 4, action })
        );
    }
}

#[test]
fn decodes_encoder_both_directions() {
    assert_eq!(
        decoded(msg::ENCODER, &[9, 3]),
        Ok(DeviceMessage::Encoder {
            control: 9,
            delta: 3
        })
    );
    assert_eq!(
        decoded(msg::ENCODER, &[9, (-2i8) as u8]),
        Ok(DeviceMessage::Encoder {
            control: 9,
            delta: -2
        })
    );
}

#[test]
fn decodes_menu_select() {
    assert_eq!(
        decoded(msg::MENU_SELECT, &[42]),
        Ok(DeviceMessage::MenuSelect { item: 42 })
    );
}

#[test]
fn decodes_heartbeat_little_endian() {
    let uptime: u32 = 0x0102_0304;
    let mut payload = uptime.to_le_bytes().to_vec();
    payload.push(1);
    assert_eq!(
        decoded(msg::HEARTBEAT, &payload),
        Ok(DeviceMessage::Heartbeat {
            uptime_s: uptime,
            encoder_mode: EncoderMode::Menu
        })
    );
}

#[test]
fn frame_carries_seq() {
    let frame = decode_device(&raw(msg::MENU_SELECT, 200, &[1])).unwrap();
    assert_eq!(
        frame,
        Frame {
            seq: 200,
            msg: DeviceMessage::MenuSelect { item: 1 }
        }
    );
}

// --- header rejections ---

#[test]
fn rejects_wrong_report_length() {
    for len in [0, 1, 62, 64, 65] {
        let buf = vec![0u8; len];
        assert_eq!(
            decode_device(&buf),
            Err(DecodeError::ReportLength { got: len })
        );
    }
}

#[test]
fn rejects_wrong_version() {
    let mut r = raw(msg::MENU_SELECT, 0, &[1]);
    r[0] = PROTOCOL_VERSION + 1;
    assert_eq!(
        decode_device(&r),
        Err(DecodeError::Version {
            got: PROTOCOL_VERSION + 1
        })
    );
    r[0] = 1;
    assert_eq!(decode_device(&r), Err(DecodeError::Version { got: 1 }));
}

#[test]
fn rejects_len_over_max() {
    let mut r = raw(msg::MENU_SELECT, 0, &[1]);
    r[3] = (MAX_PAYLOAD_LEN + 1) as u8;
    assert_eq!(
        decode_device(&r),
        Err(DecodeError::PayloadLength {
            len: (MAX_PAYLOAD_LEN + 1) as u8
        })
    );
}

#[test]
fn rejects_nonzero_bytes_after_payload() {
    let mut r = raw(msg::MENU_SELECT, 0, &[1]);
    r[10] = 0xAA;
    assert_eq!(decode_device(&r), Err(DecodeError::TrailingBytes));
}

#[test]
fn rejects_unknown_and_app_to_device_types() {
    for t in [
        0x00,
        0x06,
        0x7F,
        msg::HELLO_ACK,
        msg::REQUEST_HELLO,
        msg::TEST_INJECT,
    ] {
        assert_eq!(
            decode_device(&raw(t, 0, &[])),
            Err(DecodeError::UnknownType { msg_type: t }),
            "type {t:#04x}"
        );
    }
}

// --- payload rejections ---

#[test]
fn rejects_wrong_payload_size() {
    assert_eq!(
        decoded(msg::BUTTON, &[1]),
        Err(DecodeError::PayloadSize {
            msg_type: msg::BUTTON,
            expected: 2,
            got: 1
        })
    );
    assert_eq!(
        decoded(msg::MENU_SELECT, &[1, 0]),
        Err(DecodeError::PayloadSize {
            msg_type: msg::MENU_SELECT,
            expected: 1,
            got: 2
        })
    );
    // HELLO's size follows its control count.
    assert_eq!(
        decoded(msg::HELLO, &[0, 1, 2, 0, 2, 1, 1]),
        Err(DecodeError::PayloadSize {
            msg_type: msg::HELLO,
            expected: 9,
            got: 7
        })
    );
    assert_eq!(
        decoded(msg::HELLO, &[0, 1]),
        Err(DecodeError::PayloadSize {
            msg_type: msg::HELLO,
            expected: 5,
            got: 2
        })
    );
}

#[test]
fn rejects_invalid_field_values() {
    let bad = |msg_type, field, value| DecodeError::InvalidField {
        msg_type,
        field,
        value,
    };
    assert_eq!(
        decoded(msg::BUTTON, &[1, 0]),
        Err(bad(msg::BUTTON, "action", 0))
    );
    assert_eq!(
        decoded(msg::BUTTON, &[1, 4]),
        Err(bad(msg::BUTTON, "action", 4))
    );
    assert_eq!(
        decoded(msg::ENCODER, &[1, 0]),
        Err(bad(msg::ENCODER, "delta", 0))
    );
    assert_eq!(
        decoded(msg::HEARTBEAT, &[0, 0, 0, 0, 3]),
        Err(bad(msg::HEARTBEAT, "encoder_mode", 3))
    );
    assert_eq!(
        decoded(msg::HELLO, &[0, 0, 0, 9, 0]),
        Err(bad(msg::HELLO, "encoder_mode", 9))
    );
    assert_eq!(
        decoded(msg::HELLO, &[0, 0, 0, 0, 1, 5, 4]),
        Err(bad(msg::HELLO, "kind", 4))
    );
}

#[test]
fn rejects_too_many_controls() {
    // 28 controls would need 61 payload bytes, so len is already over max;
    // the count check fires first on the 5 byte fixed part.
    let mut payload = vec![0, 0, 0, 0, (MAX_CONTROLS + 1) as u8];
    payload.resize(MAX_PAYLOAD_LEN, 1);
    assert_eq!(
        decoded(msg::HELLO, &payload),
        Err(DecodeError::TooManyControls {
            count: (MAX_CONTROLS + 1) as u8
        })
    );
}

#[test]
fn rejects_duplicate_control_ids() {
    assert_eq!(
        decoded(msg::HELLO, &[0, 0, 0, 0, 2, 5, 1, 5, 2]),
        Err(DecodeError::DuplicateControl { id: 5 })
    );
}

// --- encoding ---

#[test]
fn encodes_to_the_documented_bytes() {
    let r = encode_device(
        9,
        &DeviceMessage::Button {
            control: 4,
            action: ButtonAction::LongPress,
        },
    )
    .unwrap();
    assert_eq!(r, raw(msg::BUTTON, 9, &[4, 3]));

    let r = encode_device(0, &DeviceMessage::Hello(sample_hello())).unwrap();
    assert_eq!(r, raw(msg::HELLO, 0, &[0, 1, 2, 2, 3, 1, 1, 2, 1, 9, 2]));
}

#[test]
fn encodes_max_controls_hello() {
    let controls = (0..MAX_CONTROLS as u8).map(Control::button).collect();
    let hello = DeviceMessage::Hello(Hello {
        controls,
        ..sample_hello()
    });
    let r = encode_device(1, &hello).unwrap();
    assert_eq!(r[3] as usize, 5 + 2 * MAX_CONTROLS);
    assert_eq!(decode_device(&r).unwrap().msg, hello);
}

#[test]
fn refuses_to_encode_invalid_messages() {
    let controls = (0..=MAX_CONTROLS as u8).map(Control::button).collect();
    let too_many = DeviceMessage::Hello(Hello {
        controls,
        ..sample_hello()
    });
    assert_eq!(
        encode_device(0, &too_many),
        Err(EncodeError::TooManyControls {
            count: MAX_CONTROLS + 1
        })
    );

    let dup = vec![
        Control {
            id: 3,
            kind: ControlKind::Button,
            push: None,
        },
        Control {
            id: 3,
            kind: ControlKind::Encoder,
            push: None,
        },
    ];
    let dup = DeviceMessage::Hello(Hello {
        controls: dup,
        ..sample_hello()
    });
    assert_eq!(
        encode_device(0, &dup),
        Err(EncodeError::DuplicateControl { id: 3 })
    );

    assert_eq!(
        encode_device(
            0,
            &DeviceMessage::Encoder {
                control: 1,
                delta: 0
            }
        ),
        Err(EncodeError::ZeroDelta)
    );
}

#[test]
fn errors_render_readably() {
    let text = DecodeError::Version { got: 2 }.to_string();
    assert!(text.contains("version") && text.contains('2'), "{text}");
}

// v2 (D-094): an encoder's entry can name its push switch.

#[test]
fn decodes_an_encoder_with_its_push_switch() {
    // Button 1, encoder 5 with push 6, button 6, encoder 7 without one.
    let m = decoded(msg::HELLO, &[0, 2, 0, 0, 4, 1, 1, 5, 3, 6, 6, 1, 7, 2]).unwrap();
    let DeviceMessage::Hello(h) = m else {
        panic!("{m:?}")
    };
    assert_eq!(
        h.controls,
        vec![
            Control::button(1),
            Control::encoder(5, Some(6)),
            Control::button(6),
            Control::encoder(7, None),
        ]
    );
    let r = encode_device(7, &DeviceMessage::Hello(h)).unwrap();
    assert_eq!(
        r,
        raw(msg::HELLO, 7, &[0, 2, 0, 0, 4, 1, 1, 5, 3, 6, 6, 1, 7, 2])
    );
}

#[test]
fn a_push_switch_must_be_one_listed_button_of_one_encoder() {
    let bad = |payload: &[u8]| decoded(msg::HELLO, payload);
    // Push 9 isn't listed.
    assert_eq!(
        bad(&[0, 2, 0, 0, 1, 5, 3, 9]),
        Err(DecodeError::BadPush {
            encoder: 5,
            push: 9
        })
    );
    // Push 7 is an encoder, not a button.
    assert_eq!(
        bad(&[0, 2, 0, 0, 2, 5, 3, 7, 7, 2]),
        Err(DecodeError::BadPush {
            encoder: 5,
            push: 7
        })
    );
    // Two encoders share button 6.
    assert_eq!(
        bad(&[0, 2, 0, 0, 3, 5, 3, 6, 7, 3, 6, 6, 1]),
        Err(DecodeError::BadPush {
            encoder: 7,
            push: 6
        })
    );
    // An encoder that is its own push.
    assert_eq!(
        bad(&[0, 2, 0, 0, 1, 5, 3, 5]),
        Err(DecodeError::BadPush {
            encoder: 5,
            push: 5
        })
    );
    // The count says two entries but the bytes stop inside the second.
    assert!(matches!(
        bad(&[0, 2, 0, 0, 2, 1, 1, 5, 3]),
        Err(DecodeError::PayloadSize { .. })
    ));
    // Extra bytes after the last entry.
    assert!(matches!(
        bad(&[0, 2, 0, 0, 1, 1, 1, 0]),
        Err(DecodeError::PayloadSize { .. })
    ));
}

#[test]
fn refuses_to_encode_a_bad_push_or_a_hello_too_long() {
    let hello = |controls| {
        DeviceMessage::Hello(Hello {
            controls,
            ..sample_hello()
        })
    };
    assert_eq!(
        encode_device(0, &hello(vec![Control::encoder(5, Some(6))])),
        Err(EncodeError::BadPush {
            encoder: 5,
            push: 6
        })
    );
    let mut on_button = Control::button(1);
    on_button.push = Some(2);
    assert_eq!(
        encode_device(0, &hello(vec![on_button, Control::button(2)])),
        Err(EncodeError::BadPush {
            encoder: 1,
            push: 2
        })
    );
    // 13 encoders each with its own push button is 26 controls in 5 + 13 * 5
    // = 70 bytes: under the control limit, over the payload.
    let controls = (0..13u8)
        .flat_map(|i| [Control::encoder(i, Some(100 + i)), Control::button(100 + i)])
        .collect();
    assert_eq!(
        encode_device(0, &hello(controls)),
        Err(EncodeError::HelloTooLong { len: 70 })
    );
}
