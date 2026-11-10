//! The engine: devices -> router -> actions, on background threads.
//!
//! The engine thread loops [`Devices::step`], resolves each input against the
//! frontmost app, and queues the actions. A worker thread runs them in order,
//! so a slow AppleScript never stops heartbeats or input from being read
//! (D-063). Everything that happens goes to a [`Sink`] as a [`Record`]: the
//! app logs them and drives the tray from them.

mod log;

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use crate::actions::{Executor, Outcome};
use crate::devices::{DeviceEvent, DeviceId, Devices, Options};
use crate::router::{trigger_of, Router};
use crate::store::{Action, Config, Trigger};
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
}

/// A wall clock gain over the monotonic clock this big means the Mac slept.
pub const SLEEP_GAP: Duration = Duration::from_secs(2);

/// Whether the Mac slept between two loop turns, from how far each clock
/// moved. On macOS `Instant` stops during sleep and the wall clock doesn't
/// (D-068); a wall clock step backwards counts as zero, not a sleep.
pub fn slept(monotonic: Duration, wall: Duration) -> bool {
    wall.saturating_sub(monotonic) >= SLEEP_GAP
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
                    Some(a) => write!(f, " -> {}", describe(a)),
                    None => write!(f, " -> no mapping"),
                }
            }
            Record::Reloaded { devices, mappings } => {
                write!(
                    f,
                    "config reloaded: {devices} device(s), {mappings} mapping(s)"
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
}

fn worker(jobs: Receiver<Job>, executor: Arc<dyn Executor>, sink: Arc<dyn Sink>) {
    // Ends when the engine drops its sender.
    for job in jobs {
        let started = Instant::now();
        let outcome = executor.run(&job.action);
        sink.record(&Record::Ran {
            id: job.id,
            action: job.action,
            outcome,
            waited: started - job.read_at,
            took: started.elapsed(),
        });
    }
}

/// The pipeline, ready to run on its own thread with [`Engine::spawn`].
pub struct Engine<B> {
    devices: Devices<B>,
    router: Router,
    frontmost: Frontmost,
    sink: Arc<dyn Sink>,
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
        let worker_sink = sink.clone();
        let worker = thread::Builder::new()
            .name("detent-actions".into())
            .spawn(move || worker(rx, executor, worker_sink))
            .expect("spawn the action worker");
        Self {
            devices: Devices::new(bus, &config.devices, &config.ignored, options),
            router: Router::new(config),
            frontmost,
            sink,
            jobs,
            _worker: worker,
        }
    }

    /// Puts a new config in place: new mappings apply to the next input, and
    /// the devices module closes units no longer paired and opens new ones.
    pub fn reload(&mut self, config: &Config) {
        self.router = Router::new(config);
        self.devices.set_lists(&config.devices, &config.ignored);
        self.sink.record(&Record::Reloaded {
            devices: config.devices.len(),
            mappings: config.mappings.len(),
        });
    }

    /// One turn: read devices, then resolve and queue any input.
    pub fn step(&mut self, now: Instant) {
        let events = self.devices.step(now);
        let read_at = Instant::now();
        for event in events {
            self.sink.record(&Record::Device(event.clone()));
            let DeviceEvent::Input { id, message } = event else {
                continue;
            };
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
            });
            if let Some(action) = action {
                for _ in 0..count {
                    let _ = self.jobs.send(Job {
                        id: id.clone(),
                        action: action.clone(),
                        read_at,
                    });
                }
            }
        }
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
                while !flag.load(Ordering::Relaxed) {
                    for command in incoming.try_iter() {
                        match command {
                            Command::Reload(config) => self.reload(&config),
                            Command::Identify(true) => self.devices.start_identify(),
                            Command::Identify(false) => self.devices.end_identify(),
                        }
                    }
                    let now = (Instant::now(), SystemTime::now());
                    let wall = now.1.duration_since(last.1).unwrap_or_default();
                    if slept(now.0 - last.0, wall) {
                        self.devices.woke();
                    }
                    last = now;
                    self.step(now.0);
                }
                // Dropping `self` drops the job sender: the worker finishes
                // what's queued, then ends.
            })
            .expect("spawn the engine");
        EngineHandle {
            stop,
            thread,
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
}

pub struct EngineHandle {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
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

    /// Stops reading devices and returns once the engine thread has ended.
    /// Actions still queued finish on the worker in the background.
    pub fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.thread.join();
    }
}
