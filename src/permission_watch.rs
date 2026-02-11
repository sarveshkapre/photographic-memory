use crate::engine::{ControlCommand, PauseReason};
use crate::permissions::ScreenRecordingStatus;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};

const PERMISSION_POLL_INTERVAL: Duration = Duration::from_secs(5);

trait PermissionProvider: Send + Sync + 'static {
    fn status(&self) -> ScreenRecordingStatus;
}

struct MacOsPermissionProvider;

impl PermissionProvider for MacOsPermissionProvider {
    fn status(&self) -> ScreenRecordingStatus {
        #[cfg(target_os = "macos")]
        {
            crate::permissions::screen_recording_status()
        }
        #[cfg(not(target_os = "macos"))]
        {
            ScreenRecordingStatus::NotSupported
        }
    }
}

pub fn spawn_permission_watch<F>(
    command_tx: UnboundedSender<ControlCommand>,
    notifier: F,
) -> Option<JoinHandle<()>>
where
    F: Fn(ScreenRecordingStatus) + Send + 'static,
{
    spawn_permission_watch_internal(
        command_tx,
        notifier,
        Arc::new(MacOsPermissionProvider),
        PERMISSION_POLL_INTERVAL,
    )
}

#[cfg(target_os = "macos")]
fn spawn_permission_watch_internal<F>(
    command_tx: UnboundedSender<ControlCommand>,
    notifier: F,
    provider: Arc<dyn PermissionProvider>,
    poll_interval: Duration,
) -> Option<JoinHandle<()>>
where
    F: Fn(ScreenRecordingStatus) + Send + 'static,
{
    let initial_status = provider.status();
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

            sleep(poll_interval).await;

            if command_tx.is_closed() {
                break;
            }

            let status = provider.status();
            if status == last_status {
                continue;
            }
            last_status = status;
            notifier(status);

            match status {
                ScreenRecordingStatus::Denied => {
                    if !auto_paused {
                        match command_tx
                            .send(ControlCommand::AutoPause(PauseReason::PermissionDenied))
                        {
                            Ok(()) => auto_paused = true,
                            Err(_) => break,
                        }
                    }
                }
                ScreenRecordingStatus::Granted => {
                    if auto_paused {
                        match command_tx
                            .send(ControlCommand::AutoResume(PauseReason::PermissionDenied))
                        {
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
    _provider: Arc<dyn PermissionProvider>,
    _poll_interval: Duration,
) -> Option<JoinHandle<()>>
where
    F: Fn(ScreenRecordingStatus) + Send + 'static,
{
    None
}

#[cfg(test)]
mod tests {
    use super::{PermissionProvider, spawn_permission_watch_internal};
    use crate::engine::{ControlCommand, PauseReason};
    use crate::permissions::ScreenRecordingStatus;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::sync::mpsc;
    use tokio::time::timeout;

    #[derive(Clone)]
    struct FakePermissionProvider {
        status: Arc<Mutex<ScreenRecordingStatus>>,
    }

    impl FakePermissionProvider {
        fn new(status: ScreenRecordingStatus) -> Self {
            Self {
                status: Arc::new(Mutex::new(status)),
            }
        }

        fn set_status(&self, status: ScreenRecordingStatus) {
            *self
                .status
                .lock()
                .expect("permission status mutex poisoned") = status;
        }
    }

    impl PermissionProvider for FakePermissionProvider {
        fn status(&self) -> ScreenRecordingStatus {
            *self
                .status
                .lock()
                .expect("permission status mutex poisoned")
        }
    }

    #[tokio::test]
    async fn sends_pause_resume_only_on_distinct_permission_transitions() {
        let provider = Arc::new(FakePermissionProvider::new(ScreenRecordingStatus::Granted));
        let (tx, mut rx) = mpsc::unbounded_channel::<ControlCommand>();

        let seen = Arc::new(Mutex::new(Vec::<ScreenRecordingStatus>::new()));
        let seen_clone = seen.clone();

        let handle = spawn_permission_watch_internal(
            tx,
            move |status| {
                seen_clone
                    .lock()
                    .expect("seen statuses mutex poisoned")
                    .push(status);
            },
            provider.clone(),
            Duration::from_millis(5),
        )
        .expect("watcher started");

        provider.set_status(ScreenRecordingStatus::Denied);
        let pause = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout waiting for pause")
            .expect("pause command");
        assert_eq!(
            pause,
            ControlCommand::AutoPause(PauseReason::PermissionDenied)
        );

        provider.set_status(ScreenRecordingStatus::Denied);
        let second = timeout(Duration::from_millis(50), rx.recv()).await;
        assert!(
            second.is_err(),
            "repeated denied status should not emit duplicate pause"
        );

        provider.set_status(ScreenRecordingStatus::Granted);
        let resume = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout waiting for resume")
            .expect("resume command");
        assert_eq!(
            resume,
            ControlCommand::AutoResume(PauseReason::PermissionDenied)
        );

        let seen = seen.lock().expect("seen statuses mutex poisoned").clone();
        assert_eq!(
            seen,
            vec![
                ScreenRecordingStatus::Denied,
                ScreenRecordingStatus::Granted
            ]
        );

        handle.abort();
        let _ = handle.await;
    }

    #[tokio::test]
    async fn returns_none_when_permission_checks_are_not_supported() {
        let provider = Arc::new(FakePermissionProvider::new(
            ScreenRecordingStatus::NotSupported,
        ));
        let (tx, _rx) = mpsc::unbounded_channel::<ControlCommand>();

        let handle =
            spawn_permission_watch_internal(tx, |_| {}, provider, Duration::from_millis(5));
        assert!(handle.is_none());
    }
}
