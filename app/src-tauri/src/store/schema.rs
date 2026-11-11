//! Config schema v1: the shapes serde reads. Semantic checks live in
//! `validate.rs`. Field reference: `app/src-tauri/examples/config.md`.

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

/// Step used by `volume_up` and `volume_down` when none is given: one of the
/// 16 steps the volume keys use.
pub const DEFAULT_VOLUME_STEP: f32 = 1.0 / 16.0;
/// How long a `applescript` action may run when no timeout is given.
pub const DEFAULT_APPLESCRIPT_TIMEOUT_MS: u32 = 5000;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    /// Paired units: only these are opened (the allowlist).
    #[serde(default)]
    pub devices: Vec<DeviceEntry>,
    /// Units never to open or offer for pairing.
    #[serde(default)]
    pub ignored: Vec<UnitRef>,
    #[serde(default)]
    pub mappings: Vec<Mapping>,
    /// Presses are logged but run nothing (the tray's Pause). Kept here so it
    /// survives a relaunch (D-075).
    #[serde(default)]
    pub paused: bool,
}

/// A paired unit: VID + PID + serial identifies it (D-004).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceEntry {
    pub name: String,
    #[serde(deserialize_with = "usb_id", serialize_with = "usb_id_out")]
    pub vid: u16,
    #[serde(deserialize_with = "usb_id", serialize_with = "usb_id_out")]
    pub pid: u16,
    pub serial: String,
    /// Its app owned OLED menu items, in order (M6, D-086).
    #[serde(default)]
    pub menu: Vec<MenuItem>,
}

/// The most app owned menu items a device shows (firmware `kMaxAppItems`).
pub const MAX_MENU_ITEMS: usize = 8;

/// One app owned item on a device's OLED menu: picking it runs the action.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MenuItem {
    /// 1 to 16 printable ASCII characters (the OLED's font).
    pub label: String,
    pub action: Action,
}

/// A unit by identity alone, for the ignore list.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UnitRef {
    #[serde(deserialize_with = "usb_id", serialize_with = "usb_id_out")]
    pub vid: u16,
    #[serde(deserialize_with = "usb_id", serialize_with = "usb_id_out")]
    pub pid: u16,
    pub serial: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    /// The serial of a unit in `devices`.
    pub device: String,
    /// The control_id the device reports in HELLO.
    pub control: u8,
    pub trigger: Trigger,
    pub scope: Scope,
    pub action: Action,
}

/// What on the control fires the mapping (D-054).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// A button going down.
    Press,
    /// A button held 600 ms.
    LongPress,
    /// An encoder turned clockwise; runs once per detent.
    Clockwise,
    /// An encoder turned counterclockwise; runs once per detent.
    Counterclockwise,
}

/// Where a mapping applies: everywhere, or only when an app is frontmost.
/// JSON: `"everywhere"` or `{ "app": "com.example.App" }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Everywhere,
    App(String),
}

/// The fixed v1 action list (product.md), minus Open dashboard (D-055).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    OpenApp {
        bundle_id: String,
    },
    VolumeUp {
        #[serde(default = "default_step")]
        step: f32,
    },
    VolumeDown {
        #[serde(default = "default_step")]
        step: f32,
    },
    VolumeSet {
        level: f32,
    },
    MuteToggle,
    /// A key combo, or a short sequence, to the frontmost app (D-082).
    SendShortcut {
        keys: String,
    },
    #[serde(rename = "applescript")]
    AppleScript {
        source: String,
        #[serde(default = "default_timeout")]
        timeout_ms: u32,
    },
}

fn default_step() -> f32 {
    DEFAULT_VOLUME_STEP
}

fn default_timeout() -> u32 {
    DEFAULT_APPLESCRIPT_TIMEOUT_MS
}

/// Writes a USB ID the way people write them: `"0x303A"`.
fn usb_id_out<S: Serializer>(id: &u16, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&format!("0x{id:04X}"))
}

/// A USB ID: `"0x303A"` (any case) or a plain number.
fn usb_id<'de, D: Deserializer<'de>>(d: D) -> Result<u16, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Number(u64),
        Text(String),
    }
    let bad = || de::Error::custom("expected a USB ID like \"0x303A\" or a number from 0 to 65535");
    match Raw::deserialize(d).map_err(|_: D::Error| bad())? {
        Raw::Number(n) => u16::try_from(n).map_err(|_| bad()),
        Raw::Text(s) => {
            let hex = s
                .strip_prefix("0x")
                .or_else(|| s.strip_prefix("0X"))
                .ok_or_else(bad)?;
            u16::from_str_radix(hex, 16).map_err(|_| bad())
        }
    }
}
