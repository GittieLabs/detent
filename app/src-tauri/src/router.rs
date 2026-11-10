//! Resolves a device event plus the frontmost app to at most one mapping:
//! an app scoped mapping for the frontmost app wins, otherwise the Everywhere
//! one, otherwise nothing (product.md: "The one concept: a mapping"). Pure.

use std::collections::HashMap;

use detent_protocol::codec::{ButtonAction, DeviceMessage};

use crate::store::{Config, Mapping, Scope, Trigger};

/// What a device message asks the router about: the control, the trigger, and
/// how many times it fired (an ENCODER delta of 3 is three clockwise detents,
/// D-054). `None` for messages that never trigger anything, including button
/// release.
pub fn trigger_of(message: &DeviceMessage) -> Option<(u8, Trigger, u8)> {
    match *message {
        DeviceMessage::Button { control, action } => match action {
            ButtonAction::Down => Some((control, Trigger::Press, 1)),
            ButtonAction::LongPress => Some((control, Trigger::LongPress, 1)),
            ButtonAction::Up => None,
        },
        DeviceMessage::Encoder { control, delta } => {
            let trigger = if delta > 0 {
                Trigger::Clockwise
            } else {
                Trigger::Counterclockwise
            };
            // The codec rejects a zero delta, so this is at least 1.
            Some((control, trigger, delta.unsigned_abs()))
        }
        DeviceMessage::Hello(_)
        | DeviceMessage::MenuSelect { .. }
        | DeviceMessage::Heartbeat { .. } => None,
    }
}

/// The mappings for one device, control, and trigger.
#[derive(Debug, Default)]
struct Slot {
    everywhere: Option<usize>,
    /// Keyed by lowercased bundle ID: Launch Services treats bundle IDs case
    /// insensitively, so a config's `com.apple.safari` matches Safari.
    apps: HashMap<String, usize>,
}

#[derive(Debug)]
pub struct Router {
    mappings: Vec<Mapping>,
    slots: HashMap<(String, u8, Trigger), Slot>,
}

impl Router {
    /// Indexes a validated config (which has no duplicate mappings).
    pub fn new(config: &Config) -> Self {
        let mut slots: HashMap<(String, u8, Trigger), Slot> = HashMap::new();
        for (i, m) in config.mappings.iter().enumerate() {
            let slot = slots
                .entry((m.device.clone(), m.control, m.trigger))
                .or_default();
            match &m.scope {
                Scope::Everywhere => slot.everywhere = Some(i),
                Scope::App(id) => {
                    slot.apps.insert(id.to_ascii_lowercase(), i);
                }
            }
        }
        Self {
            mappings: config.mappings.clone(),
            slots,
        }
    }

    /// The one mapping for this event, if any. `frontmost` is the frontmost
    /// app's bundle ID.
    pub fn resolve(
        &self,
        device_serial: &str,
        control: u8,
        trigger: Trigger,
        frontmost: Option<&str>,
    ) -> Option<&Mapping> {
        let slot = self
            .slots
            .get(&(device_serial.to_owned(), control, trigger))?;
        let app = frontmost.and_then(|id| slot.apps.get(&id.to_ascii_lowercase()));
        app.or(slot.everywhere.as_ref()).map(|&i| &self.mappings[i])
    }
}
