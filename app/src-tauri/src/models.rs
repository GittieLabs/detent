//! What the app knows about each device model (PID) that HELLO doesn't say
//! (D-081). HELLO lists controls and kinds but not which button is an
//! encoder's push switch; the starter template (D-077) lands here too.

use detent_protocol::ids;

/// The control ID of the encoder's push switch, if the model has one. In
/// Volume and Menu modes the device handles it, so it can't be mapped.
pub fn encoder_push(pid: u16) -> Option<u8> {
    // Detent Dev: `firmware/include/board_pins.h`, `kEncoderPushControl`.
    (pid == ids::PID).then_some(6)
}
