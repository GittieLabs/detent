//! Accessibility (TCC) status. Needed later for Send shortcut (CGEventPost).

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    // Boolean AXIsProcessTrusted(void); `Boolean` is an unsigned char.
    fn AXIsProcessTrusted() -> u8;
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
