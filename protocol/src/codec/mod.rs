//! Report codec. See `docs/protocol.md` for the wire format.
//!
//! Every function here takes or returns the 63 byte report that follows the
//! HID report ID; stripping and adding the ID is the transport's job.
//!
//! Decoding is strict (D-037): `len` must match the message exactly, enum
//! bytes must be known values, and bytes after the payload must be zero.
//! Nothing in this module panics on any input.

mod app;
mod device;

use std::fmt;

use crate::{MAX_PAYLOAD_LEN, PROTOCOL_VERSION, REPORT_LEN};

pub use app::{
    decode_app, encode_app, AppMessage, InjectEvent, Injected, Label, StatusText, TextError,
    MAX_INJECT_EVENTS, MAX_LABEL_LEN, MAX_STATUS_LEN,
};
pub use device::{
    decode_device, encode_device, ButtonAction, Control, ControlKind, DeviceMessage, EncoderMode,
    FirmwareVersion, Hello, MAX_CONTROLS,
};

/// Offset of the payload within a report.
const PAYLOAD_OFFSET: usize = 4;

/// A decoded report: its sequence number and message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame<M> {
    pub seq: u8,
    pub msg: M,
}

/// Why a report was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// The report isn't exactly [`REPORT_LEN`] bytes.
    ReportLength { got: usize },
    /// Byte 0 isn't [`PROTOCOL_VERSION`].
    Version { got: u8 },
    /// `len` is over [`MAX_PAYLOAD_LEN`].
    PayloadLength { len: u8 },
    /// A byte after the payload isn't zero.
    TrailingBytes,
    /// Not a known message type for this direction.
    UnknownType { msg_type: u8 },
    /// `len` doesn't match what the message needs.
    PayloadSize {
        msg_type: u8,
        expected: usize,
        got: usize,
    },
    /// A field holds a value the protocol doesn't define.
    InvalidField {
        msg_type: u8,
        field: &'static str,
        value: u8,
    },
    /// HELLO lists more than [`MAX_CONTROLS`] controls.
    TooManyControls { count: u8 },
    /// HELLO lists the same control ID twice.
    DuplicateControl { id: u8 },
    /// TEST_INJECT carries more than [`MAX_INJECT_EVENTS`] events.
    TooManyEvents { count: u8 },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReportLength { got } => {
                write!(f, "report is {got} bytes, expected {REPORT_LEN}")
            }
            Self::Version { got } => {
                write!(f, "protocol version {got}, expected {PROTOCOL_VERSION}")
            }
            Self::PayloadLength { len } => {
                write!(f, "len {len} is over the {MAX_PAYLOAD_LEN} byte maximum")
            }
            Self::TrailingBytes => write!(f, "nonzero bytes after the payload"),
            Self::UnknownType { msg_type } => write!(f, "unknown message type {msg_type:#04x}"),
            Self::PayloadSize {
                msg_type,
                expected,
                got,
            } => write!(
                f,
                "type {msg_type:#04x} needs a {expected} byte payload, got {got}"
            ),
            Self::InvalidField {
                msg_type,
                field,
                value,
            } => write!(f, "type {msg_type:#04x} has invalid {field} {value}"),
            Self::TooManyControls { count } => {
                write!(f, "HELLO lists {count} controls, max {MAX_CONTROLS}")
            }
            Self::DuplicateControl { id } => write!(f, "HELLO lists control {id} twice"),
            Self::TooManyEvents { count } => {
                write!(
                    f,
                    "TEST_INJECT carries {count} events, max {MAX_INJECT_EVENTS}"
                )
            }
        }
    }
}

impl std::error::Error for DecodeError {}

/// Why a message couldn't be encoded. Encoding never produces a report the
/// decoder would reject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeError {
    TooManyControls {
        count: usize,
    },
    DuplicateControl {
        id: u8,
    },
    ZeroDelta,
    /// MENU item with `index >= total` (including `total = 0`).
    MenuIndex {
        index: u8,
        total: u8,
    },
    /// TEST_INJECT with no events.
    NoEvents,
    TooManyEvents {
        count: usize,
    },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyControls { count } => {
                write!(f, "HELLO has {count} controls, max {MAX_CONTROLS}")
            }
            Self::DuplicateControl { id } => write!(f, "HELLO lists control {id} twice"),
            Self::ZeroDelta => write!(f, "encoder delta can't be zero"),
            Self::MenuIndex { index, total } => {
                write!(f, "menu index {index} isn't below total {total}")
            }
            Self::NoEvents => write!(f, "TEST_INJECT needs at least one event"),
            Self::TooManyEvents { count } => {
                write!(f, "TEST_INJECT has {count} events, max {MAX_INJECT_EVENTS}")
            }
        }
    }
}

impl std::error::Error for EncodeError {}

/// Header fields of a report that passed the generic checks.
struct Header<'a> {
    msg_type: u8,
    seq: u8,
    payload: &'a [u8],
}

/// Checks length, version, `len`, and zero padding. Message type and payload
/// contents are left to the per direction decoder.
fn parse_header(report: &[u8]) -> Result<Header<'_>, DecodeError> {
    if report.len() != REPORT_LEN {
        return Err(DecodeError::ReportLength { got: report.len() });
    }
    let (version, msg_type, seq, len) = (report[0], report[1], report[2], report[3]);
    if version != PROTOCOL_VERSION {
        return Err(DecodeError::Version { got: version });
    }
    if usize::from(len) > MAX_PAYLOAD_LEN {
        return Err(DecodeError::PayloadLength { len });
    }
    let end = PAYLOAD_OFFSET + usize::from(len);
    if report[end..].iter().any(|&b| b != 0) {
        return Err(DecodeError::TrailingBytes);
    }
    Ok(Header {
        msg_type,
        seq,
        payload: &report[PAYLOAD_OFFSET..end],
    })
}

/// Builds a report from a type, seq, and payload. Callers guarantee the
/// payload fits; every message is far below [`MAX_PAYLOAD_LEN`] or checked.
fn build_report(msg_type: u8, seq: u8, payload: &[u8]) -> [u8; REPORT_LEN] {
    debug_assert!(payload.len() <= MAX_PAYLOAD_LEN);
    let mut r = [0u8; REPORT_LEN];
    r[0] = PROTOCOL_VERSION;
    r[1] = msg_type;
    r[2] = seq;
    // Fits: payload.len() <= MAX_PAYLOAD_LEN (59).
    r[3] = payload.len() as u8;
    r[PAYLOAD_OFFSET..PAYLOAD_OFFSET + payload.len()].copy_from_slice(payload);
    r
}

/// Errors with [`DecodeError::PayloadSize`] unless the payload is exactly
/// `expected` bytes.
fn expect_size(msg_type: u8, payload: &[u8], expected: usize) -> Result<(), DecodeError> {
    if payload.len() == expected {
        Ok(())
    } else {
        Err(DecodeError::PayloadSize {
            msg_type,
            expected,
            got: payload.len(),
        })
    }
}
