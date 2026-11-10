//! What the UI shows, and the config edits behind its commands. Pure: built
//! from the config and the engine's records, no Tauri and no hardware, so the
//! screens' logic is tested here (D-065).

use std::fmt;

use detent_protocol::codec::{ControlKind, EncoderMode, Hello};
use serde::Serialize;

use crate::devices::{DeviceEvent, DeviceId};
use crate::engine::{describe, Record};
use crate::models;
use crate::store::{Action, ArgError, Config, DeviceEntry, Mapping, Scope, Trigger, UnitRef};

/// Button or encoder, as the drawing shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlType {
    Button,
    Encoder,
}

/// One control of a unit, from its HELLO, as the drawing shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ControlView {
    pub id: u8,
    #[serde(rename = "type")]
    pub kind: ControlType,
    /// `Button 1`, `Encoder`, `Encoder push`.
    pub label: String,
    /// The device handles it itself (the encoder and its push switch in
    /// Volume and Menu modes), so it can't be mapped.
    pub handled_by_device: bool,
}

/// A unit's controls from its HELLO, in HELLO order.
pub fn controls_of(pid: u16, hello: &Hello) -> Vec<ControlView> {
    let push = models::encoder_push(pid);
    let device_mode = hello.encoder_mode != EncoderMode::Mapped;
    hello
        .controls
        .iter()
        .map(|c| {
            let (kind, label, encoderish) = match c.kind {
                ControlKind::Encoder => (ControlType::Encoder, "Encoder".to_owned(), true),
                ControlKind::Button if Some(c.id) == push => {
                    (ControlType::Button, "Encoder push".to_owned(), true)
                }
                ControlKind::Button => (ControlType::Button, format!("Button {}", c.id), false),
            };
            ControlView {
                id: c.id,
                kind,
                label,
                handled_by_device: encoderish && device_mode,
            }
        })
        .collect()
}

/// One resolved input, for the Test screen's live view (sent as the `input`
/// event, not folded into [`AppState`]).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InputView {
    pub serial: String,
    pub control: u8,
    pub trigger: Trigger,
    pub frontmost: Option<String>,
    /// The log's short form of the action that applies, if any.
    pub action: Option<String>,
    /// The action itself, for the UI to describe in its own words.
    pub mapping: Option<Action>,
    pub count: u8,
    pub paused: bool,
    pub dry_run: bool,
}

/// An action's result, for the Test screen (sent as the `result` event).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResultView {
    pub serial: String,
    /// The log's short form of the action.
    pub action: String,
    pub mapping: Action,
    /// Something actually happened (not a dry run, not a failure).
    pub ran: bool,
    pub ok: bool,
    /// "was 13%, now 38%", "opened", "not run (dry run)", "failed: …".
    pub outcome: String,
    pub took_ms: f64,
}

/// What the volume overlay shows (D-083).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hud {
    /// The new level, 0 to 100.
    Level(u8),
    Muted(bool),
}

/// The overlay for a volume action that ran; `None` for a dry run, a
/// failure, or any other record.
pub fn hud_of(record: &Record) -> Option<Hud> {
    use crate::actions::Done;
    match record {
        Record::Ran {
            outcome: Ok(Done::Volume { to, .. }),
            ..
        } => Some(Hud::Level((to.clamp(0.0, 1.0) * 100.0).round() as u8)),
        Record::Ran {
            outcome: Ok(Done::Muted(m)),
            ..
        } => Some(Hud::Muted(*m)),
        _ => None,
    }
}

/// The Test screen's view of a [`Record::Ran`]; `None` for other records.
pub fn result_view(record: &Record) -> Option<ResultView> {
    use crate::actions::Done;
    let Record::Ran {
        id,
        action,
        outcome,
        took,
        ..
    } = record
    else {
        return None;
    };
    let pct = |v: f32| (v * 100.0).round() as i32;
    let (ran, ok, text) = match outcome {
        Ok(Done::Recorded) => (false, true, "not run (dry run)".to_owned()),
        Ok(Done::Volume { from, to }) => (
            true,
            true,
            format!("was {}%, now {}%", pct(*from), pct(*to)),
        ),
        Ok(Done::Opened(_)) => (true, true, "opened".to_owned()),
        Ok(done) => (true, true, done.to_string()),
        Err(e) => (false, false, format!("failed: {e}")),
    };
    Some(ResultView {
        serial: id.serial.clone(),
        action: describe(action),
        mapping: action.clone(),
        ran,
        ok,
        outcome: text,
        took_ms: took.as_secs_f64() * 1000.0,
    })
}

/// The live view of a [`Record::Resolved`]; `None` for other records.
pub fn input_view(record: &Record) -> Option<InputView> {
    let Record::Resolved {
        id,
        control,
        trigger,
        frontmost,
        action,
        count,
        paused,
        dry_run,
    } = record
    else {
        return None;
    };
    Some(InputView {
        serial: id.serial.clone(),
        control: *control,
        trigger: *trigger,
        frontmost: frontmost.clone(),
        action: action.as_ref().map(describe),
        mapping: action.clone(),
        count: *count,
        paused: *paused,
        dry_run: *dry_run,
    })
}

/// A paired unit as the Devices screen shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
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
    /// From its last HELLO this session; empty until then.
    pub controls: Vec<ControlView>,
    /// Its mappings, from the config.
    pub mappings: Vec<Mapping>,
    /// What its model's starter template would still add (D-077).
    pub template: Vec<Mapping>,
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

/// The Accessibility grant as the UI needs it (M5): shortcuts need it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Accessibility {
    /// Detent holds the grant (rechecked every couple of seconds).
    pub granted: bool,
    /// Some mapping sends a shortcut.
    pub needed: bool,
    /// Show the banner with its Fix button: needed and not granted.
    banner: bool,
}

impl Accessibility {
    pub fn banner(&self) -> bool {
        self.banner
    }

    fn refresh(&mut self) {
        self.banner = self.needed && !self.granted;
    }
}

/// Everything the settings window and the tray show.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppState {
    pub paired: Vec<PairedUnit>,
    pub candidates: Vec<Candidate>,
    pub unknown: Vec<UnknownUnit>,
    pub ignored: Vec<UnitRef>,
    /// The pairing wizard is open.
    pub identifying: bool,
    /// Presses run nothing (the tray's Pause).
    pub paused: bool,
    /// Actions are recorded, not run (the Test screen, D-079).
    pub dry_run: bool,
    pub accessibility: Accessibility,
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
            paused: false,
            dry_run: false,
            accessibility: Accessibility::default(),
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
                    controls: live.map(|u| u.controls.clone()).unwrap_or_default(),
                    mappings: config
                        .mappings
                        .iter()
                        .filter(|m| same(&m.device, &d.serial))
                        .cloned()
                        .collect(),
                    template: template_left(config, d),
                }
            })
            .collect();
        self.ignored = config.ignored.clone();
        self.paused = config.paused;
        self.accessibility.needed = config
            .mappings
            .iter()
            .any(|m| matches!(m.action, Action::SendShortcut { .. }));
        self.accessibility.refresh();
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

    /// Records whether Detent holds the Accessibility grant. Returns whether
    /// that changed, so the app only pushes real changes.
    pub fn set_accessibility(&mut self, granted: bool) -> bool {
        let before = self.accessibility;
        self.accessibility.granted = granted;
        self.accessibility.refresh();
        self.accessibility != before
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
        if let Record::DryRun { on } = record {
            let changed = self.dry_run != *on;
            self.dry_run = *on;
            return changed;
        }
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
                    u.controls = controls_of(id.pid, hello);
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
            DeviceEvent::Mode { id, mode } => {
                let pid = id.pid;
                if let Some(u) = self.paired_mut(id) {
                    let push = models::encoder_push(pid);
                    let device_mode = *mode != EncoderMode::Mapped;
                    for c in &mut u.controls {
                        let encoderish = c.kind == ControlType::Encoder || Some(c.id) == push;
                        if encoderish {
                            c.handled_by_device = device_mode;
                        }
                    }
                }
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
    /// The unit doesn't report this control (or hasn't connected yet).
    NoSuchControl(u8),
    /// The device handles this control itself in its current mode.
    HandledByDevice(u8),
    /// A button takes press or long press; an encoder, a direction.
    WrongTrigger(u8),
    /// An argument the config loader would refuse.
    Invalid(ArgError),
    /// No mapping with that control, trigger and scope.
    NotMapped,
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(f, "the name can't be empty"),
            Self::AlreadyPaired(s) => write!(f, "{s} is already paired"),
            Self::NotPaired(s) => write!(f, "{s} isn't paired"),
            Self::AlreadyIgnored(s) => write!(f, "{s} is already ignored"),
            Self::NoSuchControl(c) => write!(f, "this device has no control {c}"),
            Self::HandledByDevice(c) => write!(
                f,
                "control {c} is handled by the device in its current encoder mode"
            ),
            Self::WrongTrigger(c) => write!(f, "control {c} can't fire on that trigger"),
            Self::Invalid(e) => write!(f, "{e}"),
            Self::NotMapped => write!(f, "there's no such mapping"),
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

/// Adds a mapping, or replaces the one with the same device, control,
/// trigger and scope in place. `controls` are the unit's, from its HELLO.
pub fn set_mapping(
    config: &Config,
    controls: &[ControlView],
    mapping: Mapping,
) -> Result<Config, EditError> {
    if !is_paired(config, &mapping.device) {
        return Err(EditError::NotPaired(mapping.device));
    }
    let control = controls
        .iter()
        .find(|c| c.id == mapping.control)
        .ok_or(EditError::NoSuchControl(mapping.control))?;
    if control.handled_by_device {
        return Err(EditError::HandledByDevice(mapping.control));
    }
    let fits = match control.kind {
        ControlType::Button => matches!(mapping.trigger, Trigger::Press | Trigger::LongPress),
        ControlType::Encoder => matches!(
            mapping.trigger,
            Trigger::Clockwise | Trigger::Counterclockwise
        ),
    };
    if !fits {
        return Err(EditError::WrongTrigger(mapping.control));
    }
    mapping.scope.check().map_err(EditError::Invalid)?;
    mapping.action.check_args().map_err(EditError::Invalid)?;
    let mut c = config.clone();
    let slot = c.mappings.iter_mut().find(|m| {
        same(&m.device, &mapping.device)
            && m.control == mapping.control
            && m.trigger == mapping.trigger
            && m.scope == mapping.scope
    });
    match slot {
        Some(m) => *m = mapping,
        None => c.mappings.push(mapping),
    }
    Ok(c)
}

/// The template mappings for `unit` that it doesn't have yet: none of its
/// own mappings shares their control, trigger and scope.
fn template_left(config: &Config, unit: &DeviceEntry) -> Vec<Mapping> {
    models::starter_template(unit.pid)
        .into_iter()
        .map(|m| Mapping {
            device: unit.serial.clone(),
            ..m
        })
        .filter(|t| {
            !config.mappings.iter().any(|m| {
                same(&m.device, &unit.serial)
                    && m.control == t.control
                    && m.trigger == t.trigger
                    && m.scope == t.scope
            })
        })
        .collect()
}

/// Adds the unit's starter template (D-077), never replacing a mapping it
/// already has. Applying it twice adds nothing.
pub fn apply_template(config: &Config, serial: &str) -> Result<Config, EditError> {
    let unit = config
        .devices
        .iter()
        .find(|d| same(&d.serial, serial))
        .ok_or_else(|| EditError::NotPaired(serial.to_owned()))?;
    let left = template_left(config, unit);
    let mut c = config.clone();
    c.mappings.extend(left);
    Ok(c)
}

/// Removes the mapping with exactly this device, control, trigger and scope.
pub fn remove_mapping(
    config: &Config,
    serial: &str,
    control: u8,
    trigger: Trigger,
    scope: &Scope,
) -> Result<Config, EditError> {
    let mut c = config.clone();
    let before = c.mappings.len();
    c.mappings.retain(|m| {
        !(same(&m.device, serial)
            && m.control == control
            && m.trigger == trigger
            && m.scope == *scope)
    });
    if c.mappings.len() == before {
        return Err(EditError::NotMapped);
    }
    Ok(c)
}

/// Turns Pause on or off.
pub fn set_paused(config: &Config, on: bool) -> Config {
    Config {
        paused: on,
        ..config.clone()
    }
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
