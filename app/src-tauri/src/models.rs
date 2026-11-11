//! What the app knows about each device model (its USB IDs) that HELLO
//! doesn't say (D-081). HELLO lists controls and kinds but not which button
//! is an encoder's push switch; the starter template (D-077) lands here too.
//! The models themselves are `detent_protocol::ids::MODELS` (M7).

use detent_protocol::ids;

use crate::store::{
    Action, Mapping, Scope, Trigger, DEFAULT_APPLESCRIPT_TIMEOUT_MS, DEFAULT_VOLUME_STEP,
};

/// The control ID of the encoder's push switch, if the model has one. In
/// Volume and Menu modes the device handles it, so it can't be mapped.
pub fn encoder_push(vid: u16, pid: u16) -> Option<u8> {
    // Per board: `firmware/include/boards/*.h`, `kEncoderPushControl`.
    ids::model(vid, pid).and_then(|m| m.encoder_push)
}

/// The model's starter template (D-077): mappings a newly paired unit can
/// start with, `device` left empty for the caller to fill. Empty for a model
/// without one.
pub fn starter_template(vid: u16, pid: u16) -> Vec<Mapping> {
    if (vid, pid) != (ids::VID, ids::PID) {
        return Vec::new();
    }
    // Detent Dev: Keith's setup, proven on hardware in M2 and M3.
    let press = |control, scope, action| Mapping {
        device: String::new(),
        control,
        trigger: Trigger::Press,
        scope,
        action,
    };
    vec![
        press(
            1,
            Scope::Everywhere,
            Action::OpenApp {
                bundle_id: "com.apple.mail".into(),
            },
        ),
        press(1, Scope::App("com.apple.Safari".into()), Action::MuteToggle),
        press(
            2,
            Scope::Everywhere,
            Action::VolumeUp {
                step: DEFAULT_VOLUME_STEP,
            },
        ),
        press(3, Scope::Everywhere, Action::MuteToggle),
        press(
            4,
            Scope::Everywhere,
            Action::AppleScript {
                source: "tell application \"Finder\"\n\
                         if (count Finder windows) is 0 then reopen\n\
                         activate\n\
                         end tell"
                    .into(),
                timeout_ms: DEFAULT_APPLESCRIPT_TIMEOUT_MS,
            },
        ),
    ]
}
