//! Open at login through SMAppService (macOS 13+, D-067). Registers this app
//! bundle itself as a login item; System Settings > General > Login Items
//! lists it under Detent's name and the user can switch it off there too.

use objc2_service_management::{SMAppService, SMAppServiceStatus};

/// Where Detent stands as a login item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginItem {
    /// Not registered: Detent doesn't open at login.
    Off,
    /// Registered and allowed: Detent opens at login.
    On,
    /// Registered, but the user must allow it in System Settings > Login Items.
    NeedsApproval,
    /// macOS can't find this app as a login item: an unbundled binary such
    /// as `cargo run`, or a bundle macOS doesn't know.
    Unavailable,
}

fn service() -> objc2::rc::Retained<SMAppService> {
    // SAFETY: a class method with no arguments that returns this app's own
    // service object; it has no preconditions.
    unsafe { SMAppService::mainAppService() }
}

/// The current state. Never prompts.
pub fn login_item() -> LoginItem {
    // SAFETY: reading a property of a valid service object.
    let status = unsafe { service().status() };
    match status {
        SMAppServiceStatus::Enabled => LoginItem::On,
        SMAppServiceStatus::RequiresApproval => LoginItem::NeedsApproval,
        SMAppServiceStatus::NotRegistered => LoginItem::Off,
        _ => LoginItem::Unavailable,
    }
}

/// Turns open at login on or off, returning the state that results. macOS
/// may answer `NeedsApproval` instead of `On` the first time.
pub fn set_login_item(on: bool) -> Result<LoginItem, String> {
    let s = service();
    // SAFETY: register and unregister take no arguments and report failure
    // through the returned NSError, which is converted to a message here.
    let result = unsafe {
        if on {
            s.registerAndReturnError()
        } else {
            s.unregisterAndReturnError()
        }
    };
    result.map_err(|e| e.localizedDescription().to_string())?;
    Ok(login_item())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Only reads: registering in a test would change the user's login items.
    #[test]
    fn status_reads_without_prompting() {
        let state = login_item();
        // An unbundled test binary isn't a registrable app.
        assert_ne!(state, LoginItem::On, "a test binary can't be a login item");
    }
}
