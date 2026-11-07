//! Property tests for the app to device codec: no panics, round trips.

use detent_protocol::codec::{
    decode_app, encode_app, AppMessage, ButtonAction, InjectEvent, Injected, Label, StatusText,
    MAX_INJECT_EVENTS,
};
use detent_protocol::{msg, PROTOCOL_VERSION, REPORT_LEN};
use proptest::prelude::*;

fn printable(min: usize, max: usize) -> impl Strategy<Value = String> {
    prop::collection::vec(0x20u8..=0x7E, min..=max)
        .prop_map(|b| String::from_utf8(b).expect("ASCII"))
}

fn injected() -> impl Strategy<Value = Injected> {
    prop_oneof![
        (
            any::<u8>(),
            prop_oneof![
                Just(ButtonAction::Down),
                Just(ButtonAction::Up),
                Just(ButtonAction::LongPress)
            ]
        )
            .prop_map(|(control, action)| Injected::Button { control, action }),
        (any::<u8>(), any::<i8>().prop_filter("nonzero", |d| *d != 0))
            .prop_map(|(control, delta)| Injected::Encoder { control, delta }),
    ]
}

fn app_message() -> impl Strategy<Value = AppMessage> {
    prop_oneof![
        any::<bool>().prop_map(|paired| AppMessage::HelloAck { paired }),
        Just(AppMessage::MenuClear),
        (1u8..=255, any::<u8>(), printable(1, 16)).prop_flat_map(|(total, item, label)| {
            (0..total).prop_map(move |index| AppMessage::MenuItem {
                total,
                index,
                item,
                label: Label::new(&label).expect("valid label"),
            })
        }),
        printable(0, 20).prop_map(|t| AppMessage::Status {
            text: StatusText::new(&t).expect("valid text")
        }),
        Just(AppMessage::RequestHello),
        prop::collection::vec((injected(), any::<u16>()), 1..=MAX_INJECT_EVENTS).prop_map(|evs| {
            AppMessage::TestInject {
                events: evs
                    .into_iter()
                    .map(|(event, delay_ms)| InjectEvent { event, delay_ms })
                    .collect(),
            }
        }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn never_panics_on_arbitrary_bytes(bytes in prop::collection::vec(any::<u8>(), 0..128)) {
        let _ = decode_app(&bytes);
    }

    #[test]
    fn never_panics_past_the_header(
        msg_type in prop_oneof![
            Just(msg::HELLO_ACK), Just(msg::MENU), Just(msg::STATUS),
            Just(msg::REQUEST_HELLO), Just(msg::TEST_INJECT), any::<u8>()
        ],
        len in 0u8..=70,
        body in prop::collection::vec(any::<u8>(), REPORT_LEN - 4),
    ) {
        let mut r = [0u8; REPORT_LEN];
        r[0] = PROTOCOL_VERSION;
        r[1] = msg_type;
        r[3] = len;
        r[4..].copy_from_slice(&body);
        let _ = decode_app(&r);
    }

    #[test]
    fn round_trips(seq in any::<u8>(), m in app_message()) {
        let report = encode_app(seq, &m).expect("valid message encodes");
        let frame = decode_app(&report).expect("encoded report decodes");
        prop_assert_eq!(frame.seq, seq);
        prop_assert_eq!(frame.msg, m);
    }

    /// Text constructors never panic on any string.
    #[test]
    fn text_constructors_never_panic(s in ".{0,40}") {
        let _ = Label::new(&s);
        let _ = StatusText::new(&s);
    }
}
