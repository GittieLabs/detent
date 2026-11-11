//! Devices: finds attached units, opens the paired ones, runs the HELLO
//! handshake, decodes their reports, and notices when they go away.
//!
//! [`Devices::step`] is one turn of the engine's loop, with the time passed
//! in so timeouts are testable. Each step rescans the device list if a second
//! has passed, reads every open unit (waiting at most `read_wait` in all),
//! then drops any unit silent for 6 s. Only allowlisted units are ever opened;
//! the ignore list wins over the allowlist; other Detent units are reported
//! once per attach and never read (D-062).

use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::fmt;
use std::thread;
use std::time::{Duration, Instant};

use detent_protocol::codec::{
    decode_device, encode_app, AppMessage, ButtonAction, DecodeError, DeviceMessage, EncoderMode,
    Hello, Label, StatusText,
};
use detent_protocol::ids;

use crate::store::{DeviceEntry, UnitRef};
use crate::transport::{Bus, Channel, DeviceTransport, TransportError};

/// A unit's identity: VID, PID, and serial together (D-004). Stable across
/// replugs and reboots.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeviceId {
    pub vid: u16,
    pub pid: u16,
    pub serial: String,
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04X}:{:04X} {}", self.vid, self.pid, self.serial)
    }
}

/// Why a unit stopped being connected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lost {
    /// A read or write failed: unplugged, or the OS closed it.
    Link(TransportError),
    /// It left the device list.
    Detached,
    /// No valid report for [`Options::heartbeat_timeout`].
    HeartbeatTimeout,
    /// The Mac slept; handles are reopened rather than trusted (D-068).
    Slept,
    /// Taken off the allowlist, or put on the ignore list.
    Unpaired,
}

/// What happened, for the engine and its log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceEvent {
    /// A paired unit's encoder mode changed since its last HELLO or
    /// heartbeat (the user switched it on the device's menu).
    Mode {
        id: DeviceId,
        mode: EncoderMode,
    },
    /// A paired unit was opened and asked for its HELLO. `product` is its
    /// USB product string (`Detent <model name>`), if it reports one.
    Connected {
        id: DeviceId,
        product: Option<String>,
    },
    /// The unit described itself and was told it's paired.
    Hello {
        id: DeviceId,
        hello: Hello,
    },
    /// A button, encoder, or menu pick.
    Input {
        id: DeviceId,
        message: DeviceMessage,
    },
    /// A report the codec refused. The unit stays connected.
    Rejected {
        id: DeviceId,
        error: DecodeError,
    },
    Disconnected {
        id: DeviceId,
        why: Lost,
    },
    /// A paired unit is attached but wouldn't open. Reported once; retried
    /// every scan.
    OpenFailed {
        id: DeviceId,
        error: TransportError,
    },
    /// A Detent unit that's neither paired nor ignored was attached. Not
    /// opened, except read only while identifying. Reported once per attach.
    Unknown {
        vid: u16,
        pid: u16,
        serial: Option<String>,
    },
    /// The unknown unit reported earlier is no longer attached.
    UnknownGone {
        vid: u16,
        pid: u16,
        serial: Option<String>,
    },
    /// Identifying (D-066): an unpaired unit was opened read only and asked
    /// for its HELLO. It's never acked and its input never reaches the router.
    CandidateFound {
        id: DeviceId,
        product: Option<String>,
    },
    /// A candidate described itself.
    CandidateHello {
        id: DeviceId,
        hello: Hello,
    },
    /// The user pressed a button on this candidate, or left Setup on its
    /// OLED (a HELLO after the first): this is the unit they mean.
    CandidatePressed(DeviceId),
    /// A candidate was unplugged or went silent.
    CandidateGone(DeviceId),
}

/// Timings. The defaults are the product's.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// How often the device list is scanned.
    pub scan_every: Duration,
    /// Silence after which a unit counts as gone. HEARTBEAT comes every 2 s.
    pub heartbeat_timeout: Duration,
    /// Longest a step waits for reports, across all units.
    pub read_wait: Duration,
    /// How long an action's result stays on the device's status line
    /// before the engine clears it (D-087).
    pub status_for: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            scan_every: Duration::from_secs(1),
            heartbeat_timeout: Duration::from_secs(6),
            read_wait: Duration::from_millis(20),
            status_for: Duration::from_secs(4),
        }
    }
}

struct Unit {
    id: DeviceId,
    path: CString,
    transport: Box<dyn DeviceTransport>,
    last_heard: Instant,
    hello: Option<Hello>,
    seq_out: u8,
    /// Opened read only while identifying, not paired (D-066).
    candidate: bool,
    /// HELLOs received, for spotting a candidate leaving Setup.
    hellos: u32,
    /// Its app owned OLED menu labels, sent after each HELLO ack (M6).
    menu: Vec<String>,
}

impl Unit {
    fn send(&mut self, msg: &AppMessage) -> Result<(), TransportError> {
        let report = encode_app(self.seq_out, msg).expect("app messages built here encode");
        // Seq advances even when the send fails, so gaps are visible.
        self.seq_out = self.seq_out.wrapping_add(1);
        self.transport.write(&report)
    }
}

fn same_serial(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Opens paired units and turns their reports into [`DeviceEvent`]s.
pub struct Devices<B> {
    bus: B,
    allow: Vec<DeviceEntry>,
    ignore: Vec<UnitRef>,
    opts: Options,
    next_scan: Option<Instant>,
    units: Vec<Unit>,
    /// Unknown units already reported, by path, while they stay attached.
    unknown_seen: HashMap<CString, (u16, u16, Option<String>)>,
    /// Paired units whose open failure was already reported.
    open_failed: HashSet<DeviceId>,
    /// Set by [`Devices::woke`]; handled at the start of the next step.
    woke: bool,
    /// Set by [`Devices::set_lists`]; handled at the start of the next step.
    relisted: bool,
    /// The pairing wizard is open: unpaired units are opened read only.
    identifying: bool,
}

impl<B: Bus> Devices<B> {
    pub fn new(bus: B, devices: &[DeviceEntry], ignored: &[UnitRef], opts: Options) -> Self {
        Self {
            bus,
            allow: devices.to_vec(),
            ignore: ignored.to_vec(),
            opts,
            next_scan: None,
            units: Vec::new(),
            unknown_seen: HashMap::new(),
            open_failed: HashSet::new(),
            woke: false,
            relisted: false,
            identifying: false,
        }
    }

    /// Paired units open now, in the order they were opened.
    pub fn connected(&self) -> Vec<&DeviceId> {
        self.units
            .iter()
            .filter(|u| !u.candidate)
            .map(|u| &u.id)
            .collect()
    }

    /// A connected paired unit's latest HELLO.
    pub fn hello(&self, id: &DeviceId) -> Option<&Hello> {
        self.units
            .iter()
            .find(|u| &u.id == id && !u.candidate)?
            .hello
            .as_ref()
    }

    /// The pairing wizard opened: from the next step, unpaired Detent units
    /// (Detent's VID and PID, with a serial, not ignored) are opened read
    /// only so a press can identify one (D-066).
    pub fn start_identify(&mut self) {
        self.identifying = true;
        self.next_scan = None;
    }

    /// The wizard closed: candidates are closed now, with no events.
    pub fn end_identify(&mut self) {
        self.identifying = false;
        self.units.retain(|u| !u.candidate);
    }

    /// Detent is quitting: tells each open paired unit it's no longer paired
    /// (so its LED and OLED say so) and closes every unit. Candidates are
    /// never written (D-066).
    pub fn release_all(&mut self) {
        for unit in self.units.iter_mut().filter(|u| !u.candidate) {
            let _ = unit.send(&AppMessage::HelloAck { paired: false });
        }
        self.units.clear();
    }

    /// The Mac just woke from sleep. The next step closes every unit and
    /// rescans at once, so units come back in about a second instead of
    /// after the 6 s silence timeout, and none is read through a stale handle.
    pub fn woke(&mut self) {
        self.woke = true;
    }

    /// Replaces the allowlist and ignore list (a config reload). The next
    /// step closes open units that are no longer paired or are now ignored,
    /// and rescans at once so newly paired units open; units still allowed
    /// keep their handles.
    pub fn set_lists(&mut self, devices: &[DeviceEntry], ignored: &[UnitRef]) {
        self.allow = devices.to_vec();
        self.ignore = ignored.to_vec();
        self.relisted = true;
    }

    /// Sets a paired unit's OLED status line (`""` clears it). Cut to the
    /// protocol's 20 printable ASCII characters; a unit that isn't open, or a
    /// failed send, is ignored (the next read notices a dead link).
    pub fn send_status(&mut self, id: &DeviceId, text: &str) {
        let clean: String = text
            .chars()
            .filter(|c| (' '..='~').contains(c))
            .take(detent_protocol::codec::MAX_STATUS_LEN)
            .collect();
        let Ok(text) = StatusText::new(&clean) else {
            return;
        };
        if let Some(unit) = self.units.iter_mut().find(|u| !u.candidate && u.id == *id) {
            let _ = unit.send(&AppMessage::Status { text });
        }
    }

    /// The menu labels the allowlist gives a unit.
    fn menu_of(&self, id: &DeviceId) -> Vec<String> {
        self.allow
            .iter()
            .find(|d| d.vid == id.vid && d.pid == id.pid && same_serial(&d.serial, &id.serial))
            .map(|d| d.menu.iter().map(|m| m.label.clone()).collect())
            .unwrap_or_default()
    }

    fn still_allowed(&self, id: &DeviceId) -> bool {
        let matches = |vid, pid, serial: &str| {
            vid == id.vid && pid == id.pid && same_serial(serial, &id.serial)
        };
        self.allow.iter().any(|d| matches(d.vid, d.pid, &d.serial))
            && !self.ignore.iter().any(|u| matches(u.vid, u.pid, &u.serial))
    }

    /// One turn: scan if due, read, check heartbeats. Blocks at most about
    /// `read_wait` (sleeping that long when nothing is open).
    pub fn step(&mut self, now: Instant) -> Vec<DeviceEvent> {
        let mut events = Vec::new();
        if std::mem::take(&mut self.relisted) {
            let gone: HashSet<DeviceId> = self
                .units
                .iter()
                .filter(|u| !u.candidate && !self.still_allowed(&u.id))
                .map(|u| u.id.clone())
                .collect();
            // A changed menu goes out now to units that have said HELLO;
            // the others get it after their HELLO is acked.
            let menus: Vec<Vec<String>> = self.units.iter().map(|u| self.menu_of(&u.id)).collect();
            for (unit, menu) in self.units.iter_mut().zip(menus) {
                if unit.candidate || unit.menu == menu {
                    continue;
                }
                unit.menu = menu;
                if unit.hello.is_some() {
                    let _ = send_menu(unit);
                }
            }
            // The device stays "paired" until told otherwise or unmounted.
            for unit in self.units.iter_mut().filter(|u| gone.contains(&u.id)) {
                let _ = unit.send(&AppMessage::HelloAck { paired: false });
            }
            self.drop_where(&mut events, |u| {
                gone.contains(&u.id).then_some(Lost::Unpaired)
            });
            // A candidate just paired or ignored closes quietly; a paired one
            // is reopened, with HELLO_ACK, by the scan below.
            let ignore = self.ignore.clone();
            let allow = self.allow.clone();
            self.units.retain(|u| {
                let listed = |vid, pid, serial: &str| {
                    vid == u.id.vid && pid == u.id.pid && same_serial(serial, &u.id.serial)
                };
                !u.candidate
                    || !(allow.iter().any(|d| listed(d.vid, d.pid, &d.serial))
                        || ignore.iter().any(|i| listed(i.vid, i.pid, &i.serial)))
            });
            self.open_failed.clear();
            self.next_scan = None;
        }
        if std::mem::take(&mut self.woke) {
            self.drop_where(&mut events, |_| Some(Lost::Slept));
            self.next_scan = None;
        }
        if self.next_scan.is_none_or(|t| now >= t) {
            self.next_scan = Some(now + self.opts.scan_every);
            self.scan(now, &mut events);
        }
        if self.units.is_empty() {
            thread::sleep(self.opts.read_wait);
        } else {
            self.read_all(now, &mut events);
        }
        let timeout = self.opts.heartbeat_timeout;
        self.drop_where(&mut events, |u| {
            (now.saturating_duration_since(u.last_heard) >= timeout)
                .then_some(Lost::HeartbeatTimeout)
        });
        events
    }

    fn is_ignored(&self, c: &Channel, serial: &str) -> bool {
        self.ignore
            .iter()
            .any(|u| u.vid == c.vid && u.pid == c.pid && same_serial(&u.serial, serial))
    }

    fn is_paired(&self, c: &Channel, serial: &str) -> bool {
        self.allow
            .iter()
            .any(|d| d.vid == c.vid && d.pid == c.pid && same_serial(&d.serial, serial))
    }

    fn scan(&mut self, now: Instant, events: &mut Vec<DeviceEvent>) {
        // A failed scan changes nothing; the next one retries.
        let Ok(channels) = self.bus.scan() else {
            return;
        };

        // Close units whose entry is gone, even if their reads still work.
        let present: HashSet<&CString> = channels.iter().map(|c| &c.path).collect();
        self.drop_where(events, |u| {
            (!present.contains(&u.path)).then_some(Lost::Detached)
        });
        self.unknown_seen.retain(|path, (vid, pid, serial)| {
            let still = present.contains(path);
            if !still {
                events.push(DeviceEvent::UnknownGone {
                    vid: *vid,
                    pid: *pid,
                    serial: serial.clone(),
                });
            }
            still
        });
        let present_ids: HashSet<DeviceId> = channels
            .iter()
            .filter_map(|c| {
                Some(DeviceId {
                    vid: c.vid,
                    pid: c.pid,
                    serial: c.serial.clone()?,
                })
            })
            .collect();
        self.open_failed.retain(|id| present_ids.contains(id));

        for c in &channels {
            let serial = c.serial.as_deref().unwrap_or("");
            if !serial.is_empty() && self.is_ignored(c, serial) {
                // Ignored: no longer news (reported again if it's un-ignored).
                self.unknown_seen.remove(&c.path);
                continue;
            }
            if serial.is_empty() || !self.is_paired(c, serial) {
                let ours = c.vid == ids::VID && c.pid == ids::PID;
                if ours
                    && self
                        .unknown_seen
                        .insert(c.path.clone(), (c.vid, c.pid, c.serial.clone()))
                        .is_none()
                {
                    events.push(DeviceEvent::Unknown {
                        vid: c.vid,
                        pid: c.pid,
                        serial: c.serial.clone(),
                    });
                }
                if ours && self.identifying && !serial.is_empty() {
                    let id = DeviceId {
                        vid: c.vid,
                        pid: c.pid,
                        serial: serial.to_owned(),
                    };
                    if !self.units.iter().any(|u| u.id == id) {
                        self.open_candidate(c, id, now, events);
                    }
                }
                continue;
            }
            // Paired now: no longer news (reported again if it's unpaired).
            self.unknown_seen.remove(&c.path);
            let id = DeviceId {
                vid: c.vid,
                pid: c.pid,
                serial: serial.to_owned(),
            };
            if self.units.iter().any(|u| u.id == id) {
                continue;
            }
            self.open(c, id, now, events);
        }
    }

    /// Opens an unpaired unit read only. A failure is silent: the wizard just
    /// doesn't list it, and the next scan tries again.
    fn open_candidate(
        &mut self,
        c: &Channel,
        id: DeviceId,
        now: Instant,
        events: &mut Vec<DeviceEvent>,
    ) {
        let Ok(transport) = self.bus.open_channel(c) else {
            return;
        };
        let mut unit = Unit {
            id: id.clone(),
            path: c.path.clone(),
            transport,
            last_heard: now,
            hello: None,
            seq_out: 0,
            candidate: true,
            hellos: 0,
            // A candidate is never written to beyond REQUEST_HELLO (D-066).
            menu: Vec::new(),
        };
        if unit.send(&AppMessage::RequestHello).is_err() {
            return;
        }
        events.push(DeviceEvent::CandidateFound {
            id,
            product: c.product.clone(),
        });
        self.units.push(unit);
    }

    fn open(&mut self, c: &Channel, id: DeviceId, now: Instant, events: &mut Vec<DeviceEvent>) {
        let transport = match self.bus.open_channel(c) {
            Ok(t) => t,
            Err(error) => {
                if self.open_failed.insert(id.clone()) {
                    events.push(DeviceEvent::OpenFailed { id, error });
                }
                return;
            }
        };
        self.open_failed.remove(&id);
        let mut unit = Unit {
            id: id.clone(),
            path: c.path.clone(),
            transport,
            last_heard: now,
            hello: None,
            seq_out: 0,
            candidate: false,
            hellos: 0,
            menu: self.menu_of(&id),
        };
        // The mount HELLO went out before we had the channel open; ask again.
        if let Err(e) = unit.send(&AppMessage::RequestHello) {
            events.push(DeviceEvent::OpenFailed { id, error: e });
            return;
        }
        events.push(DeviceEvent::Connected {
            id,
            product: c.product.clone(),
        });
        self.units.push(unit);
    }

    fn read_all(&mut self, now: Instant, events: &mut Vec<DeviceEvent>) {
        // The first read of each unit shares the wait; the rest only drain.
        let n = u32::try_from(self.units.len()).unwrap_or(u32::MAX);
        let first_wait = self.opts.read_wait / n;
        let mut lost: Vec<(usize, Lost)> = Vec::new();
        for (i, unit) in self.units.iter_mut().enumerate() {
            let mut wait = first_wait;
            loop {
                match unit.transport.read(wait) {
                    Ok(Some(report)) => {
                        if let Err(why) = handle(unit, &report, now, events) {
                            lost.push((i, why));
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        lost.push((i, Lost::Link(e)));
                        break;
                    }
                }
                wait = Duration::ZERO;
            }
        }
        for (i, why) in lost.into_iter().rev() {
            let unit = self.units.remove(i);
            events.push(lost_event(unit, why));
        }
    }

    /// Closes every unit `why` returns a reason for, in order.
    fn drop_where(&mut self, events: &mut Vec<DeviceEvent>, why: impl Fn(&Unit) -> Option<Lost>) {
        let mut kept = Vec::with_capacity(self.units.len());
        for unit in self.units.drain(..) {
            match why(&unit) {
                Some(why) => events.push(lost_event(unit, why)),
                None => kept.push(unit),
            }
        }
        self.units = kept;
    }
}

/// Sends a unit its app owned menu: every item in order, or a clear when it
/// has none (so a device never keeps items from another host). Labels were
/// checked by the config loader; one that somehow isn't valid is skipped.
fn send_menu(unit: &mut Unit) -> Result<(), TransportError> {
    let labels: Vec<Label> = unit
        .menu
        .iter()
        .filter_map(|l| Label::new(l).ok())
        .collect();
    if labels.is_empty() {
        return unit.send(&AppMessage::MenuClear);
    }
    let total = labels.len() as u8;
    for (i, label) in labels.into_iter().enumerate() {
        unit.send(&AppMessage::MenuItem {
            total,
            index: i as u8,
            item: i as u8 + 1,
            label,
        })?;
    }
    Ok(())
}

/// The event for a unit closing: a paired unit disconnected, a candidate gone.
fn lost_event(unit: Unit, why: Lost) -> DeviceEvent {
    if unit.candidate {
        DeviceEvent::CandidateGone(unit.id)
    } else {
        DeviceEvent::Disconnected { id: unit.id, why }
    }
}

/// A candidate's report: describes it or picks it, never acked or routed.
fn handle_candidate(unit: &mut Unit, msg: DeviceMessage, events: &mut Vec<DeviceEvent>) {
    match msg {
        DeviceMessage::Hello(hello) => {
            unit.hellos += 1;
            events.push(DeviceEvent::CandidateHello {
                id: unit.id.clone(),
                hello,
            });
            if unit.hellos > 1 {
                events.push(DeviceEvent::CandidatePressed(unit.id.clone()));
            }
        }
        DeviceMessage::Button {
            action: ButtonAction::Down,
            ..
        } => events.push(DeviceEvent::CandidatePressed(unit.id.clone())),
        _ => {}
    }
}

/// Decodes one report from `unit`. Any valid report counts as hearing from
/// it; garbage doesn't. Err means the unit must be closed.
fn handle(
    unit: &mut Unit,
    report: &[u8],
    now: Instant,
    events: &mut Vec<DeviceEvent>,
) -> Result<(), Lost> {
    let frame = match decode_device(report) {
        Ok(frame) => frame,
        Err(error) => {
            events.push(DeviceEvent::Rejected {
                id: unit.id.clone(),
                error,
            });
            return Ok(());
        }
    };
    unit.last_heard = now;
    if unit.candidate {
        handle_candidate(unit, frame.msg, events);
        return Ok(());
    }
    match frame.msg {
        DeviceMessage::Hello(hello) => {
            unit.send(&AppMessage::HelloAck { paired: true })
                .map_err(Lost::Link)?;
            send_menu(unit).map_err(Lost::Link)?;
            unit.hello = Some(hello.clone());
            events.push(DeviceEvent::Hello {
                id: unit.id.clone(),
                hello,
            });
        }
        DeviceMessage::Heartbeat { encoder_mode, .. } => {
            if let Some(h) = unit.hello.as_mut() {
                if h.encoder_mode != encoder_mode {
                    h.encoder_mode = encoder_mode;
                    events.push(DeviceEvent::Mode {
                        id: unit.id.clone(),
                        mode: encoder_mode,
                    });
                }
            }
        }
        message @ (DeviceMessage::Button { .. }
        | DeviceMessage::Encoder { .. }
        | DeviceMessage::MenuSelect { .. }) => events.push(DeviceEvent::Input {
            id: unit.id.clone(),
            message,
        }),
    }
    Ok(())
}
