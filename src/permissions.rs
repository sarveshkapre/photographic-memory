use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenRecordingStatus {
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
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "failed to launch System Settings",
                    ))
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

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use super::{ScreenRecordingStatus, screen_recording_status};

    #[test]
    fn reports_not_supported_on_non_macos() {
        assert_eq!(
            screen_recording_status(),
            ScreenRecordingStatus::NotSupported
        );
    }
}
