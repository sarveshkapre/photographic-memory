use crate::engine::{ControlCommand, PauseReason};
use crate::system_activity::{
    DisplaySleepStatus, ScreenLockStatus, display_sleep_status, screen_lock_status,
};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};

const ACTIVITY_POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityEvent {
    ScreenLock(ScreenLockStatus),
    DisplaySleep(DisplaySleepStatus),
}

trait ActivityProvider: Send + Sync + 'static {
    fn lock_status(&self) -> ScreenLockStatus;
    fn display_sleep_status(&self) -> DisplaySleepStatus;
}

struct MacOsActivityProvider;

impl ActivityProvider for MacOsActivityProvider {
    fn lock_status(&self) -> ScreenLockStatus {
        screen_lock_status()
    }

    fn display_sleep_status(&self) -> DisplaySleepStatus {
        display_sleep_status()
    }
}

pub fn spawn_activity_watch(
    command_tx: UnboundedSender<ControlCommand>,
    notifier: impl Fn(ActivityEvent) + Send + 'static,
) -> Option<JoinHandle<()>> {
    spawn_activity_watch_internal(
        command_tx,
        notifier,
        Arc::new(MacOsActivityProvider),
        ACTIVITY_POLL_INTERVAL,
    )
}

#[cfg(target_os = "macos")]
fn spawn_activity_watch_internal(
    command_tx: UnboundedSender<ControlCommand>,
    notifier: impl Fn(ActivityEvent) + Send + 'static,
    provider: Arc<dyn ActivityProvider>,
    poll_interval: Duration,
) -> Option<JoinHandle<()>> {
    let initial_lock = provider.lock_status();
    let initial_display = provider.display_sleep_status();

    if matches!(initial_lock, ScreenLockStatus::NotSupported)
        && matches!(initial_display, DisplaySleepStatus::NotSupported)
    {
        return None;
    }

    Some(tokio::spawn(async move {
        let mut last_lock = initial_lock;
        let mut last_display = initial_display;

        if matches!(initial_lock, ScreenLockStatus::Locked) {
            let _ = command_tx.send(ControlCommand::AutoPause(PauseReason::ScreenLocked));
        }
        if matches!(initial_display, DisplaySleepStatus::Asleep) {
            let _ = command_tx.send(ControlCommand::AutoPause(PauseReason::DisplayAsleep));
        }

        loop {
            if command_tx.is_closed() {
                break;
            }

            sleep(poll_interval).await;

            if command_tx.is_closed() {
                break;
            }

            let lock_status = provider.lock_status();
            if !matches!(
                lock_status,
                ScreenLockStatus::Unknown | ScreenLockStatus::NotSupported
            ) && lock_status != last_lock
            {
                last_lock = lock_status;
                notifier(ActivityEvent::ScreenLock(lock_status));

                match lock_status {
                    ScreenLockStatus::Locked => {
                        let _ =
                            command_tx.send(ControlCommand::AutoPause(PauseReason::ScreenLocked));
                    }
                    ScreenLockStatus::Unlocked => {
                        let _ =
                            command_tx.send(ControlCommand::AutoResume(PauseReason::ScreenLocked));
                    }
                    ScreenLockStatus::Unknown | ScreenLockStatus::NotSupported => {}
                }
            }

            let display_status = provider.display_sleep_status();
            if !matches!(
                display_status,
                DisplaySleepStatus::Unknown | DisplaySleepStatus::NotSupported
            ) && display_status != last_display
            {
                last_display = display_status;
                notifier(ActivityEvent::DisplaySleep(display_status));

                match display_status {
                    DisplaySleepStatus::Asleep => {
                        let _ =
                            command_tx.send(ControlCommand::AutoPause(PauseReason::DisplayAsleep));
                    }
                    DisplaySleepStatus::Awake => {
                        let _ =
                            command_tx.send(ControlCommand::AutoResume(PauseReason::DisplayAsleep));
                    }
                    DisplaySleepStatus::Unknown | DisplaySleepStatus::NotSupported => {}
                }
            }
        }
    }))
}

#[cfg(not(target_os = "macos"))]
fn spawn_activity_watch_internal(
    _command_tx: UnboundedSender<ControlCommand>,
    _notifier: impl Fn(ActivityEvent) + Send + 'static,
    _provider: Arc<dyn ActivityProvider>,
    _poll_interval: Duration,
) -> Option<JoinHandle<()>> {
    None
}

#[cfg(test)]
mod tests {
    use super::{ActivityEvent, ActivityProvider, spawn_activity_watch_internal};
    use crate::engine::{ControlCommand, PauseReason};
    use crate::system_activity::{DisplaySleepStatus, ScreenLockStatus};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::sync::mpsc;

    #[derive(Clone)]
    struct FakeProvider {
        lock: Arc<Mutex<ScreenLockStatus>>,
        display: Arc<Mutex<DisplaySleepStatus>>,
    }

    impl FakeProvider {
        fn new(lock: ScreenLockStatus, display: DisplaySleepStatus) -> Self {
            Self {
                lock: Arc::new(Mutex::new(lock)),
                display: Arc::new(Mutex::new(display)),
            }
        }

        fn set_lock(&self, status: ScreenLockStatus) {
            *self.lock.lock().expect("lock mutex poisoned") = status;
        }

        fn set_display(&self, status: DisplaySleepStatus) {
            *self.display.lock().expect("display mutex poisoned") = status;
        }
    }

    impl ActivityProvider for FakeProvider {
        fn lock_status(&self) -> ScreenLockStatus {
            *self.lock.lock().expect("lock mutex poisoned")
        }

        fn display_sleep_status(&self) -> DisplaySleepStatus {
            *self.display.lock().expect("display mutex poisoned")
        }
    }

    #[tokio::test]
    async fn auto_pauses_on_initial_locked_and_display_asleep() {
        let provider = Arc::new(FakeProvider::new(
            ScreenLockStatus::Locked,
            DisplaySleepStatus::Asleep,
        ));
        let (tx, mut rx) = mpsc::unbounded_channel::<ControlCommand>();

        let seen = Arc::new(Mutex::new(Vec::<ActivityEvent>::new()));
        let seen_clone = seen.clone();
        let handle = spawn_activity_watch_internal(
            tx,
            move |ev| seen_clone.lock().expect("seen mutex poisoned").push(ev),
            provider,
            Duration::from_millis(5),
        )
        .expect("watcher started");

        let mut got = Vec::new();
        for _ in 0..2 {
            let cmd = tokio::time::timeout(Duration::from_secs(1), rx.recv())
                .await
                .expect("timeout waiting for command")
                .expect("command");
            got.push(cmd);
        }

        assert!(got.contains(&ControlCommand::AutoPause(PauseReason::ScreenLocked)));
        assert!(got.contains(&ControlCommand::AutoPause(PauseReason::DisplayAsleep)));

        handle.abort();
        let _ = handle.await;
    }

    #[tokio::test]
    async fn auto_resumes_on_transitions() {
        let provider = Arc::new(FakeProvider::new(
            ScreenLockStatus::Unlocked,
            DisplaySleepStatus::Awake,
        ));
        let (tx, mut rx) = mpsc::unbounded_channel::<ControlCommand>();

        let handle =
            spawn_activity_watch_internal(tx, |_| {}, provider.clone(), Duration::from_millis(5))
                .expect("watcher started");

        provider.set_lock(ScreenLockStatus::Locked);
        let cmd = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout waiting for pause")
            .expect("command");
        assert_eq!(cmd, ControlCommand::AutoPause(PauseReason::ScreenLocked));

        provider.set_lock(ScreenLockStatus::Unlocked);
        let cmd = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout waiting for resume")
            .expect("command");
        assert_eq!(cmd, ControlCommand::AutoResume(PauseReason::ScreenLocked));

        provider.set_display(DisplaySleepStatus::Asleep);
        let cmd = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout waiting for display pause")
            .expect("command");
        assert_eq!(cmd, ControlCommand::AutoPause(PauseReason::DisplayAsleep));

        provider.set_display(DisplaySleepStatus::Awake);
        let cmd = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout waiting for display resume")
            .expect("command");
        assert_eq!(cmd, ControlCommand::AutoResume(PauseReason::DisplayAsleep));

        handle.abort();
        let _ = handle.await;
    }
}
