//! Device to app messages: HELLO, BUTTON, ENCODER, MENU_SELECT, HEARTBEAT.

use super::{build_report, expect_size, parse_header, DecodeError, EncodeError, Frame};
use crate::{msg, MAX_PAYLOAD_LEN, REPORT_LEN};

/// Most controls a HELLO can list: 5 fixed bytes plus 2 per control must fit
/// in the 59 byte payload. Each encoder with a push switch takes one byte
/// more, so a HELLO with such encoders fits fewer.
pub const MAX_CONTROLS: usize = 27;

/// The `kind` byte of an encoder whose entry carries its push switch's
/// control ID as a third byte (v2, D-094). Decodes to [`ControlKind::Encoder`]
/// with `push` set.
pub const KIND_ENCODER_WITH_PUSH: u8 = 3;

/// Fixed part of HELLO before the control list.
const HELLO_FIXED: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderMode {
    Volume = 0,
    Menu = 1,
    Mapped = 2,
}

impl EncoderMode {
    fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(Self::Volume),
            1 => Some(Self::Menu),
            2 => Some(Self::Mapped),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    Button = 1,
    Encoder = 2,
}

impl ControlKind {
    fn from_byte(b: u8) -> Option<Self> {
        match b {
            1 => Some(Self::Button),
            2 => Some(Self::Encoder),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonAction {
    Down = 1,
    Up = 2,
    /// Fires once at 600 ms held, before Up.
    LongPress = 3,
}

impl ButtonAction {
    pub(super) fn from_byte(b: u8) -> Option<Self> {
        match b {
            1 => Some(Self::Down),
            2 => Some(Self::Up),
            3 => Some(Self::LongPress),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirmwareVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Control {
    pub id: u8,
    pub kind: ControlKind,
    /// An encoder's push switch: the ID of a button in the same HELLO, which
    /// the encoder mode routes with the encoder (D-094). `None` for buttons
    /// and for encoders without one.
    pub push: Option<u8>,
}

impl Control {
    pub fn button(id: u8) -> Self {
        Self {
            id,
            kind: ControlKind::Button,
            push: None,
        }
    }

    pub fn encoder(id: u8, push: Option<u8>) -> Self {
        Self {
            id,
            kind: ControlKind::Encoder,
            push,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hello {
    pub firmware: FirmwareVersion,
    pub encoder_mode: EncoderMode,
    /// At most [`MAX_CONTROLS`], IDs unique.
    pub controls: Vec<Control>,
}

/// A message the device sends to the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceMessage {
    Hello(Hello),
    Button {
        control: u8,
        action: ButtonAction,
    },
    /// Positive is clockwise. Never zero.
    Encoder {
        control: u8,
        delta: i8,
    },
    MenuSelect {
        item: u8,
    },
    Heartbeat {
        uptime_s: u32,
        encoder_mode: EncoderMode,
    },
}

/// Decodes a 63 byte device to app report.
pub fn decode_device(report: &[u8]) -> Result<Frame<DeviceMessage>, DecodeError> {
    let h = parse_header(report)?;
    let t = h.msg_type;
    let p = h.payload;
    let invalid = |field, value| DecodeError::InvalidField {
        msg_type: t,
        field,
        value,
    };

    let msg = match t {
        msg::HELLO => DeviceMessage::Hello(decode_hello(p)?),
        msg::BUTTON => {
            expect_size(t, p, 2)?;
            let action = ButtonAction::from_byte(p[1]).ok_or(invalid("action", p[1]))?;
            DeviceMessage::Button {
                control: p[0],
                action,
            }
        }
        msg::ENCODER => {
            expect_size(t, p, 2)?;
            if p[1] == 0 {
                return Err(invalid("delta", 0));
            }
            DeviceMessage::Encoder {
                control: p[0],
                delta: i8::from_le_bytes([p[1]]),
            }
        }
        msg::MENU_SELECT => {
            expect_size(t, p, 1)?;
            DeviceMessage::MenuSelect { item: p[0] }
        }
        msg::HEARTBEAT => {
            expect_size(t, p, 5)?;
            let encoder_mode = EncoderMode::from_byte(p[4]).ok_or(invalid("encoder_mode", p[4]))?;
            DeviceMessage::Heartbeat {
                uptime_s: u32::from_le_bytes([p[0], p[1], p[2], p[3]]),
                encoder_mode,
            }
        }
        _ => return Err(DecodeError::UnknownType { msg_type: t }),
    };
    Ok(Frame { seq: h.seq, msg })
}

fn decode_hello(p: &[u8]) -> Result<Hello, DecodeError> {
    let t = msg::HELLO;
    if p.len() < HELLO_FIXED {
        return Err(DecodeError::PayloadSize {
            msg_type: t,
            expected: HELLO_FIXED,
            got: p.len(),
        });
    }
    let count = p[4];
    if usize::from(count) > MAX_CONTROLS {
        return Err(DecodeError::TooManyControls { count });
    }
    let encoder_mode = EncoderMode::from_byte(p[3]).ok_or(DecodeError::InvalidField {
        msg_type: t,
        field: "encoder_mode",
        value: p[3],
    })?;

    // Entries are 2 bytes, or 3 for an encoder with its push switch.
    let mut seen = [false; 256];
    let mut controls = Vec::with_capacity(usize::from(count));
    let mut at = HELLO_FIXED;
    for _ in 0..count {
        let short = |need: usize| DecodeError::PayloadSize {
            msg_type: t,
            expected: need,
            got: p.len(),
        };
        let &[id, kind] = p.get(at..at + 2).ok_or(short(at + 2))? else {
            unreachable!("a 2 byte slice");
        };
        let (kind, push) = match kind {
            KIND_ENCODER_WITH_PUSH => {
                let push = *p.get(at + 2).ok_or(short(at + 3))?;
                at += 3;
                (ControlKind::Encoder, Some(push))
            }
            k => {
                at += 2;
                let kind = ControlKind::from_byte(k).ok_or(DecodeError::InvalidField {
                    msg_type: t,
                    field: "kind",
                    value: k,
                })?;
                (kind, None)
            }
        };
        if std::mem::replace(&mut seen[usize::from(id)], true) {
            return Err(DecodeError::DuplicateControl { id });
        }
        controls.push(Control { id, kind, push });
    }
    expect_size(t, p, at)?;
    if let Some((encoder, push)) = bad_push(&controls) {
        return Err(DecodeError::BadPush { encoder, push });
    }
    Ok(Hello {
        firmware: FirmwareVersion {
            major: p[0],
            minor: p[1],
            patch: p[2],
        },
        encoder_mode,
        controls,
    })
}

/// The first encoder whose push switch isn't a button listed in the same
/// HELLO, or is another encoder's too, or a button that claims a push.
fn bad_push(controls: &[Control]) -> Option<(u8, u8)> {
    let mut claimed = [false; 256];
    for c in controls {
        let Some(push) = c.push else { continue };
        let is_button = controls
            .iter()
            .any(|b| b.id == push && b.kind == ControlKind::Button);
        if c.kind != ControlKind::Encoder
            || !is_button
            || std::mem::replace(&mut claimed[usize::from(push)], true)
        {
            return Some((c.id, push));
        }
    }
    None
}

/// HELLO's payload size for these controls.
fn hello_len(controls: &[Control]) -> usize {
    HELLO_FIXED
        + controls
            .iter()
            .map(|c| if c.push.is_some() { 3 } else { 2 })
            .sum::<usize>()
}

/// Encodes a device to app message. Refuses anything the decoder would reject.
pub fn encode_device(seq: u8, m: &DeviceMessage) -> Result<[u8; REPORT_LEN], EncodeError> {
    let report = match m {
        DeviceMessage::Hello(h) => {
            if h.controls.len() > MAX_CONTROLS {
                return Err(EncodeError::TooManyControls {
                    count: h.controls.len(),
                });
            }
            let len = hello_len(&h.controls);
            if len > MAX_PAYLOAD_LEN {
                return Err(EncodeError::HelloTooLong { len });
            }
            if let Some((encoder, push)) = bad_push(&h.controls) {
                return Err(EncodeError::BadPush { encoder, push });
            }
            let mut seen = [false; 256];
            let mut p = Vec::with_capacity(len);
            p.extend_from_slice(&[
                h.firmware.major,
                h.firmware.minor,
                h.firmware.patch,
                h.encoder_mode as u8,
                // Fits: checked against MAX_CONTROLS (27) above.
                h.controls.len() as u8,
            ]);
            for c in &h.controls {
                if std::mem::replace(&mut seen[usize::from(c.id)], true) {
                    return Err(EncodeError::DuplicateControl { id: c.id });
                }
                match c.push {
                    Some(push) => p.extend_from_slice(&[c.id, KIND_ENCODER_WITH_PUSH, push]),
                    None => p.extend_from_slice(&[c.id, c.kind as u8]),
                }
            }
            build_report(msg::HELLO, seq, &p)
        }
        DeviceMessage::Button { control, action } => {
            build_report(msg::BUTTON, seq, &[*control, *action as u8])
        }
        DeviceMessage::Encoder { control, delta } => {
            if *delta == 0 {
                return Err(EncodeError::ZeroDelta);
            }
            build_report(msg::ENCODER, seq, &[*control, delta.to_le_bytes()[0]])
        }
        DeviceMessage::MenuSelect { item } => build_report(msg::MENU_SELECT, seq, &[*item]),
        DeviceMessage::Heartbeat {
            uptime_s,
            encoder_mode,
        } => {
            let u = uptime_s.to_le_bytes();
            build_report(
                msg::HEARTBEAT,
                seq,
                &[u[0], u[1], u[2], u[3], *encoder_mode as u8],
            )
        }
    };
    Ok(report)
}
