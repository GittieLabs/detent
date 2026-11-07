# Protocol

`PROTOCOL_VERSION = 1` (draft; frozen at the end of M1)

Changes only with a version bump. The Rust crate in `protocol/` is the source of truth; `scripts/gen-header.sh` generates `firmware/include/detent_protocol.h` from it, and a test fails if they drift.

## Transport

One HID interface carries two top level collections (D-036):

1. **Vendor channel**, usage page `0xFF00`, usage `0x01`, report ID `6`. All app traffic. Every vendor report is the report ID plus exactly 63 bytes, in both directions. Unused bytes are zero.
2. **Consumer Control**, usage page `0x000C`, report ID `4`. Volume and mute pass through from the device (D-005). Not part of this protocol.

The report IDs are fixed by the Arduino-ESP32 core (`HID_REPORT_ID_VENDOR`, `HID_REPORT_ID_CONSUMER_CONTROL`); the firmware asserts they match.

On the host (hidapi):

- Open the `0xFF00` entry **non exclusive**. hidapi opens exclusively by default on macOS, which swallows Consumer Control and stops the volume keys.
- A read returns 64 bytes: `0x06`, then the 63 byte report. Consumer Control reports (`0x04`, 3 bytes) arrive on the same handle; drop anything whose first byte isn't `0x06`.
- A write is 64 bytes: `0x06`, then the 63 byte report. A write with any other report ID reports success but never reaches the device.
- Opening the vendor channel needs no macOS permission (Input Monitoring included), verified on macOS with a process that had never been granted it.

## Report layout

| Offset | Size | Field | Notes |
| --- | --- | --- | --- |
| 0 | 1 | `version` | `PROTOCOL_VERSION`. Receiver drops and logs any other value. |
| 1 | 1 | `type` | Message type below. Device to app `0x01..0x7F`, app to device `0x80..0xFF`. |
| 2 | 1 | `seq` | Per direction, wraps at 255. Lets Debug spot dropped reports. |
| 3 | 1 | `len` | Payload length in bytes, 0 to 59. |
| 4 | 59 | `payload` | Type specific, little endian. |

Offsets are within the 63 byte report, after the report ID. A report with `len > 59`, an unknown `type`, or the wrong `version` is rejected. Decoding is strict (D-037): `len` must equal the message's exact size, enum values must be defined, and unused bytes (after the payload, and inside fixed width text fields) must be zero. The codec must never panic on any input (property tested).

## Device to app

| Type | Name | Payload |
| --- | --- | --- |
| `0x01` | HELLO | `fw_major u8, fw_minor u8, fw_patch u8, encoder_mode u8, control_count u8`, then `control_count` pairs of `(control_id u8, kind u8)`. Kind: `1` button, `2` encoder. Max 27 controls. |
| `0x02` | BUTTON | `control_id u8, action u8`. Action: `1` down, `2` up, `3` long press (fires once at 600 ms held, before up). |
| `0x03` | ENCODER | `control_id u8, delta i8`. Sent only in Mapped mode. Positive is clockwise. Device may coalesce detents within 20 ms. |
| `0x04` | MENU_SELECT | `item_id u8`. An app owned OLED menu item was picked. |
| `0x05` | HEARTBEAT | `uptime_s u32, encoder_mode u8`. Every 2 s while the host has the device mounted, measured from the previous one. `uptime_s` is seconds since boot and restarts at 0 after about 49.7 days (the device's 32 bit millisecond clock). |

Encoder mode values: `0` Volume, `1` Menu, `2` Mapped.

Device sends HELLO when the host mounts it, on REQUEST_HELLO, and after leaving Setup. The mount HELLO usually goes out before any app has the channel open, so hosts send REQUEST_HELLO after opening. Seq numbers advance even when a send fails, so gaps are visible. Paired (from HELLO_ACK) resets when the host unmounts the device.

## App to device

| Type | Name | Payload |
| --- | --- | --- |
| `0x81` | HELLO_ACK | `paired u8` (`0` or `1`). Device shows pairing status on the OLED. |
| `0x82` | MENU | `total u8, index u8, item_id u8, label_len u8, label [u8; 16]`. One app owned item per report, index `0..total-1`. `total = 0` clears the app owned items, and then every other byte is zero. Otherwise `label_len` is 1 to 16 and the label is printable ASCII (`0x20..0x7E`), zero padded to 16 bytes. Payload is always 20 bytes. |
| `0x83` | STATUS | `text_len u8, text [u8; 20]`. Short status line for the OLED, for example the dashboard now showing. `text_len` 0 to 20 (0 clears the line), printable ASCII, zero padded to 20 bytes. Payload is always 21 bytes. |
| `0x84` | REQUEST_HELLO | Empty. |
| `0xF0` | TEST_INJECT | Debug firmware only. `count u8`, then `count` events of `(type u8, a u8, b u8, delay_ms u16)` where `type/a/b` are a BUTTON or ENCODER message's type and first two payload bytes. Device replays them through the real USB path. Release firmware ignores this type. Each event fires `delay_ms` after the previous one (the first, after the report arrives); a new TEST_INJECT replaces any script still running; replayed events use the same seq counter as real input. 1 to 11 events; each must be a valid BUTTON or ENCODER (defined action, nonzero delta). `delay_ms` is little endian. |

## Identity

| Field | Value |
| --- | --- |
| VID | `0x303A` (Espressif) |
| PID | Placeholder `0x8XXX` in `protocol/src/ids.rs` until the Espressif usb-pids PR merges (D-014). Never `0x4001` or other default TinyUSB PIDs. |
| Serial | 12 uppercase hex chars from the chip's base MAC (`esp_efuse_mac_get_default`), formatted by the firmware, not the core |
| Product string | `Detent <model name>` (placeholder `Detent Dev`). Manufacturer string `GittieLabs`. |
