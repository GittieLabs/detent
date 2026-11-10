//! Actions: run a mapping's [`Action`]. [`Executor`] is the seam the engine
//! calls; [`SystemExecutor`] has the real effects (one function per action
//! type) and [`RecordingExecutor`] only records, for tests and Dry run.
//!
//! The trait is `Executor`, not `Action`, because `Action` is the config's
//! data type (D-058). Every executor checks arguments first, with the same
//! rules the config loader uses ([`Action::check_args`]).

pub mod applescript;

use std::fmt;
use std::sync::Mutex;

use crate::store::{Action, ArgError};

pub use applescript::ScriptError;

/// What an action did, for the engine's log.
#[derive(Debug, Clone, PartialEq)]
pub enum Done {
    /// Launch Services accepted the request to open this app.
    Opened(String),
    /// Output volume moved, 0.0 to 1.0. Also unmutes (D-059).
    Volume { from: f32, to: f32 },
    /// Mute is now this.
    Muted(bool),
    /// The script finished; its result, trimmed.
    Script(String),
    /// Nothing ran: a recording executor (tests, Dry run).
    Recorded,
}

impl fmt::Display for Done {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pct = |v: f32| (v * 100.0).round() as i32;
        match self {
            Self::Opened(id) => write!(f, "opened {id}"),
            Self::Volume { from, to } => write!(f, "volume {}% -> {}%", pct(*from), pct(*to)),
            Self::Muted(true) => write!(f, "muted"),
            Self::Muted(false) => write!(f, "unmuted"),
            Self::Script(out) if out.is_empty() => write!(f, "script finished"),
            Self::Script(out) => write!(f, "script returned {out:?}"),
            Self::Recorded => write!(f, "recorded (dry run)"),
        }
    }
}

/// Why an action didn't run or didn't finish.
#[derive(Debug, Clone, PartialEq)]
pub enum ActionError {
    /// Arguments out of range; nothing ran.
    Invalid(ArgError),
    #[cfg(target_os = "macos")]
    OpenApp(crate::macos::OpenAppError),
    #[cfg(target_os = "macos")]
    Audio(crate::macos::AudioError),
    Script(ScriptError),
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(e) => write!(f, "invalid action: {e}"),
            #[cfg(target_os = "macos")]
            Self::OpenApp(e) => e.fmt(f),
            #[cfg(target_os = "macos")]
            Self::Audio(e) => e.fmt(f),
            Self::Script(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ActionError {}

/// The result every executor returns.
pub type Outcome = Result<Done, ActionError>;

/// Runs actions. Called from the engine's background thread; may block (an
/// AppleScript runs until it finishes or times out).
pub trait Executor: Send + Sync {
    fn run(&self, action: &Action) -> Outcome;
}

/// The volume a volume action moves to from `current`, clamped to 0..=1, or
/// `None` for actions that don't set the volume.
pub fn next_volume(current: f32, action: &Action) -> Option<f32> {
    let to = match action {
        Action::VolumeUp { step } => current + step,
        Action::VolumeDown { step } => current - step,
        Action::VolumeSet { level } => *level,
        _ => return None,
    };
    Some(to.clamp(0.0, 1.0))
}

/// Records each valid action it's given and runs nothing. For tests and the
/// Dry run toggle; refuses bad arguments exactly as the real one does.
#[derive(Debug, Default)]
pub struct RecordingExecutor {
    calls: Mutex<Vec<Action>>,
}

impl RecordingExecutor {
    /// The actions run so far, in order.
    pub fn calls(&self) -> Vec<Action> {
        self.calls.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl Executor for RecordingExecutor {
    fn run(&self, action: &Action) -> Outcome {
        action.check_args().map_err(ActionError::Invalid)?;
        self.calls
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(action.clone());
        Ok(Done::Recorded)
    }
}

/// Runs actions for real: NSWorkspace, CoreAudio, and osascript (D-053).
#[cfg(target_os = "macos")]
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemExecutor;

#[cfg(target_os = "macos")]
impl Executor for SystemExecutor {
    fn run(&self, action: &Action) -> Outcome {
        action.check_args().map_err(ActionError::Invalid)?;
        match action {
            Action::OpenApp { bundle_id } => open_app(bundle_id),
            Action::VolumeUp { .. } | Action::VolumeDown { .. } | Action::VolumeSet { .. } => {
                volume(action)
            }
            Action::MuteToggle => mute_toggle(),
            Action::AppleScript { source, timeout_ms } => run_applescript(source, *timeout_ms),
        }
    }
}

#[cfg(target_os = "macos")]
fn open_app(bundle_id: &str) -> Outcome {
    crate::macos::open_app(bundle_id).map_err(ActionError::OpenApp)?;
    Ok(Done::Opened(bundle_id.to_owned()))
}

/// Volume up, down, or set. Unmutes first if muted, as the volume keys do
/// (D-059); a device with no mute control just has its volume set.
#[cfg(target_os = "macos")]
fn volume(action: &Action) -> Outcome {
    use crate::macos::{muted, set_muted, set_volume, volume, AudioError};
    let from = volume().map_err(ActionError::Audio)?;
    let to = next_volume(from, action).unwrap_or(from);
    match muted() {
        Ok(true) => set_muted(false).map_err(ActionError::Audio)?,
        Ok(false) | Err(AudioError::Unsupported(_)) => {}
        Err(e) => return Err(ActionError::Audio(e)),
    }
    set_volume(to).map_err(ActionError::Audio)?;
    Ok(Done::Volume { from, to })
}

#[cfg(target_os = "macos")]
fn mute_toggle() -> Outcome {
    use crate::macos::{muted, set_muted};
    let now = !muted().map_err(ActionError::Audio)?;
    set_muted(now).map_err(ActionError::Audio)?;
    Ok(Done::Muted(now))
}

#[cfg(target_os = "macos")]
fn run_applescript(source: &str, timeout_ms: u32) -> Outcome {
    let timeout = std::time::Duration::from_millis(timeout_ms.into());
    applescript::run(source, timeout)
        .map(Done::Script)
        .map_err(ActionError::Script)
}
