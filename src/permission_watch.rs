use crate::engine::ControlCommand;
use crate::permissions::ScreenRecordingStatus;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};

const PERMISSION_POLL_INTERVAL: Duration = Duration::from_secs(5);

pub fn spawn_permission_watch<F>(
    command_tx: UnboundedSender<ControlCommand>,
    notifier: F,
) -> Option<JoinHandle<()>>
where
    F: Fn(ScreenRecordingStatus) + Send + 'static,
{
    spawn_permission_watch_internal(command_tx, notifier)
}

#[cfg(target_os = "macos")]
fn spawn_permission_watch_internal<F>(
    command_tx: UnboundedSender<ControlCommand>,
    notifier: F,
) -> Option<JoinHandle<()>>
where
    F: Fn(ScreenRecordingStatus) + Send + 'static,
{
    use crate::permissions::screen_recording_status;

    let initial_status = screen_recording_status();
    if matches!(initial_status, ScreenRecordingStatus::NotSupported) {
        return None;
    }

    Some(tokio::spawn(async move {
        let mut last_status = initial_status;
        let mut auto_paused = false;
        loop {
            if command_tx.is_closed() {
                break;
            }

            sleep(PERMISSION_POLL_INTERVAL).await;

            if command_tx.is_closed() {
                break;
            }

            let status = screen_recording_status();
            if status == last_status {
                continue;
            }
            last_status = status;
            notifier(status);

            match status {
                ScreenRecordingStatus::Denied => {
                    if !auto_paused {
                        match command_tx.send(ControlCommand::Pause) {
                            Ok(()) => auto_paused = true,
                            Err(_) => break,
                        }
                    }
                }
                ScreenRecordingStatus::Granted => {
                    if auto_paused {
                        match command_tx.send(ControlCommand::Resume) {
                            Ok(()) => auto_paused = false,
                            Err(_) => break,
                        }
                    }
                }
                ScreenRecordingStatus::NotSupported => break,
            }
        }
    }))
}

#[cfg(not(target_os = "macos"))]
fn spawn_permission_watch_internal<F>(
    _command_tx: UnboundedSender<ControlCommand>,
    _notifier: F,
) -> Option<JoinHandle<()>>
where
    F: Fn(ScreenRecordingStatus) + Send + 'static,
{
    None
}
