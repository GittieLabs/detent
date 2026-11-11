//! Property tests for the device to app codec. M1 exit criterion: the codec
//! rejects malformed reports without panicking.

use detent_protocol::codec::{
    decode_device, encode_device, ButtonAction, Control, ControlKind, DeviceMessage, EncoderMode,
    FirmwareVersion, Hello, MAX_CONTROLS,
};
use detent_protocol::{msg, PROTOCOL_VERSION, REPORT_LEN};
use proptest::prelude::*;

fn encoder_mode() -> impl Strategy<Value = EncoderMode> {
    prop_oneof![
        Just(EncoderMode::Volume),
        Just(EncoderMode::Menu),
        Just(EncoderMode::Mapped)
    ]
}

fn kind() -> impl Strategy<Value = ControlKind> {
    prop_oneof![Just(ControlKind::Button), Just(ControlKind::Encoder)]
}

fn hello() -> impl Strategy<Value = Hello> {
    (
        any::<(u8, u8, u8)>(),
        encoder_mode(),
        // Distinct IDs: a set, then paired with kinds.
        prop::collection::btree_set(any::<u8>(), 0..=MAX_CONTROLS),
        prop::collection::vec(kind(), MAX_CONTROLS),
        prop::collection::vec(any::<bool>(), MAX_CONTROLS),
    )
        .prop_map(
            |((major, minor, patch), encoder_mode, ids, kinds, wants_push)| {
                let mut controls: Vec<Control> = ids
                    .into_iter()
                    .zip(kinds)
                    .map(|(id, kind)| Control {
                        id,
                        kind,
                        push: None,
                    })
                    .collect();
                // v2 (D-094): some encoders take a free button as their push
                // switch, while the HELLO still fits one report.
                let buttons: Vec<u8> = controls
                    .iter()
                    .filter(|c| c.kind == ControlKind::Button)
                    .map(|c| c.id)
                    .collect();
                let mut free = buttons.into_iter();
                let mut len = 5 + 2 * controls.len();
                for (c, wants) in controls.iter_mut().zip(wants_push) {
                    if c.kind == ControlKind::Encoder && wants && len < 59 {
                        if let Some(b) = free.next() {
                            c.push = Some(b);
                            len += 1;
                        }
                    }
                }
                Hello {
                    firmware: FirmwareVersion {
                        major,
                        minor,
                        patch,
                    },
                    encoder_mode,
                    controls,
                }
            },
        )
}

fn device_message() -> impl Strategy<Value = DeviceMessage> {
    prop_oneof![
        hello().prop_map(DeviceMessage::Hello),
        (
            any::<u8>(),
            prop_oneof![
                Just(ButtonAction::Down),
                Just(ButtonAction::Up),
                Just(ButtonAction::LongPress)
            ]
        )
            .prop_map(|(control, action)| DeviceMessage::Button { control, action }),
        (any::<u8>(), any::<i8>().prop_filter("nonzero", |d| *d != 0))
            .prop_map(|(control, delta)| DeviceMessage::Encoder { control, delta }),
        any::<u8>().prop_map(|item| DeviceMessage::MenuSelect { item }),
        (any::<u32>(), encoder_mode()).prop_map(|(uptime_s, encoder_mode)| {
            DeviceMessage::Heartbeat {
                uptime_s,
                encoder_mode,
            }
        }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    /// Any byte string, any length: decode returns, never panics.
    #[test]
    fn never_panics_on_arbitrary_bytes(bytes in prop::collection::vec(any::<u8>(), 0..128)) {
        let _ = decode_device(&bytes);
    }

    /// Well formed headers with arbitrary payloads reach the per message
    /// decoders, where the interesting bounds checks live.
    #[test]
    fn never_panics_past_the_header(
        msg_type in prop_oneof![
            Just(msg::HELLO), Just(msg::BUTTON), Just(msg::ENCODER),
            Just(msg::MENU_SELECT), Just(msg::HEARTBEAT), any::<u8>()
        ],
        seq in any::<u8>(),
        len in 0u8..=70,
        body in prop::collection::vec(any::<u8>(), REPORT_LEN - 4),
    ) {
        let mut r = [0u8; REPORT_LEN];
        r[0] = PROTOCOL_VERSION;
        r[1] = msg_type;
        r[2] = seq;
        r[3] = len;
        r[4..].copy_from_slice(&body);
        let _ = decode_device(&r);
    }

    /// Every valid message survives encode then decode unchanged.
    #[test]
    fn round_trips(seq in any::<u8>(), m in device_message()) {
        let report = encode_device(seq, &m).expect("valid message encodes");
        let frame = decode_device(&report).expect("encoded report decodes");
        prop_assert_eq!(frame.seq, seq);
        prop_assert_eq!(frame.msg, m);
    }
}
