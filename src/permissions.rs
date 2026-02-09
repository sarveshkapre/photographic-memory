use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenRecordingStatus {
    Granted,
    Denied,
    NotSupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibilityStatus {
    Granted,
    Denied,
    NotSupported,
}

impl ScreenRecordingStatus {
    pub fn is_granted(&self) -> bool {
        matches!(
            self,
            ScreenRecordingStatus::Granted | ScreenRecordingStatus::NotSupported
        )
    }

    pub fn needs_user_action(&self) -> bool {
        matches!(self, ScreenRecordingStatus::Denied)
    }
}

pub fn screen_recording_status() -> ScreenRecordingStatus {
    #[cfg(target_os = "macos")]
    unsafe {
        if CGPreflightScreenCaptureAccess() {
            ScreenRecordingStatus::Granted
        } else {
            ScreenRecordingStatus::Denied
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        ScreenRecordingStatus::NotSupported
    }
}

pub const SCREEN_RECORDING_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenRecording";

pub fn screen_recording_help_message() -> &'static str {
    "Grant Screen Recording permission: System Settings -> Privacy & Security -> Screen Recording. Toggle Photographic Memory on, then relaunch or re-check."
}

pub fn open_screen_recording_settings() -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(SCREEN_RECORDING_SETTINGS_URL)
            .status()
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(std::io::Error::other("failed to launch System Settings"))
                }
            })
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "screen recording permission not required",
        ))
    }
}

impl AccessibilityStatus {
    pub fn is_granted(&self) -> bool {
        matches!(
            self,
            AccessibilityStatus::Granted | AccessibilityStatus::NotSupported
        )
    }

    pub fn needs_user_action(&self) -> bool {
        matches!(self, AccessibilityStatus::Denied)
    }
}

pub fn accessibility_status() -> AccessibilityStatus {
    #[cfg(target_os = "macos")]
    unsafe {
        if AXIsProcessTrusted() {
            AccessibilityStatus::Granted
        } else {
            AccessibilityStatus::Denied
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        AccessibilityStatus::NotSupported
    }
}

pub const ACCESSIBILITY_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

pub fn accessibility_help_message() -> &'static str {
    "Grant Accessibility permission: System Settings -> Privacy & Security -> Accessibility. Toggle Photographic Memory on, then relaunch or re-check."
}

pub fn open_accessibility_settings() -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(ACCESSIBILITY_SETTINGS_URL)
            .status()
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(std::io::Error::other("failed to launch System Settings"))
                }
            })
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "accessibility permission not required",
        ))
    }
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn AXIsProcessTrusted() -> bool;
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use super::{
        AccessibilityStatus, ScreenRecordingStatus, accessibility_status, screen_recording_status,
    };

    #[test]
    fn reports_not_supported_on_non_macos() {
        assert_eq!(
            screen_recording_status(),
            ScreenRecordingStatus::NotSupported
        );
    }

    #[test]
    fn reports_accessibility_not_supported_on_non_macos() {
        assert_eq!(accessibility_status(), AccessibilityStatus::NotSupported);
    }
}
