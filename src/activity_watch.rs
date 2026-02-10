use crate::engine::{ControlCommand, PauseReason};
use crate::system_activity::{ScreenLockStatus, screen_lock_status};
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};

const ACTIVITY_POLL_INTERVAL: Duration = Duration::from_secs(2);

pub fn spawn_activity_watch(
    command_tx: UnboundedSender<ControlCommand>,
    notifier: impl Fn(ScreenLockStatus) + Send + 'static,
) -> Option<JoinHandle<()>> {
    spawn_activity_watch_internal(command_tx, notifier)
}

#[cfg(target_os = "macos")]
fn spawn_activity_watch_internal(
    command_tx: UnboundedSender<ControlCommand>,
    notifier: impl Fn(ScreenLockStatus) + Send + 'static,
) -> Option<JoinHandle<()>> {
    let initial_status = screen_lock_status();
    if matches!(initial_status, ScreenLockStatus::NotSupported) {
        return None;
    }

    Some(tokio::spawn(async move {
        let mut last_status = initial_status;
        loop {
            if command_tx.is_closed() {
                break;
            }

            sleep(ACTIVITY_POLL_INTERVAL).await;

            if command_tx.is_closed() {
                break;
            }

            let status = screen_lock_status();
            if status == last_status {
                continue;
            }
            last_status = status;
            notifier(status);

            match status {
                ScreenLockStatus::Locked => {
                    let _ = command_tx.send(ControlCommand::AutoPause(PauseReason::ScreenLocked));
                }
                ScreenLockStatus::Unlocked => {
                    let _ = command_tx.send(ControlCommand::AutoResume(PauseReason::ScreenLocked));
                }
                ScreenLockStatus::Unknown | ScreenLockStatus::NotSupported => {}
            }
        }
    }))
}

#[cfg(not(target_os = "macos"))]
fn spawn_activity_watch_internal(
    _command_tx: UnboundedSender<ControlCommand>,
    _notifier: impl Fn(ScreenLockStatus) + Send + 'static,
) -> Option<JoinHandle<()>> {
    None
}
