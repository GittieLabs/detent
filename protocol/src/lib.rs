//! Detent USB wire format. See `docs/protocol.md`.
//!
//! This crate is the source of truth: `scripts/gen-header.sh` generates
//! `firmware/include/detent_protocol.h` from it. Changing anything here means
//! updating `docs/protocol.md`, bumping [`PROTOCOL_VERSION`], and regenerating
//! the header, all in one commit.
//!
//! Constants only for now; the codec lands in M1.

pub mod ids;
pub mod msg;

/// Wire format version, byte 0 of every report. Receivers drop any other value.
pub const PROTOCOL_VERSION: u8 = 1;

/// Every report is exactly this many bytes, in both directions.
pub const REPORT_LEN: usize = 64;

/// `version`, `type`, `seq`, `len`.
pub const HEADER_LEN: usize = 4;

/// Largest legal `len` value.
pub const MAX_PAYLOAD_LEN: usize = REPORT_LEN - HEADER_LEN;

/// Vendor defined HID usage page of the app traffic interface.
pub const USAGE_PAGE: u16 = 0xFF00;

/// HID usage of the app traffic interface.
pub const USAGE: u16 = 0x01;
