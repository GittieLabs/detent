//! The engine: devices -> router -> actions, on background threads.
//!
//! The engine thread loops [`Devices::step`], resolves each input against the
//! frontmost app, and queues the actions. A worker thread runs them in order,
//! so a slow AppleScript never stops heartbeats or input from being read
//! (D-063). Everything that happens goes to a [`Sink`] as a [`Record`]: the
//! app logs them and drives the tray from them.

mod log;

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use detent_protocol::codec::DeviceMessage;

use crate::actions::{Done, Executor, Outcome, RecordingExecutor};
use crate::devices::{DeviceEvent, DeviceId, Devices, Options};
use crate::router::{trigger_of, Router};
use crate::store::{Action, Config, MenuItem, Trigger};
use crate::transport::Bus;

pub use log::{default_path as default_log_path, FileLog};

/// Something that happened, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum Record {
    /// Straight from the devices module, input included.
    Device(DeviceEvent),
    /// An input matched against the mappings. `action` is `None` when no
    /// mapping applies; `count` is how many times it will run (detents).
    Resolved {
        id: DeviceId,
        control: u8,
        trigger: Trigger,
        frontmost: Option<String>,
        action: Option<Action>,
        count: u8,
        /// Detent is paused: the action is logged, not run.
        paused: bool,
        /// Dry run (the Test screen, D-079): the action is recorded, not run.
        dry_run: bool,
    },
    /// An action finished. `waited` is from the report being read to the
    /// action starting (press to dispatch, less USB transit); `took` is the
    /// action itself.
    Ran {
        id: DeviceId,
        action: Action,
        outcome: Outcome,
        waited: Duration,
        took: Duration,
    },
    /// A new config was put in place without a relaunch.
    Reloaded { devices: usize, mappings: usize },
    /// Dry run turned on or off (D-079).
    DryRun { on: bool },
    /// An app owned OLED menu item was picked (M6). `label` and `action`
    /// are `None` when the id matches no item (the menu changed meanwhile).
    MenuPicked {
        id: DeviceId,
        item: u8,
        label: Option<String>,
        action: Option<Action>,
        paused: bool,
        dry_run: bool,
    },
    /// A thread caught a panic and carried on (M9 crash recovery). The
    /// panic hook has already logged where it happened, with a backtrace.
    Recovered { thread: String, message: String },
}

/// A wall clock gain over the monotonic clock this big means the Mac slept.
pub const SLEEP_GAP: Duration = Duration::from_secs(2);

/// Whether the Mac slept between two loop turns, from how far each clock
/// moved. On macOS `Instant` stops during sleep and the wall clock doesn't
/// (D-068); a wall clock step backwards counts as zero, not a sleep.
pub fn slept(monotonic: Duration, wall: Duration) -> bool {
    wall.saturating_sub(monotonic) >= SLEEP_GAP
}

/// How long the engine waits before its next turn after `in_a_row` panics
/// without a clean turn between: 250 ms, doubling, at most 30 s. A bug that
/// panics on every turn then costs a log line every 30 s, not a busy loop.
pub fn recovery_delay(in_a_row: u32) -> Duration {
    let max = Duration::from_secs(30);
    let shift = in_a_row.saturating_sub(1).min(16);
    Duration::from_millis(250)
        .checked_mul(1 << shift)
        .map_or(max, |d| d.min(max))
}

/// A panic's message, from its payload (`panic!` gives a `&str` or a
/// `String`).
pub fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "a panic with no message".to_owned()
    }
}

/// Writes every panic to `log`: the thread, the message, where, and a
/// backtrace, then hands it to the previous hook (stderr). Panics the
/// engine catches are logged this way too, before it carries on.
pub fn install_panic_hook(log: Arc<FileLog>) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = thread::current();
        let name = thread.name().unwrap_or("unnamed");
        let message = panic_message(info.payload());
        let at = info
            .location()
            .map(|l| format!(" at {}:{}", l.file(), l.line()))
            .unwrap_or_default();
        log.line(&format!("panic in thread {name}: {message}{at}"));
        let trace = std::backtrace::Backtrace::force_capture().to_string();
        log.line(&format!("backtrace:\n{trace}"));
        previous(info);
    }));
}

/// Where records go. Called from the engine and worker threads.
pub trait Sink: Send + Sync {
    fn record(&self, record: &Record);
}

/// The frontmost app's bundle ID, asked once per input.
pub type Frontmost = Arc<dyn Fn() -> Option<String> + Send + Sync>;

/// Short, log friendly form of an action.
pub fn describe(action: &Action) -> String {
    let pct = |v: f32| (v * 100.0).round() as i32;
    match action {
        Action::OpenApp { bundle_id } => format!("open_app {bundle_id}"),
        Action::VolumeUp { step } => format!("volume_up {}%", pct(*step)),
        Action::VolumeDown { step } => format!("volume_down {}%", pct(*step)),
        Action::VolumeSet { level } => format!("volume_set {}%", pct(*level)),
        Action::MuteToggle => "mute_toggle".into(),
        Action::SendShortcut { keys } => format!("send_shortcut {keys}"),
        Action::OpenDashboard {
            url, display, mode, ..
        } => {
            let on = display
                .as_deref()
                .map(|d| format!(" on {d}"))
                .unwrap_or_default();
            let how = if *mode == crate::store::DashboardMode::Window {
                " in a window"
            } else {
                ""
            };
            format!("open_dashboard {url}{on}{how}")
        }
        Action::AppleScript { source, .. } => {
            let first = source.lines().next().unwrap_or("");
            let more = if source.lines().nth(1).is_some() {
                " ..."
            } else {
                ""
            };
            format!("applescript {first:?}{more}")
        }
    }
}

fn input_name(control: u8, trigger: Trigger) -> String {
    match trigger {
        Trigger::Press => format!("button {control} press"),
        Trigger::LongPress => format!("button {control} long_press"),
        Trigger::Clockwise => format!("encoder {control} clockwise"),
        Trigger::Counterclockwise => format!("encoder {control} counterclockwise"),
    }
}

impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ms = |d: &Duration| d.as_secs_f64() * 1000.0;
        match self {
            Record::Device(e) => write!(f, "device {e:?}"),
            Record::Resolved {
                id,
                control,
                trigger,
                frontmost,
                action,
                count,
                paused,
                dry_run,
            } => {
                write!(f, "{} {}", id.serial, input_name(*control, *trigger))?;
                if *count > 1 {
                    write!(f, " x{count}")?;
                }
                write!(
                    f,
                    " in {}",
                    frontmost.as_deref().unwrap_or("(no frontmost app)")
                )?;
                match action {
                    Some(a) => write!(f, " -> {}", describe(a))?,
                    None => write!(f, " -> no mapping")?,
                }
                if *paused && action.is_some() {
                    write!(f, " (paused, not run)")?;
                } else if *dry_run && action.is_some() {
                    write!(f, " (dry run)")?;
                }
                Ok(())
            }
            Record::Reloaded { devices, mappings } => {
                write!(
                    f,
                    "config reloaded: {devices} device(s), {mappings} mapping(s)"
                )
            }
            Record::MenuPicked {
                id,
                item,
                label,
                action,
                paused,
                dry_run,
            } => {
                match (label, action) {
                    (Some(l), Some(a)) => write!(f, "{} menu {l:?} -> {}", id.serial, describe(a))?,
                    _ => write!(f, "{} menu item {item} -> no such item", id.serial)?,
                }
                if action.is_some() && *paused {
                    write!(f, " (paused, not run)")?;
                } else if action.is_some() && *dry_run {
                    write!(f, " (dry run)")?;
                }
                Ok(())
            }
            Record::DryRun { on: true } => {
                write!(f, "dry run on: actions are logged, not run")
            }
            Record::DryRun { on: false } => write!(f, "dry run off"),
            Record::Recovered { thread, message } => {
                write!(
                    f,
                    "recovered from a panic in the {thread} thread: {message}"
                )
            }
            Record::Ran {
                id,
                action,
                outcome,
                waited,
                took,
            } => {
                write!(f, "{} {}: ", id.serial, describe(action))?;
                match outcome {
                    Ok(done) => write!(f, "{done}")?,
                    Err(e) => write!(f, "FAILED {e}")?,
                }
                write!(
                    f,
                    " (dispatch {:.1} ms, took {:.1} ms)",
                    ms(waited),
                    ms(took)
                )
            }
        }
    }
}

struct Job {
    id: DeviceId,
    action: Action,
    read_at: Instant,
    /// Run on the recording executor instead (D-079).
    dry_run: bool,
}

/// What the device's status line shows for an action's result (D-087): a
/// few words, at most 20 printable ASCII characters. `None` for a dry run.
pub fn status_of(outcome: &Outcome) -> Option<String> {
    let text = match outcome {
        Ok(Done::Recorded) => return None,
        Ok(Done::Volume { to, .. }) => format!("Vol {}%", (to * 100.0).round() as i32),
        Ok(Done::Muted(true)) => "Muted".into(),
        Ok(Done::Muted(false)) => "Unmuted".into(),
        Ok(Done::Opened(id)) => {
            // `com.apple.mail` -> `Mail`: the bundle ID's last part.
            let last = id.rsplit('.').next().unwrap_or(id);
            let mut chars = last.chars();
            let name: String = chars
                .next()
                .map(|c| c.to_uppercase().chain(chars).collect())
                .unwrap_or_default();
            format!("Opened {name}")
        }
        Ok(Done::Sent(keys)) => format!("Sent {keys}"),
        Ok(Done::Script(_)) => "Script done".into(),
        Ok(Done::Dashboard { shown: true }) => "Dashboard on".into(),
        Ok(Done::Dashboard { shown: false }) => "Dashboard off".into(),
        Err(_) => "Failed".into(),
    };
    Some(
        text.chars()
            .filter(|c| (' '..='~').contains(c))
            .take(detent_protocol::codec::MAX_STATUS_LEN)
            .collect(),
    )
}

fn worker(
    jobs: Receiver<Job>,
    executor: Arc<dyn Executor>,
    sink: Arc<dyn Sink>,
    status: Sender<(DeviceId, String)>,
) {
    let recording = RecordingExecutor::default();
    // Ends when the engine drops its sender.
    for job in jobs {
        let started = Instant::now();
        let run = || {
            if job.dry_run {
                recording.run(&job.action)
            } else {
                executor.run(&job.action)
            }
        };
        // A panicking action fails that action only; the worker keeps going.
        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)).unwrap_or_else(|payload| {
                let message = panic_message(payload.as_ref());
                sink.record(&Record::Recovered {
                    thread: "actions".into(),
                    message: message.clone(),
                });
                Err(crate::actions::ActionError::Panicked(message))
            });
        if let Some(text) = status_of(&outcome) {
            let _ = status.send((job.id.clone(), text));
        }
        sink.record(&Record::Ran {
            id: job.id,
            action: job.action,
            outcome,
            waited: started - job.read_at,
            took: started.elapsed(),
        });
    }
}

/// Menu items by serial (lowercase), item `n` at index `n - 1`.
fn menus_of(config: &Config) -> HashMap<String, Vec<MenuItem>> {
    config
        .devices
        .iter()
        .map(|d| (d.serial.to_lowercase(), d.menu.clone()))
        .collect()
}

/// The pipeline, ready to run on its own thread with [`Engine::spawn`].
pub struct Engine<B> {
    devices: Devices<B>,
    router: Router,
    frontmost: Frontmost,
    sink: Arc<dyn Sink>,
    /// The config's Pause: input is resolved and logged, never queued.
    paused: bool,
    /// Dry run: actions go to a recording executor (D-079).
    dry_run: bool,
    /// Each paired unit's app owned menu items, by serial (lowercase).
    menus: HashMap<String, Vec<MenuItem>>,
    /// Action results from the worker, for the device's status line.
    statuses: Receiver<(DeviceId, String)>,
    /// When each unit's status line is due to be cleared.
    clear_at: HashMap<DeviceId, Instant>,
    status_for: Duration,
    jobs: Sender<Job>,
    /// Kept so the worker is owned by the engine; dropping `jobs` ends it.
    _worker: JoinHandle<()>,
}

impl<B: Bus + 'static> Engine<B> {
    /// Builds the pipeline from a loaded config and starts the action worker.
    pub fn new(
        bus: B,
        config: &Config,
        executor: Arc<dyn Executor>,
        frontmost: Frontmost,
        sink: Arc<dyn Sink>,
        options: Options,
    ) -> Self {
        let (jobs, rx) = mpsc::channel();
        let (status_tx, statuses) = mpsc::channel();
        let worker_sink = sink.clone();
        let worker = thread::Builder::new()
            .name("detent-actions".into())
            .spawn(move || worker(rx, executor, worker_sink, status_tx))
            .expect("spawn the action worker");
        Self {
            devices: Devices::new(bus, &config.devices, &config.ignored, options),
            router: Router::new(config),
            frontmost,
            sink,
            paused: config.paused,
            dry_run: false,
            menus: menus_of(config),
            statuses,
            clear_at: HashMap::new(),
            status_for: options.status_for,
            jobs,
            _worker: worker,
        }
    }

    /// Puts a new config in place: new mappings apply to the next input, and
    /// the devices module closes units no longer paired and opens new ones.
    pub fn reload(&mut self, config: &Config) {
        self.router = Router::new(config);
        self.paused = config.paused;
        self.menus = menus_of(config);
        self.devices.set_lists(&config.devices, &config.ignored);
        self.sink.record(&Record::Reloaded {
            devices: config.devices.len(),
            mappings: config.mappings.len(),
        });
    }

    /// Puts each finished action's result on its device's status line, and
    /// clears lines whose time is up (a newer result restarts the clock).
    fn show_statuses(&mut self, now: Instant) {
        while let Ok((id, text)) = self.statuses.try_recv() {
            self.devices.send_status(&id, &text);
            self.clear_at.insert(id, now + self.status_for);
        }
        let due: Vec<DeviceId> = self
            .clear_at
            .iter()
            .filter(|(_, at)| now >= **at)
            .map(|(id, _)| id.clone())
            .collect();
        for id in due {
            self.clear_at.remove(&id);
            self.devices.send_status(&id, "");
        }
    }

    /// Runs the picked menu item's action, as a press would run a mapping:
    /// Pause and dry run apply.
    fn menu_picked(&mut self, id: DeviceId, item: u8, read_at: Instant) {
        let picked = self
            .menus
            .get(&id.serial.to_lowercase())
            .and_then(|items| items.get(usize::from(item).checked_sub(1)?))
            .cloned();
        let action = picked.as_ref().map(|m| m.action.clone());
        self.sink.record(&Record::MenuPicked {
            id: id.clone(),
            item,
            label: picked.map(|m| m.label),
            action: action.clone(),
            paused: self.paused,
            dry_run: self.dry_run,
        });
        if let (Some(action), false) = (action, self.paused) {
            let _ = self.jobs.send(Job {
                id,
                action,
                read_at,
                dry_run: self.dry_run,
            });
        }
    }

    /// Turns dry run on or off (D-079): from the next input, actions are
    /// recorded instead of run. Logged as [`Record::DryRun`].
    /// Asking for the state it's already in records nothing.
    pub fn set_dry_run(&mut self, on: bool) {
        if self.dry_run != on {
            self.dry_run = on;
            self.sink.record(&Record::DryRun { on });
        }
    }

    /// One turn: read devices, then resolve and queue any input.
    pub fn step(&mut self, now: Instant) {
        self.show_statuses(now);
        let events = self.devices.step(now);
        let read_at = Instant::now();
        for event in events {
            self.sink.record(&Record::Device(event.clone()));
            let DeviceEvent::Input { id, message } = event else {
                continue;
            };
            if let DeviceMessage::MenuSelect { item } = message {
                self.menu_picked(id, item, read_at);
                continue;
            }
            let Some((control, trigger, count)) = trigger_of(&message) else {
                continue;
            };
            let frontmost = (self.frontmost)();
            let action = self
                .router
                .resolve(&id.serial, control, trigger, frontmost.as_deref())
                .map(|m| m.action.clone());
            self.sink.record(&Record::Resolved {
                id: id.clone(),
                control,
                trigger,
                frontmost,
                action: action.clone(),
                count,
                paused: self.paused,
                dry_run: self.dry_run,
            });
            if self.paused {
                continue;
            }
            if let Some(action) = action {
                for _ in 0..count {
                    let _ = self.jobs.send(Job {
                        id: id.clone(),
                        action: action.clone(),
                        read_at,
                        dry_run: self.dry_run,
                    });
                }
            }
        }
    }

    /// One loop turn: the app's commands, the sleep check, then a step.
    fn turn(&mut self, incoming: &Receiver<Command>, last: &mut (Instant, SystemTime)) {
        for command in incoming.try_iter() {
            match command {
                Command::Reload(config) => self.reload(&config),
                Command::Identify(true) => self.devices.start_identify(),
                Command::Identify(false) => self.devices.end_identify(),
                Command::DryRun(on) => self.set_dry_run(on),
            }
        }
        let now = (Instant::now(), SystemTime::now());
        let wall = now.1.duration_since(last.1).unwrap_or_default();
        if slept(now.0 - last.0, wall) {
            self.devices.woke();
        }
        *last = now;
        self.step(now.0);
    }

    /// Runs the engine on its own thread until [`EngineHandle::stop`].
    pub fn spawn(mut self) -> EngineHandle {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let (commands, incoming) = mpsc::channel::<Command>();
        let thread = thread::Builder::new()
            .name("detent-engine".into())
            .spawn(move || {
                let mut last = (Instant::now(), SystemTime::now());
                let mut panics = 0u32;
                while !flag.load(Ordering::Relaxed) {
                    let turn = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        self.turn(&incoming, &mut last)
                    }));
                    match turn {
                        Ok(()) => panics = 0,
                        Err(payload) => {
                            // Carry on in place (D-102): log it, reopen every
                            // unit so none is left half read, and back off.
                            panics = panics.saturating_add(1);
                            self.sink.record(&Record::Recovered {
                                thread: "engine".into(),
                                message: panic_message(payload.as_ref()),
                            });
                            self.devices.reopen_all();
                            let until = Instant::now() + recovery_delay(panics);
                            while Instant::now() < until && !flag.load(Ordering::Relaxed) {
                                thread::sleep(Duration::from_millis(20));
                            }
                        }
                    }
                }
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    self.devices.release_all()
                }));
                // Dropping `self` drops the job sender: the worker finishes
                // what's queued, then ends.
            })
            .expect("spawn the engine");
        EngineHandle {
            stop,
            thread: Mutex::new(Some(thread)),
            commands,
        }
    }
}

/// A running engine.
/// What the app asks of a running engine, applied in order before its next
/// read (within about 20 ms).
enum Command {
    Reload(Config),
    Identify(bool),
    DryRun(bool),
}

pub struct EngineHandle {
    stop: Arc<AtomicBool>,
    /// Taken by the first `stop`.
    thread: Mutex<Option<JoinHandle<()>>>,
    commands: Sender<Command>,
}

impl EngineHandle {
    /// Hands the engine a new config. The caller has already saved and
    /// validated it.
    pub fn reload(&self, config: Config) {
        let _ = self.commands.send(Command::Reload(config));
    }

    /// Starts or ends pairing identification (D-066): while on, unpaired
    /// units are opened read only and report `Candidate*` events.
    pub fn identify(&self, on: bool) {
        let _ = self.commands.send(Command::Identify(on));
    }

    /// Turns dry run on or off (D-079), before the next read.
    pub fn set_dry_run(&self, on: bool) {
        let _ = self.commands.send(Command::DryRun(on));
    }

    /// Stops reading devices, tells paired units they're released, and
    /// returns once the engine thread has ended. Later calls do nothing.
    /// Actions still queued finish on the worker in the background.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        let thread = self.thread.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(thread) = thread {
            let _ = thread.join();
        }
    }
}
