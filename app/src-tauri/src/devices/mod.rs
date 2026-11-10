//! Devices: finds attached units, opens the paired ones, runs the HELLO
//! handshake, decodes their reports, and notices when they go away.
//!
//! [`Devices::step`] is one turn of the engine's loop, with the time passed
//! in so timeouts are testable. Each step rescans the device list if a second
//! has passed, reads every open unit (waiting at most `read_wait` in all),
//! then drops any unit silent for 6 s. Only allowlisted units are ever opened;
//! the ignore list wins over the allowlist; other Detent units are reported
//! once per attach and never read (D-062).

use std::collections::HashSet;
use std::ffi::CString;
use std::fmt;
use std::thread;
use std::time::{Duration, Instant};

use detent_protocol::codec::{
    decode_device, encode_app, AppMessage, DecodeError, DeviceMessage, Hello,
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
}

/// What happened, for the engine and its log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceEvent {
    /// A paired unit was opened and asked for its HELLO.
    Connected(DeviceId),
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
    /// A Detent unit that's neither paired nor ignored was attached. Never
    /// opened. Reported once per attach.
    Unknown {
        vid: u16,
        pid: u16,
        serial: Option<String>,
    },
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
}

impl Default for Options {
    fn default() -> Self {
        Self {
            scan_every: Duration::from_secs(1),
            heartbeat_timeout: Duration::from_secs(6),
            read_wait: Duration::from_millis(20),
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
    /// Paths of unknown units already reported, while they stay attached.
    unknown_seen: HashSet<CString>,
    /// Paired units whose open failure was already reported.
    open_failed: HashSet<DeviceId>,
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
            unknown_seen: HashSet::new(),
            open_failed: HashSet::new(),
        }
    }

    /// Units open now, in the order they were opened.
    pub fn connected(&self) -> Vec<&DeviceId> {
        self.units.iter().map(|u| &u.id).collect()
    }

    /// A connected unit's latest HELLO.
    pub fn hello(&self, id: &DeviceId) -> Option<&Hello> {
        self.units.iter().find(|u| &u.id == id)?.hello.as_ref()
    }

    /// One turn: scan if due, read, check heartbeats. Blocks at most about
    /// `read_wait` (sleeping that long when nothing is open).
    pub fn step(&mut self, now: Instant) -> Vec<DeviceEvent> {
        let mut events = Vec::new();
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
        self.unknown_seen.retain(|p| present.contains(p));
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
                continue;
            }
            if serial.is_empty() || !self.is_paired(c, serial) {
                let ours = c.vid == ids::VID && c.pid == ids::PID;
                if ours && self.unknown_seen.insert(c.path.clone()) {
                    events.push(DeviceEvent::Unknown {
                        vid: c.vid,
                        pid: c.pid,
                        serial: c.serial.clone(),
                    });
                }
                continue;
            }
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
        };
        // The mount HELLO went out before we had the channel open; ask again.
        if let Err(e) = unit.send(&AppMessage::RequestHello) {
            events.push(DeviceEvent::OpenFailed { id, error: e });
            return;
        }
        events.push(DeviceEvent::Connected(id));
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
            events.push(DeviceEvent::Disconnected { id: unit.id, why });
        }
    }

    /// Closes every unit `why` returns a reason for, in order.
    fn drop_where(&mut self, events: &mut Vec<DeviceEvent>, why: impl Fn(&Unit) -> Option<Lost>) {
        let mut kept = Vec::with_capacity(self.units.len());
        for unit in self.units.drain(..) {
            match why(&unit) {
                Some(why) => events.push(DeviceEvent::Disconnected { id: unit.id, why }),
                None => kept.push(unit),
            }
        }
        self.units = kept;
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
    match frame.msg {
        DeviceMessage::Hello(hello) => {
            unit.send(&AppMessage::HelloAck { paired: true })
                .map_err(Lost::Link)?;
            unit.hello = Some(hello.clone());
            events.push(DeviceEvent::Hello {
                id: unit.id.clone(),
                hello,
            });
        }
        DeviceMessage::Heartbeat { encoder_mode, .. } => {
            if let Some(h) = unit.hello.as_mut() {
                h.encoder_mode = encoder_mode;
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
