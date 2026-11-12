//! USB identity. See "Identity" in `docs/protocol.md`, D-112 and D-113.
//!
//! A Detent unit is recognized by its vendor collection, a product string
//! starting [`PRODUCT_PREFIX`], a serial, and a valid HELLO, never by its USB
//! IDs (D-112). The IDs below are chosen locally under each chip vendor's VID
//! and only name the model (for the starter template) and keep boards apart
//! in system tools. No vendor allocation is requested until Detent has real
//! adoption; a collision with another product can't be mistaken for a unit,
//! since that product won't call itself Detent or answer with a HELLO.

/// Espressif's vendor ID (the ESP32-S3 boards).
pub const VID: u16 = 0x303A;

/// The XIAO ESP32-S3's product ID, chosen locally near the top of
/// 0x8000..=0x8FFF, far above Espressif's allocations when chosen (0x83EA).
/// Never TinyUSB or IDF defaults such as 0x4001.
pub const PID: u16 = 0x8FDE;
/// The Adafruit ESP32-S3 Feather's product ID (local, D-113).
pub const FEATHER_PID: u16 = 0x8FDF;
/// The ESP32-S3 DevKitC's product ID (local, D-113).
pub const DEVKITC_PID: u16 = 0x8FE0;

/// Raspberry Pi's vendor ID (the RP2040 Pico).
pub const PICO_VID: u16 = 0x2E8A;
/// The Pico's product ID, chosen locally far above Raspberry Pi's own
/// (bootloader 0x0003, SDK and MicroPython in the low hundreds).
pub const PICO_PID: u16 = 0xF0DE;

/// Every Detent product string starts with this: `Detent <model name>`.
pub const PRODUCT_PREFIX: &str = "Detent ";

/// A Detent model: a board build with its own USB IDs and product name
/// (D-113). Its controls come from HELLO (D-094), not from here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Model {
    pub vid: u16,
    pub pid: u16,
    /// The `<model name>` in the product string `Detent <model name>`.
    pub name: &'static str,
    /// The firmware's board flag, `-DDETENT_BOARD_<board>`: the generated
    /// header gives that build these IDs and this name.
    pub board: &'static str,
}

/// Every model Detent builds firmware for.
pub const MODELS: &[Model] = &[
    Model {
        vid: VID,
        pid: PID,
        name: "XIAO",
        board: "XIAO_ESP32S3",
    },
    Model {
        vid: VID,
        pid: FEATHER_PID,
        name: "Feather",
        board: "FEATHER_ESP32S3",
    },
    Model {
        vid: VID,
        pid: DEVKITC_PID,
        name: "DevKitC",
        board: "DEVKITC_ESP32S3",
    },
    Model {
        vid: PICO_VID,
        pid: PICO_PID,
        name: "Pico",
        board: "PICO",
    },
];

/// The model with these USB IDs, if Detent builds it.
pub fn model(vid: u16, pid: u16) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.vid == vid && m.pid == pid)
}

/// Whether a USB product string claims to be a Detent unit. Together with
/// the vendor collection and a serial, this is what makes a channel worth
/// opening; its HELLO confirms it.
pub fn is_detent_product(product: Option<&str>) -> bool {
    product.is_some_and(|p| p.len() > PRODUCT_PREFIX.len() && p.starts_with(PRODUCT_PREFIX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_model_is_found_by_its_ids_and_nothing_else() {
        assert_eq!(model(VID, PID).map(|m| m.name), Some("XIAO"));
        assert_eq!(model(VID, FEATHER_PID).map(|m| m.name), Some("Feather"));
        assert_eq!(model(VID, DEVKITC_PID).map(|m| m.name), Some("DevKitC"));
        assert_eq!(model(PICO_VID, PICO_PID).map(|m| m.name), Some("Pico"));
        assert!(model(VID, PICO_PID).is_none(), "VID and PID go together");
        assert!(
            model(0x05AC, 0x0256).is_none(),
            "an Apple keyboard isn't one"
        );
        for (i, a) in MODELS.iter().enumerate() {
            for b in &MODELS[i + 1..] {
                assert!((a.vid, a.pid) != (b.vid, b.pid), "{a:?} and {b:?}");
                assert_ne!(a.name, b.name);
                assert_ne!(a.board, b.board);
            }
        }
    }

    #[test]
    fn a_detent_is_known_by_its_product_string() {
        assert!(is_detent_product(Some("Detent XIAO")));
        assert!(is_detent_product(Some("Detent My Build")));
        assert!(
            is_detent_product(Some("Detent Dev")),
            "v2 firmware, to say it's outdated"
        );
        assert!(!is_detent_product(None));
        assert!(!is_detent_product(Some("Detent ")));
        assert!(!is_detent_product(Some("Detentive")));
        assert!(!is_detent_product(Some("USB Receiver")));
        assert!(
            !is_detent_product(Some("detent xiao")),
            "the firmware writes it exactly"
        );
    }
}
