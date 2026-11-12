//! What the app knows about each device model (its USB IDs) beyond what HELLO
//! says (D-081): the starter template (D-077). HELLO describes the controls,
//! push switches included (D-094). The models themselves are
//! `detent_protocol::ids::MODELS` (M7).

use detent_protocol::ids;

use crate::store::{
    Action, Mapping, Scope, Trigger, DEFAULT_APPLESCRIPT_TIMEOUT_MS, DEFAULT_VOLUME_STEP,
};

/// The model's starter template (D-077): mappings a newly paired unit can
/// start with, `device` left empty for the caller to fill. Empty for a model
/// without one.
pub fn starter_template(vid: u16, pid: u16) -> Vec<Mapping> {
    // By model (D-113); the IDs only name it. Only the XIAO has one: the
    // other boards' pins are still placeholders (M7).
    if ids::model(vid, pid).is_none_or(|m| m.name != "XIAO") {
        return Vec::new();
    }
    // Detent XIAO (was Detent Dev): Keith's setup, proven on hardware in M2
    // and M3.
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
