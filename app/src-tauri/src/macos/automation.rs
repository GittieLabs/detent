//! Automation (TCC for Apple Events): whether Detent may send Apple Events
//! to an app, asked without ever showing the prompt (M9, D-106). The
//! osascript child Detent starts is attributed to Detent, so this is the
//! consent its scripts get.

use std::ffi::c_void;

use crate::actions::applescript::Consent;

#[repr(C)]
struct AEDesc {
    descriptor_type: u32,
    data_handle: *mut c_void,
}

#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    // OSErr AECreateDesc(DescType, const void *, Size, AEDesc *);
    fn AECreateDesc(type_code: u32, data: *const c_void, size: isize, result: *mut AEDesc) -> i16;
    // OSErr AEDisposeDesc(AEDesc *);
    fn AEDisposeDesc(desc: *mut AEDesc) -> i16;
    // OSStatus AEDeterminePermissionToAutomateTarget(const AEAddressDesc *,
    //     AEEventClass, AEEventID, Boolean askUserIfNeeded);
    fn AEDeterminePermissionToAutomateTarget(
        target: *const AEDesc,
        event_class: u32,
        event_id: u32,
        ask_user_if_needed: u8,
    ) -> i32;
}

const TYPE_APPLICATION_BUNDLE_ID: u32 = u32::from_be_bytes(*b"bund");
const TYPE_WILD_CARD: u32 = u32::from_be_bytes(*b"****");
const ERR_AE_EVENT_NOT_PERMITTED: i32 = -1743;
const ERR_AE_EVENT_WOULD_REQUIRE_USER_CONSENT: i32 = -1744;

/// Detent's Automation consent for the app with `bundle_id`. Never
/// prompts. An app that isn't running (`procNotFound`) is `Unknown`.
pub fn automation_consent(bundle_id: &str) -> Consent {
    let mut desc = AEDesc {
        descriptor_type: 0,
        data_handle: std::ptr::null_mut(),
    };
    // SAFETY: the bytes outlive the call, which copies them into `desc`.
    let made = unsafe {
        AECreateDesc(
            TYPE_APPLICATION_BUNDLE_ID,
            bundle_id.as_ptr().cast(),
            bundle_id.len() as isize,
            &mut desc,
        )
    };
    if made != 0 {
        return Consent::Unknown;
    }
    // SAFETY: `desc` was made above; ask_user_if_needed is false, so this
    // never shows UI. Any event class and ID ('****').
    let status =
        unsafe { AEDeterminePermissionToAutomateTarget(&desc, TYPE_WILD_CARD, TYPE_WILD_CARD, 0) };
    // SAFETY: disposes the descriptor made above, once.
    unsafe { AEDisposeDesc(&mut desc) };
    match status {
        0 => Consent::Granted,
        ERR_AE_EVENT_NOT_PERMITTED => Consent::Denied,
        ERR_AE_EVENT_WOULD_REQUIRE_USER_CONSENT => Consent::NotAsked,
        _ => Consent::Unknown,
    }
}
