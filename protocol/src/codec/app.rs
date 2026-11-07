//! App to device messages: HELLO_ACK, MENU, STATUS, REQUEST_HELLO, TEST_INJECT.

use std::fmt;

use super::device::ButtonAction;
use super::{build_report, expect_size, parse_header, DecodeError, EncodeError, Frame};
use crate::{msg, REPORT_LEN};

/// Longest OLED menu label.
pub const MAX_LABEL_LEN: usize = 16;
/// Longest OLED status line.
pub const MAX_STATUS_LEN: usize = 20;
/// Most events one TEST_INJECT carries: 1 + 5 per event must fit in 59 bytes.
pub const MAX_INJECT_EVENTS: usize = 11;

/// MENU payload: total, index, item, label_len, then a fixed 16 byte label.
const MENU_LEN: usize = 4 + MAX_LABEL_LEN;
/// STATUS payload: text_len, then a fixed 20 byte text field.
const STATUS_LEN: usize = 1 + MAX_STATUS_LEN;
/// One TEST_INJECT event: type, a, b, delay_ms (u16 LE).
const INJECT_EVENT_LEN: usize = 5;

/// Why a string can't be a [`Label`] or [`StatusText`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextError {
    Empty,
    TooLong {
        max: usize,
        got: usize,
    },
    /// Only `0x20..=0x7E` is allowed: the OLED font is ASCII.
    NotPrintableAscii,
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "text is empty"),
            Self::TooLong { max, got } => write!(f, "text is {got} bytes, max {max}"),
            Self::NotPrintableAscii => write!(f, "text must be printable ASCII"),
        }
    }
}

impl std::error::Error for TextError {}

fn is_printable(b: u8) -> bool {
    (0x20..=0x7E).contains(&b)
}

fn check_text(s: &str, min: usize, max: usize) -> Result<(), TextError> {
    if s.len() < min {
        return Err(TextError::Empty);
    }
    if s.len() > max {
        return Err(TextError::TooLong { max, got: s.len() });
    }
    if !s.bytes().all(is_printable) {
        return Err(TextError::NotPrintableAscii);
    }
    Ok(())
}

/// An OLED menu label: 1 to 16 printable ASCII characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label(String);

impl Label {
    pub fn new(s: &str) -> Result<Self, TextError> {
        check_text(s, 1, MAX_LABEL_LEN).map(|()| Self(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An OLED status line: 0 to 20 printable ASCII characters. Empty clears it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusText(String);

impl StatusText {
    pub fn new(s: &str) -> Result<Self, TextError> {
        check_text(s, 0, MAX_STATUS_LEN).map(|()| Self(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An input event the device replays through its real USB path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Injected {
    Button {
        control: u8,
        action: ButtonAction,
    },
    /// Never zero.
    Encoder {
        control: u8,
        delta: i8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InjectEvent {
    pub event: Injected,
    /// Wait before replaying this event.
    pub delay_ms: u16,
}

/// A message the app sends to the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMessage {
    HelloAck {
        paired: bool,
    },
    /// One app owned OLED menu item; `index < total`.
    MenuItem {
        total: u8,
        index: u8,
        item: u8,
        label: Label,
    },
    /// Removes every app owned OLED menu item (MENU with `total = 0`).
    MenuClear,
    Status {
        text: StatusText,
    },
    RequestHello,
    /// Debug firmware only. 1 to [`MAX_INJECT_EVENTS`] events.
    TestInject {
        events: Vec<InjectEvent>,
    },
}

/// Decodes a 63 byte app to device report.
pub fn decode_app(report: &[u8]) -> Result<Frame<AppMessage>, DecodeError> {
    let h = parse_header(report)?;
    let t = h.msg_type;
    let p = h.payload;
    let invalid = |field, value| DecodeError::InvalidField {
        msg_type: t,
        field,
        value,
    };

    let msg = match t {
        msg::HELLO_ACK => {
            expect_size(t, p, 1)?;
            match p[0] {
                0 => AppMessage::HelloAck { paired: false },
                1 => AppMessage::HelloAck { paired: true },
                v => return Err(invalid("paired", v)),
            }
        }
        msg::MENU => decode_menu(p)?,
        msg::STATUS => {
            expect_size(t, p, STATUS_LEN)?;
            let text = decode_text(t, "text_len", "text", p[0], &p[1..], 0)?;
            AppMessage::Status {
                text: StatusText(text),
            }
        }
        msg::REQUEST_HELLO => {
            expect_size(t, p, 0)?;
            AppMessage::RequestHello
        }
        msg::TEST_INJECT => decode_inject(p)?,
        _ => return Err(DecodeError::UnknownType { msg_type: t }),
    };
    Ok(Frame { seq: h.seq, msg })
}

/// Reads `len` bytes of printable ASCII from a fixed width `field` whose
/// remaining bytes must be zero.
fn decode_text(
    msg_type: u8,
    len_field: &'static str,
    text_field: &'static str,
    len: u8,
    field: &[u8],
    min: usize,
) -> Result<String, DecodeError> {
    let n = usize::from(len);
    if n < min || n > field.len() {
        return Err(DecodeError::InvalidField {
            msg_type,
            field: len_field,
            value: len,
        });
    }
    if field[n..].iter().any(|&b| b != 0) {
        return Err(DecodeError::TrailingBytes);
    }
    if let Some(&b) = field[..n].iter().find(|&&b| !is_printable(b)) {
        return Err(DecodeError::InvalidField {
            msg_type,
            field: text_field,
            value: b,
        });
    }
    // Printable ASCII is valid UTF-8.
    Ok(field[..n].iter().map(|&b| char::from(b)).collect())
}

fn decode_menu(p: &[u8]) -> Result<AppMessage, DecodeError> {
    let t = msg::MENU;
    expect_size(t, p, MENU_LEN)?;
    let (total, index, item, label_len) = (p[0], p[1], p[2], p[3]);
    let invalid = |field, value| DecodeError::InvalidField {
        msg_type: t,
        field,
        value,
    };
    if total == 0 {
        // A clear carries nothing else.
        if index != 0 {
            return Err(invalid("index", index));
        }
        if item != 0 {
            return Err(invalid("item", item));
        }
        if label_len != 0 {
            return Err(invalid("label_len", label_len));
        }
        if p[4..].iter().any(|&b| b != 0) {
            return Err(DecodeError::TrailingBytes);
        }
        return Ok(AppMessage::MenuClear);
    }
    if index >= total {
        return Err(invalid("index", index));
    }
    let label = decode_text(t, "label_len", "label", label_len, &p[4..], 1)?;
    Ok(AppMessage::MenuItem {
        total,
        index,
        item,
        label: Label(label),
    })
}

fn decode_inject(p: &[u8]) -> Result<AppMessage, DecodeError> {
    let t = msg::TEST_INJECT;
    let invalid = |field, value| DecodeError::InvalidField {
        msg_type: t,
        field,
        value,
    };
    if p.is_empty() {
        return Err(DecodeError::PayloadSize {
            msg_type: t,
            expected: 1,
            got: 0,
        });
    }
    let count = p[0];
    if usize::from(count) > MAX_INJECT_EVENTS {
        return Err(DecodeError::TooManyEvents { count });
    }
    if count == 0 {
        return Err(invalid("count", 0));
    }
    expect_size(t, p, 1 + INJECT_EVENT_LEN * usize::from(count))?;

    let mut events = Vec::with_capacity(usize::from(count));
    // Size checked above, so there is no remainder.
    let (chunks, _) = p[1..].as_chunks::<INJECT_EVENT_LEN>();
    for e in chunks {
        let &[kind, a, b, d0, d1] = e;
        let event = match kind {
            msg::BUTTON => Injected::Button {
                control: a,
                action: ButtonAction::from_byte(b).ok_or(invalid("action", b))?,
            },
            msg::ENCODER if b == 0 => return Err(invalid("delta", 0)),
            msg::ENCODER => Injected::Encoder {
                control: a,
                delta: i8::from_le_bytes([b]),
            },
            other => return Err(invalid("event_type", other)),
        };
        events.push(InjectEvent {
            event,
            delay_ms: u16::from_le_bytes([d0, d1]),
        });
    }
    Ok(AppMessage::TestInject { events })
}

/// Writes `text` into a zeroed fixed width field preceded by its length.
fn push_text(p: &mut Vec<u8>, text: &str, width: usize) {
    // Fits: Label and StatusText are at most 20 bytes.
    p.push(text.len() as u8);
    let start = p.len();
    p.resize(start + width, 0);
    p[start..start + text.len()].copy_from_slice(text.as_bytes());
}

/// Encodes an app to device message. Refuses anything the decoder would reject.
pub fn encode_app(seq: u8, m: &AppMessage) -> Result<[u8; REPORT_LEN], EncodeError> {
    let report = match m {
        AppMessage::HelloAck { paired } => build_report(msg::HELLO_ACK, seq, &[u8::from(*paired)]),
        AppMessage::MenuItem {
            total,
            index,
            item,
            label,
        } => {
            if index >= total {
                return Err(EncodeError::MenuIndex {
                    index: *index,
                    total: *total,
                });
            }
            let mut p = vec![*total, *index, *item];
            push_text(&mut p, label.as_str(), MAX_LABEL_LEN);
            build_report(msg::MENU, seq, &p)
        }
        AppMessage::MenuClear => build_report(msg::MENU, seq, &[0; MENU_LEN]),
        AppMessage::Status { text } => {
            let mut p = Vec::with_capacity(STATUS_LEN);
            push_text(&mut p, text.as_str(), MAX_STATUS_LEN);
            build_report(msg::STATUS, seq, &p)
        }
        AppMessage::RequestHello => build_report(msg::REQUEST_HELLO, seq, &[]),
        AppMessage::TestInject { events } => {
            if events.is_empty() {
                return Err(EncodeError::NoEvents);
            }
            if events.len() > MAX_INJECT_EVENTS {
                return Err(EncodeError::TooManyEvents {
                    count: events.len(),
                });
            }
            // Fits: at most MAX_INJECT_EVENTS (11).
            let mut p = vec![events.len() as u8];
            for e in events {
                let (kind, a, b) = match e.event {
                    Injected::Button { control, action } => (msg::BUTTON, control, action as u8),
                    Injected::Encoder { delta: 0, .. } => return Err(EncodeError::ZeroDelta),
                    Injected::Encoder { control, delta } => {
                        (msg::ENCODER, control, delta.to_le_bytes()[0])
                    }
                };
                let d = e.delay_ms.to_le_bytes();
                p.extend_from_slice(&[kind, a, b, d[0], d[1]]);
            }
            build_report(msg::TEST_INJECT, seq, &p)
        }
    };
    Ok(report)
}
