#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenLockStatus {
    Locked,
    Unlocked,
    Unknown,
    NotSupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplaySleepStatus {
    Asleep,
    Awake,
    Unknown,
    NotSupported,
}

#[cfg(target_os = "macos")]
pub fn screen_lock_status() -> ScreenLockStatus {
    use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionaryRef;
    use core_foundation::string::CFString;

    unsafe {
        let dict: CFDictionaryRef = CGSessionCopyCurrentDictionary();
        if dict.is_null() {
            return ScreenLockStatus::Unknown;
        }

        let key = CFString::new("CGSSessionScreenIsLocked");
        let value: *const std::ffi::c_void =
            core_foundation::dictionary::CFDictionaryGetValue(dict, key.as_concrete_TypeRef() as _);

        let status = if value.is_null() {
            ScreenLockStatus::Unknown
        } else {
            // CGSessionCopyCurrentDictionary returns a CFDictionary with CFBoolean values for this key.
            let bool_ref = value as CFTypeRef;
            if CFBoolean::type_id() == core_foundation::base::CFGetTypeID(bool_ref) {
                if CFBoolean::wrap_under_get_rule(bool_ref as _).into() {
                    ScreenLockStatus::Locked
                } else {
                    ScreenLockStatus::Unlocked
                }
            } else {
                ScreenLockStatus::Unknown
            }
        };

        CFRelease(dict as _);
        status
    }
}

#[cfg(not(target_os = "macos"))]
pub fn screen_lock_status() -> ScreenLockStatus {
    ScreenLockStatus::NotSupported
}

#[cfg(target_os = "macos")]
pub fn display_sleep_status() -> DisplaySleepStatus {
    unsafe {
        let display = CGMainDisplayID();
        let asleep = CGDisplayIsAsleep(display);
        if asleep != 0 {
            DisplaySleepStatus::Asleep
        } else {
            DisplaySleepStatus::Awake
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn display_sleep_status() -> DisplaySleepStatus {
    DisplaySleepStatus::NotSupported
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGSessionCopyCurrentDictionary() -> core_foundation::dictionary::CFDictionaryRef;
}

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGMainDisplayID() -> u32;
    fn CGDisplayIsAsleep(display: u32) -> u32;
}
