//! Semantic checks serde can't express. Each failure names its field path.

use std::collections::HashMap;

use super::schema::{Action, Config, Scope};

/// A rule a parsed config broke, at a field path like `mappings[2].action.level`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub path: String,
    pub message: String,
}

fn problem(path: impl Into<String>, message: impl Into<String>) -> Problem {
    Problem {
        path: path.into(),
        message: message.into(),
    }
}

/// Looks like a bundle ID: dotted, no spaces.
fn is_bundle_id(s: &str) -> bool {
    s.contains('.') && !s.starts_with('.') && !s.ends_with('.') && !s.contains(char::is_whitespace)
}

const BUNDLE_ID: &str = "must be a bundle ID such as com.apple.mail";

/// Returns the first problem found, in document order.
pub fn check(config: &Config) -> Result<(), Problem> {
    let mut serials: HashMap<&str, usize> = HashMap::new();
    for (i, d) in config.devices.iter().enumerate() {
        if d.name.trim().is_empty() {
            return Err(problem(format!("devices[{i}].name"), "must not be empty"));
        }
        if d.serial.trim().is_empty() {
            return Err(problem(format!("devices[{i}].serial"), "must not be empty"));
        }
        if let Some(first) = serials.insert(&d.serial, i) {
            return Err(problem(
                format!("devices[{i}].serial"),
                format!("{} is already listed in devices[{first}]", d.serial),
            ));
        }
    }
    for (i, u) in config.ignored.iter().enumerate() {
        if u.serial.trim().is_empty() {
            return Err(problem(format!("ignored[{i}].serial"), "must not be empty"));
        }
    }

    let mut seen: HashMap<(&str, u8, _, &Scope), usize> = HashMap::new();
    for (i, m) in config.mappings.iter().enumerate() {
        if !serials.contains_key(m.device.as_str()) {
            return Err(problem(
                format!("mappings[{i}].device"),
                format!("{} isn't in devices", m.device),
            ));
        }
        if let Scope::App(id) = &m.scope {
            if !is_bundle_id(id) {
                return Err(problem(format!("mappings[{i}].scope.app"), BUNDLE_ID));
            }
        }
        check_action(&m.action, &format!("mappings[{i}].action"))?;
        if let Some(first) = seen.insert((m.device.as_str(), m.control, m.trigger, &m.scope), i) {
            return Err(problem(
                format!("mappings[{i}]"),
                format!("same device, control, trigger, and scope as mappings[{first}]"),
            ));
        }
    }
    Ok(())
}

fn check_action(action: &Action, path: &str) -> Result<(), Problem> {
    action
        .check_args()
        .map_err(|e| problem(format!("{path}.{}", e.field), e.message))
}

/// An action argument out of range, by field name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArgError {
    pub field: &'static str,
    pub message: &'static str,
}

impl std::fmt::Display for ArgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for ArgError {}

impl Action {
    /// Checks the arguments, the one set of rules the config loader and the
    /// executors share. Rejects NaN wherever a number is expected.
    pub fn check_args(&self) -> Result<(), ArgError> {
        let bad = |field, message| Err(ArgError { field, message });
        let step_ok = |s: f32| s > 0.0 && s <= 1.0;
        match self {
            Action::OpenApp { bundle_id } if !is_bundle_id(bundle_id) => {
                bad("bundle_id", BUNDLE_ID)
            }
            Action::VolumeUp { step } | Action::VolumeDown { step } if !step_ok(*step) => {
                bad("step", "must be more than 0 and at most 1")
            }
            Action::VolumeSet { level } if !(0.0..=1.0).contains(level) => {
                bad("level", "must be from 0 to 1")
            }
            Action::AppleScript { source, .. } if source.trim().is_empty() => {
                bad("source", "must not be empty")
            }
            Action::AppleScript { timeout_ms, .. } if !(100..=60_000).contains(timeout_ms) => {
                bad("timeout_ms", "must be from 100 to 60000")
            }
            _ => Ok(()),
        }
    }
}
