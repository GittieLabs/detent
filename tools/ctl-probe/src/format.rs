//! Turns one HID read into one line of output.

use std::time::Duration;

use detent_protocol::codec::{
    decode_device, ButtonAction, ControlKind, DeviceMessage, EncoderMode, Frame,
};
use detent_protocol::VENDOR_REPORT_ID;

/// What a HID read turned out to be.
#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    /// A valid device to app message.
    Message(Frame<DeviceMessage>),
    /// A vendor report the codec rejected, with the reason.
    Rejected(String),
    /// Another collection on the interface (Consumer Control). Not app traffic.
    Foreign { report_id: u8 },
    /// An empty read.
    Empty,
}

/// Classifies a hidapi read: report ID first, then the 63 byte report.
pub fn classify(read: &[u8]) -> Event {
    match read.split_first() {
        None => Event::Empty,
        Some((&VENDOR_REPORT_ID, report)) => match decode_device(report) {
            Ok(frame) => Event::Message(frame),
            Err(err) => Event::Rejected(err.to_string()),
        },
        Some((&report_id, _)) => Event::Foreign { report_id },
    }
}

fn mode(m: EncoderMode) -> &'static str {
    match m {
        EncoderMode::Volume => "volume",
        EncoderMode::Menu => "menu",
        EncoderMode::Mapped => "mapped",
    }
}

fn kind(k: ControlKind) -> &'static str {
    match k {
        ControlKind::Button => "button",
        ControlKind::Encoder => "encoder",
    }
}

fn action(a: ButtonAction) -> &'static str {
    match a {
        ButtonAction::Down => "down",
        ButtonAction::Up => "up",
        ButtonAction::LongPress => "long press",
    }
}

/// The message part of a line, without time or seq.
pub fn describe(m: &DeviceMessage) -> String {
    match m {
        DeviceMessage::Hello(h) => {
            let controls: Vec<String> = h
                .controls
                .iter()
                .map(|c| match c.push {
                    Some(push) => format!("{} {} (push {push})", c.id, kind(c.kind)),
                    None => format!("{} {}", c.id, kind(c.kind)),
                })
                .collect();
            format!(
                "HELLO fw {}.{}.{}, mode {}, {} controls{}{}",
                h.firmware.major,
                h.firmware.minor,
                h.firmware.patch,
                mode(h.encoder_mode),
                h.controls.len(),
                if controls.is_empty() { "" } else { ": " },
                controls.join(", ")
            )
        }
        DeviceMessage::Button { control, action: a } => format!("BUTTON {control} {}", action(*a)),
        DeviceMessage::Encoder { control, delta } => format!("ENCODER {control} {delta:+}"),
        DeviceMessage::MenuSelect { item } => format!("MENU_SELECT item {item}"),
        DeviceMessage::Heartbeat {
            uptime_s,
            encoder_mode,
        } => format!("HEARTBEAT uptime {uptime_s}s, mode {}", mode(*encoder_mode)),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Hex with trailing zeros collapsed, so 64 byte reads stay readable.
pub fn hex_trimmed(bytes: &[u8]) -> String {
    let end = bytes.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    let zeros = bytes.len() - end;
    match (end, zeros) {
        (0, 0) => String::new(),
        (_, 0) => hex(&bytes[..end]),
        (0, z) => format!("00 x{z}"),
        (_, z) => format!("{} ... 00 x{z}", hex(&bytes[..end])),
    }
}

/// One output line for a read, or `None` for reads that aren't shown
/// (foreign collections and empty reads, unless `raw`).
pub fn line(at: Duration, read: &[u8], raw: bool) -> Option<String> {
    let t = at.as_secs_f64();
    let body = match classify(read) {
        Event::Message(f) => format!("{t:>8.3}s  seq {:>3}  {}", f.seq, describe(&f.msg)),
        Event::Rejected(why) => format!("{t:>8.3}s  rejected: {why}"),
        Event::Foreign { report_id } if raw => {
            format!("{t:>8.3}s  report id {report_id} (not app traffic)")
        }
        Event::Foreign { .. } | Event::Empty => return None,
    };
    Some(if raw {
        format!("{body}\n          {}", hex_trimmed(read))
    } else {
        body
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use detent_protocol::codec::{encode_device, Control, FirmwareVersion, Hello};
    use detent_protocol::PROTOCOL_VERSION;

    /// A hidapi read: report ID 6, then the encoded report.
    fn read_of(seq: u8, m: &DeviceMessage) -> Vec<u8> {
        let mut r = vec![VENDOR_REPORT_ID];
        r.extend_from_slice(&encode_device(seq, m).expect("fixture encodes"));
        r
    }

    fn at() -> Duration {
        Duration::from_millis(1_234)
    }

    #[test]
    fn describes_every_message() {
        let hello = DeviceMessage::Hello(Hello {
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
                Control::encoder(9, Some(1)),
                Control::encoder(5, None),
            ],
        });
        assert_eq!(
            describe(&hello),
            "HELLO fw 0.1.2, mode mapped, 3 controls: 1 button, 9 encoder (push 1), 5 encoder"
        );
        let empty = DeviceMessage::Hello(Hello {
            firmware: FirmwareVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            encoder_mode: EncoderMode::Volume,
            controls: vec![],
        });
        assert_eq!(describe(&empty), "HELLO fw 1.0.0, mode volume, 0 controls");
        assert_eq!(
            describe(&DeviceMessage::Button {
                control: 4,
                action: ButtonAction::LongPress
            }),
            "BUTTON 4 long press"
        );
        assert_eq!(
            describe(&DeviceMessage::Encoder {
                control: 9,
                delta: -3
            }),
            "ENCODER 9 -3"
        );
        assert_eq!(
            describe(&DeviceMessage::Encoder {
                control: 9,
                delta: 2
            }),
            "ENCODER 9 +2"
        );
        assert_eq!(
            describe(&DeviceMessage::MenuSelect { item: 42 }),
            "MENU_SELECT item 42"
        );
        assert_eq!(
            describe(&DeviceMessage::Heartbeat {
                uptime_s: 61,
                encoder_mode: EncoderMode::Menu
            }),
            "HEARTBEAT uptime 61s, mode menu"
        );
    }

    #[test]
    fn line_has_time_seq_and_message() {
        let read = read_of(7, &DeviceMessage::MenuSelect { item: 42 });
        assert_eq!(
            line(at(), &read, false).unwrap(),
            "   1.234s  seq   7  MENU_SELECT item 42"
        );
    }

    #[test]
    fn raw_appends_hex() {
        let read = read_of(7, &DeviceMessage::MenuSelect { item: 42 });
        let l = line(at(), &read, true).unwrap();
        assert!(
            l.ends_with(&format!(
                "\n          06 {PROTOCOL_VERSION:02x} 04 07 01 2a ... 00 x58"
            )),
            "{l}"
        );
    }

    #[test]
    fn rejected_reports_say_why() {
        // The spike firmware's heartbeat: 'D', counter. Version byte is 0x44.
        let mut read = vec![VENDOR_REPORT_ID, b'D', 0x10];
        read.resize(64, 0);
        assert_eq!(
            line(at(), &read, false).unwrap(),
            format!("   1.234s  rejected: protocol version 68, expected {PROTOCOL_VERSION}")
        );
    }

    #[test]
    fn consumer_control_is_hidden_unless_raw() {
        let read = [4u8, 0xe9, 0x00];
        assert_eq!(classify(&read), Event::Foreign { report_id: 4 });
        assert_eq!(line(at(), &read, false), None);
        assert_eq!(
            line(at(), &read, true).unwrap(),
            "   1.234s  report id 4 (not app traffic)\n          04 e9 ... 00 x1"
        );
    }

    #[test]
    fn empty_reads_are_skipped() {
        assert_eq!(classify(&[]), Event::Empty);
        assert_eq!(line(at(), &[], true), None);
    }

    #[test]
    fn hex_collapses_trailing_zeros() {
        assert_eq!(
            hex_trimmed(&[0x06, 0x44, 0x01, 0, 0, 0]),
            "06 44 01 ... 00 x3"
        );
        assert_eq!(hex_trimmed(&[0, 0]), "00 x2");
        assert_eq!(hex_trimmed(&[0xab]), "ab");
        assert_eq!(hex_trimmed(&[]), "");
    }
}
