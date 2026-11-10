//! Keystrokes for Send shortcut (D-007, D-082) and Secure Input. Posting
//! needs the Accessibility grant; nothing here prompts. (System volume keys
//! were tried for the HUD and macOS 27 ignores them, D-083.)

use std::ffi::c_void;
use std::fmt;

use crate::keys::Combo;

/// An opaque `CGEventRef`.
#[repr(C)]
pub struct CGEvent {
    _private: [u8; 0],
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventCreateKeyboardEvent(source: *const c_void, keycode: u16, down: bool) -> *mut CGEvent;
    fn CGEventSetFlags(event: *mut CGEvent, flags: u64);
    fn CGEventPost(tap: u32, event: *mut CGEvent);
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *const c_void);
}

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    // Boolean IsSecureEventInputEnabled(void);
    fn IsSecureEventInputEnabled() -> u8;
}

/// `kCGHIDEventTap`: events enter as if from the keyboard.
const HID_TAP: u32 = 0;

const FLAG_SHIFT: u64 = 1 << 17;
const FLAG_CONTROL: u64 = 1 << 18;
const FLAG_OPTION: u64 = 1 << 19;
const FLAG_CMD: u64 = 1 << 20;

/// Modifier keys in press order, with their key codes and flags.
fn modifiers(c: &Combo) -> Vec<(u16, u64)> {
    let m = c.mods;
    [
        (m.control, 0x3B, FLAG_CONTROL),
        (m.option, 0x3A, FLAG_OPTION),
        (m.shift, 0x38, FLAG_SHIFT),
        (m.cmd, 0x37, FLAG_CMD),
    ]
    .into_iter()
    .filter(|(on, _, _)| *on)
    .map(|(_, code, flag)| (code, flag))
    .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyPostError {
    /// Detent doesn't hold the Accessibility grant.
    NotTrusted,
    /// A password field (or similar) has Secure Input on.
    SecureInput,
    /// CoreGraphics couldn't make an event.
    NoEvent,
}

impl fmt::Display for KeyPostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotTrusted => write!(f, "Detent needs Accessibility to send keys"),
            Self::SecureInput => write!(
                f,
                "Secure Input is on (a password field has focus), so macOS blocks sent keys"
            ),
            Self::NoEvent => write!(f, "macOS couldn't create the key event"),
        }
    }
}

impl std::error::Error for KeyPostError {}

/// Whether Secure Input is on anywhere: macOS then drops synthetic keys.
pub fn secure_input_on() -> bool {
    // SAFETY: takes no arguments and only reads system state.
    unsafe { IsSecureEventInputEnabled() != 0 }
}

/// Posts one key event with `flags` held.
fn post_key(code: u16, down: bool, flags: u64) -> Result<(), KeyPostError> {
    // SAFETY: a null source is allowed; the event is checked for null, used,
    // then released exactly once.
    unsafe {
        let event = CGEventCreateKeyboardEvent(std::ptr::null(), code, down);
        if event.is_null() {
            return Err(KeyPostError::NoEvent);
        }
        CGEventSetFlags(event, flags);
        CGEventPost(HID_TAP, event);
        CFRelease(event as *const c_void);
    }
    Ok(())
}

/// Sends combos to the frontmost app, in order. Checks the grant and Secure
/// Input first, so a refused sequence sends nothing at all.
pub fn post_combos(combos: &[Combo]) -> Result<(), KeyPostError> {
    if !super::accessibility_trusted() {
        return Err(KeyPostError::NotTrusted);
    }
    if secure_input_on() {
        return Err(KeyPostError::SecureInput);
    }
    for c in combos {
        let mods = modifiers(c);
        let mut flags = 0;
        // Real modifier key presses: some apps (Electron ones among them)
        // read the modifier keys, not just the flags on the key event.
        for (code, flag) in &mods {
            flags |= flag;
            post_key(*code, true, flags)?;
        }
        post_key(c.code, true, flags)?;
        post_key(c.code, false, flags)?;
        for (code, flag) in mods.iter().rev() {
            flags &= !flag;
            post_key(*code, false, flags)?;
        }
    }
    Ok(())
}
