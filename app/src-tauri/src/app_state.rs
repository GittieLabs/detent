//! What the UI shows, and the config edits behind its commands. Pure: built
//! from the config and the engine's records, no Tauri and no hardware, so the
//! screens' logic is tested here (D-065).

use std::fmt;

use detent_protocol::codec::{ControlKind, Hello};
use serde::Serialize;

use crate::devices::{DeviceEvent, DeviceId};
use crate::engine::Record;
use crate::store::{Config, DeviceEntry, UnitRef};

/// A paired unit as the Devices screen shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PairedUnit {
    pub name: String,
    pub vid: u16,
    pub pid: u16,
    pub serial: String,
    pub connected: bool,
    /// The model from the USB product string, once seen this session.
    pub model: Option<String>,
    /// `major.minor.patch` from the last HELLO this session.
    pub firmware: Option<String>,
    pub buttons: usize,
    pub encoders: usize,
}

/// An unpaired unit the wizard has open read only (D-066).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Candidate {
    pub vid: u16,
    pub pid: u16,
    pub serial: String,
    pub model: Option<String>,
    pub firmware: Option<String>,
    pub buttons: usize,
    pub encoders: usize,
    /// The user pressed a button on this one most recently.
    pub picked: bool,
    /// The wizard's prefilled name: from the model, never a paired unit's.
    pub suggested_name: String,
}

/// An attached Detent unit that's neither paired nor ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnknownUnit {
    pub vid: u16,
    pub pid: u16,
    pub serial: Option<String>,
}

/// Everything the settings window and the tray show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppState {
    pub paired: Vec<PairedUnit>,
    pub candidates: Vec<Candidate>,
    pub unknown: Vec<UnknownUnit>,
    pub ignored: Vec<UnitRef>,
    /// The pairing wizard is open.
    pub identifying: bool,
    /// Why Detent can't run normally (no config loaded, USB unavailable).
    pub problem: Option<String>,
}

/// `Detent Dev` -> `Dev`; other product strings as they are.
fn model_of(product: &Option<String>) -> Option<String> {
    product
        .as_deref()
        .map(|p| p.strip_prefix("Detent ").unwrap_or(p).to_owned())
}

fn firmware_of(h: &Hello) -> String {
    let f = h.firmware;
    format!("{}.{}.{}", f.major, f.minor, f.patch)
}

fn counts(h: &Hello) -> (usize, usize) {
    let buttons = h
        .controls
        .iter()
        .filter(|c| c.kind == ControlKind::Button)
        .count();
    (buttons, h.controls.len() - buttons)
}

fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// `Detent Dev` for model `Dev` (`Detent` with no model), with ` 2`, ` 3`, ...
/// added while a paired unit already has that name.
fn suggested_name(model: &Option<String>, paired: &[PairedUnit]) -> String {
    let base = match model {
        Some(m) => format!("Detent {m}"),
        None => "Detent".to_owned(),
    };
    let taken = |name: &str| paired.iter().any(|u| same(&u.name, name));
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|name| !taken(name))
        .expect("some suffix is free")
}

impl AppState {
    pub fn new(config: &Config) -> Self {
        let mut s = Self {
            paired: Vec::new(),
            candidates: Vec::new(),
            unknown: Vec::new(),
            ignored: Vec::new(),
            identifying: false,
            problem: None,
        };
        s.set_config(config);
        s
    }

    /// Puts a new config's paired and ignored lists in place, keeping what's
    /// known live about units still paired (connected, model, firmware).
    pub fn set_config(&mut self, config: &Config) {
        let old = std::mem::take(&mut self.paired);
        self.paired = config
            .devices
            .iter()
            .map(|d| {
                let live = old.iter().find(|u| same(&u.serial, &d.serial));
                PairedUnit {
                    name: d.name.clone(),
                    vid: d.vid,
                    pid: d.pid,
                    serial: d.serial.clone(),
                    connected: live.is_some_and(|u| u.connected),
                    model: live.and_then(|u| u.model.clone()),
                    firmware: live.and_then(|u| u.firmware.clone()),
                    buttons: live.map_or(0, |u| u.buttons),
                    encoders: live.map_or(0, |u| u.encoders),
                }
            })
            .collect();
        self.ignored = config.ignored.clone();
        let listed = |serial: &Option<String>| {
            serial.as_deref().is_some_and(|s| {
                config.devices.iter().any(|d| same(&d.serial, s))
                    || config.ignored.iter().any(|u| same(&u.serial, s))
            })
        };
        self.unknown.retain(|u| !listed(&u.serial));
        self.candidates.retain(|c| !listed(&Some(c.serial.clone())));
        for c in &mut self.candidates {
            c.suggested_name = suggested_name(&c.model, &self.paired);
        }
    }

    /// Opens or closes the wizard's identification. Closing clears candidates.
    pub fn set_identifying(&mut self, on: bool) {
        self.identifying = on;
        if !on {
            self.candidates.clear();
        }
    }

    fn paired_mut(&mut self, id: &DeviceId) -> Option<&mut PairedUnit> {
        self.paired
            .iter_mut()
            .find(|u| u.vid == id.vid && u.pid == id.pid && same(&u.serial, &id.serial))
    }

    fn candidate_mut(&mut self, id: &DeviceId) -> Option<&mut Candidate> {
        self.candidates
            .iter_mut()
            .find(|c| c.vid == id.vid && c.pid == id.pid && same(&c.serial, &id.serial))
    }

    /// Folds in one engine record. Returns whether anything the UI shows
    /// changed, so the app only pushes real changes.
    pub fn apply(&mut self, record: &Record) -> bool {
        let Record::Device(event) = record else {
            return false;
        };
        let before = self.clone();
        match event {
            DeviceEvent::Connected { id, product } => {
                if let Some(u) = self.paired_mut(id) {
                    u.connected = true;
                    if product.is_some() {
                        u.model = model_of(product);
                    }
                }
                self.unknown
                    .retain(|u| u.serial.as_deref().is_none_or(|s| !same(s, &id.serial)));
            }
            DeviceEvent::Hello { id, hello } => {
                let (buttons, encoders) = counts(hello);
                if let Some(u) = self.paired_mut(id) {
                    u.firmware = Some(firmware_of(hello));
                    (u.buttons, u.encoders) = (buttons, encoders);
                }
            }
            DeviceEvent::Disconnected { id, .. } => {
                if let Some(u) = self.paired_mut(id) {
                    u.connected = false;
                }
            }
            DeviceEvent::Unknown { vid, pid, serial } => {
                let unit = UnknownUnit {
                    vid: *vid,
                    pid: *pid,
                    serial: serial.clone(),
                };
                if !self.unknown.contains(&unit) {
                    self.unknown.push(unit);
                }
            }
            DeviceEvent::UnknownGone { vid, pid, serial } => {
                self.unknown
                    .retain(|u| !(u.vid == *vid && u.pid == *pid && u.serial == *serial));
            }
            DeviceEvent::CandidateFound { id, product } => {
                if self.identifying && self.candidate_mut(id).is_none() {
                    let model = model_of(product);
                    self.candidates.push(Candidate {
                        suggested_name: suggested_name(&model, &self.paired),
                        vid: id.vid,
                        pid: id.pid,
                        serial: id.serial.clone(),
                        model,
                        firmware: None,
                        buttons: 0,
                        encoders: 0,
                        picked: false,
                    });
                }
            }
            DeviceEvent::CandidateHello { id, hello } => {
                let (buttons, encoders) = counts(hello);
                if let Some(c) = self.candidate_mut(id) {
                    c.firmware = Some(firmware_of(hello));
                    (c.buttons, c.encoders) = (buttons, encoders);
                }
            }
            DeviceEvent::CandidatePressed(id) => {
                if self.candidate_mut(id).is_some() {
                    for c in &mut self.candidates {
                        c.picked =
                            c.vid == id.vid && c.pid == id.pid && same(&c.serial, &id.serial);
                    }
                }
            }
            DeviceEvent::CandidateGone(id) => {
                self.candidates.retain(|c| {
                    !(c.vid == id.vid && c.pid == id.pid && same(&c.serial, &id.serial))
                });
            }
            DeviceEvent::Input { .. }
            | DeviceEvent::Rejected { .. }
            | DeviceEvent::OpenFailed { .. } => {}
        }
        *self != before
    }
}

/// Why a config edit was refused. Shown to the user as is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    EmptyName,
    AlreadyPaired(String),
    NotPaired(String),
    AlreadyIgnored(String),
    NotIgnored(String),
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(f, "the name can't be empty"),
            Self::AlreadyPaired(s) => write!(f, "{s} is already paired"),
            Self::NotPaired(s) => write!(f, "{s} isn't paired"),
            Self::AlreadyIgnored(s) => write!(f, "{s} is already ignored"),
            Self::NotIgnored(s) => write!(f, "{s} isn't ignored"),
        }
    }
}

impl std::error::Error for EditError {}

fn name_of(name: &str) -> Result<String, EditError> {
    let name = name.trim();
    if name.is_empty() {
        Err(EditError::EmptyName)
    } else {
        Ok(name.to_owned())
    }
}

fn is_paired(config: &Config, serial: &str) -> bool {
    config.devices.iter().any(|d| same(&d.serial, serial))
}

/// Pairs a unit under `name` (trimmed), taking it off the ignore list.
pub fn pair(
    config: &Config,
    vid: u16,
    pid: u16,
    serial: &str,
    name: &str,
) -> Result<Config, EditError> {
    let name = name_of(name)?;
    if is_paired(config, serial) {
        return Err(EditError::AlreadyPaired(serial.to_owned()));
    }
    let mut c = config.clone();
    c.ignored.retain(|u| !same(&u.serial, serial));
    c.devices.push(DeviceEntry {
        name,
        vid,
        pid,
        serial: serial.to_owned(),
    });
    Ok(c)
}

/// Renames a paired unit.
pub fn rename(config: &Config, serial: &str, name: &str) -> Result<Config, EditError> {
    let name = name_of(name)?;
    let mut c = config.clone();
    let d = c
        .devices
        .iter_mut()
        .find(|d| same(&d.serial, serial))
        .ok_or_else(|| EditError::NotPaired(serial.to_owned()))?;
    d.name = name;
    Ok(c)
}

/// How many mappings name this unit (what unpairing would remove).
pub fn mappings_of(config: &Config, serial: &str) -> usize {
    config
        .mappings
        .iter()
        .filter(|m| same(&m.device, serial))
        .count()
}

/// Unpairs a unit and removes its mappings, which can't name a unit that
/// isn't paired (D-070). Returns the new config and how many were removed.
pub fn unpair(config: &Config, serial: &str) -> Result<(Config, usize), EditError> {
    if !is_paired(config, serial) {
        return Err(EditError::NotPaired(serial.to_owned()));
    }
    let removed = mappings_of(config, serial);
    let mut c = config.clone();
    c.devices.retain(|d| !same(&d.serial, serial));
    c.mappings.retain(|m| !same(&m.device, serial));
    Ok((c, removed))
}

/// Puts an unpaired unit on the ignore list. A paired unit must be unpaired
/// first, so ignoring never silently removes mappings.
pub fn ignore(config: &Config, vid: u16, pid: u16, serial: &str) -> Result<Config, EditError> {
    if is_paired(config, serial) {
        return Err(EditError::AlreadyPaired(serial.to_owned()));
    }
    if config.ignored.iter().any(|u| same(&u.serial, serial)) {
        return Err(EditError::AlreadyIgnored(serial.to_owned()));
    }
    let mut c = config.clone();
    c.ignored.push(UnitRef {
        vid,
        pid,
        serial: serial.to_owned(),
    });
    Ok(c)
}

/// Takes a unit off the ignore list.
pub fn unignore(config: &Config, serial: &str) -> Result<Config, EditError> {
    if !config.ignored.iter().any(|u| same(&u.serial, serial)) {
        return Err(EditError::NotIgnored(serial.to_owned()));
    }
    let mut c = config.clone();
    c.ignored.retain(|u| !same(&u.serial, serial));
    Ok(c)
}
