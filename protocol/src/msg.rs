//! Message types, byte 1 of every report.
//!
//! Device to app types are `0x01..=0x7F`; app to device types are `0x80..=0xFF`.

// Device to app.
pub const HELLO: u8 = 0x01;
pub const BUTTON: u8 = 0x02;
pub const ENCODER: u8 = 0x03;
pub const MENU_SELECT: u8 = 0x04;
pub const HEARTBEAT: u8 = 0x05;

// App to device.
pub const HELLO_ACK: u8 = 0x81;
pub const MENU: u8 = 0x82;
pub const STATUS: u8 = 0x83;
pub const REQUEST_HELLO: u8 = 0x84;
/// Debug firmware only; release firmware ignores it.
pub const TEST_INJECT: u8 = 0xF0;

/// Every message type with its name, in `docs/protocol.md` order. Used by
/// header generation and Debug output.
pub const ALL: [(u8, &str); 10] = [
    (HELLO, "HELLO"),
    (BUTTON, "BUTTON"),
    (ENCODER, "ENCODER"),
    (MENU_SELECT, "MENU_SELECT"),
    (HEARTBEAT, "HEARTBEAT"),
    (HELLO_ACK, "HELLO_ACK"),
    (MENU, "MENU"),
    (STATUS, "STATUS"),
    (REQUEST_HELLO, "REQUEST_HELLO"),
    (TEST_INJECT, "TEST_INJECT"),
];

/// Whether `t` is in the device to app range. Says nothing about whether the
/// type is defined; use [`name`] for that.
pub const fn is_device_to_app(t: u8) -> bool {
    matches!(t, 0x01..=0x7F)
}

/// Whether `t` is in the app to device range.
pub const fn is_app_to_device(t: u8) -> bool {
    t >= 0x80
}

/// Name of a defined message type, or `None` for an unknown one.
pub fn name(t: u8) -> Option<&'static str> {
    ALL.iter().find(|(id, _)| *id == t).map(|(_, n)| *n)
}
