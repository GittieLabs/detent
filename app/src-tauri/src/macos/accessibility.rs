//! Accessibility (TCC): the grant Send shortcut needs (CGEventPost), and the
//! banner's Fix: macOS's prompt plus the Settings pane.

use std::ffi::c_void;

use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSString, NSURL};

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    // Boolean AXIsProcessTrusted(void); `Boolean` is an unsigned char.
    fn AXIsProcessTrusted() -> u8;
    // Boolean AXIsProcessTrustedWithOptions(CFDictionaryRef options);
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> u8;
    static kAXTrustedCheckOptionPrompt: *const c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFBooleanTrue: *const c_void;
    static kCFTypeDictionaryKeyCallBacks: [u8; 0];
    static kCFTypeDictionaryValueCallBacks: [u8; 0];
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        count: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> *const c_void;
    fn CFRelease(cf: *const c_void);
}

/// Where macOS lists apps allowed to control the computer.
const SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

/// Asks macOS to show its Accessibility prompt for Detent (if not already
/// granted) and opens Privacy & Security > Accessibility. Returns whether the
/// grant is held now.
pub fn request_accessibility() -> bool {
    // SAFETY: a one entry CFDictionary of CF constants with the standard
    // CFType callbacks; it's created, passed, and released exactly once.
    let granted = unsafe {
        let keys = [kAXTrustedCheckOptionPrompt];
        let values = [kCFBooleanTrue];
        let options = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            kCFTypeDictionaryKeyCallBacks.as_ptr().cast(),
            kCFTypeDictionaryValueCallBacks.as_ptr().cast(),
        );
        let granted = AXIsProcessTrustedWithOptions(options) != 0;
        if !options.is_null() {
            CFRelease(options);
        }
        granted
    };
    if !granted {
        if let Some(url) = NSURL::URLWithString(&NSString::from_str(SETTINGS_URL)) {
            NSWorkspace::sharedWorkspace().openURL(&url);
        }
    }
    granted
}

/// Whether this process holds the Accessibility grant. Never prompts.
pub fn accessibility_trusted() -> bool {
    // SAFETY: takes no arguments, has no preconditions, and only reads the
    // TCC state for the calling process.
    unsafe { AXIsProcessTrusted() != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_consistently_without_prompting() {
        // The value depends on the machine's TCC state, so only check that the
        // call returns and is stable. Granting is a manual step (acceptance.md).
        let first = accessibility_trusted();
        assert_eq!(first, accessibility_trusted());
        eprintln!("accessibility_trusted() = {first}");
    }
}
