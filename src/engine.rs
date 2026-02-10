use crate::analysis::{AnalysisResult, Analyzer};
use crate::context_log::{ContextEntry, ContextLog};
use crate::privacy::{CaptureDecision, PrivacyGuard};
use crate::scheduler::{CaptureSchedule, Scheduler};
use crate::screenshot::ScreenshotProvider;
use crate::storage::{ReclaimOutcome, ensure_disk_headroom, reclaim_disk_space};
use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PauseReason {
    PermissionDenied,
    ScreenLocked,
    DisplayAsleep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCommand {
    UserPause,
    UserResume,
    AutoPause(PauseReason),
    AutoResume(PauseReason),
    Stop,
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    Started,
    Paused,
    Resumed,
    AutoPaused {
        reason: PauseReason,
    },
    AutoResumed {
        reason: PauseReason,
    },
    CaptureSkipped {
        tick_index: u64,
        reason: String,
    },
    CaptureSucceeded {
        capture_index: u64,
        path: PathBuf,
    },
    CaptureFailed {
        capture_index: u64,
        message: String,
    },
    DiskCleanup {
        deleted_files: usize,
        freed_bytes: u64,
        remaining_bytes: u64,
    },
    BudgetExceeded {
        bytes_written: u64,
        limit_bytes: u64,
    },
    Stopped,
    Completed {
        total_ticks: u64,
        captures: u64,
        skipped: u64,
        failures: u64,
    },
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub output_dir: PathBuf,
    pub filename_prefix: String,
    pub schedule: CaptureSchedule,
    pub min_free_disk_bytes: u64,
    /// Only attempt a real capture every N scheduler ticks.
    ///
    /// Intended as a safeguard for high-frequency schedules (e.g. 30ms) to avoid runaway disk churn.
    /// A value of 1 captures on every tick.
    pub capture_stride: u64,
    /// Optional session-level cap for bytes written to the output directory.
    ///
    /// This is a best-effort guardrail (measured via `metadata.len()` of each written capture file).
    pub max_session_bytes: Option<u64>,
}

pub const DEFAULT_MIN_FREE_DISK_BYTES: u64 = 1_073_741_824; // 1 GiB

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EngineSummary {
    pub total_ticks: u64,
    pub captures: u64,
    pub skipped: u64,
    pub failures: u64,
}

pub struct CaptureEngine {
    screenshot_provider: Arc<dyn ScreenshotProvider>,
    analyzer: Arc<dyn Analyzer>,
    privacy_guard: Arc<dyn PrivacyGuard>,
    context_log: ContextLog,
}

impl CaptureEngine {
    pub fn new(
        screenshot_provider: Arc<dyn ScreenshotProvider>,
        analyzer: Arc<dyn Analyzer>,
        privacy_guard: Arc<dyn PrivacyGuard>,
        context_log: ContextLog,
    ) -> Self {
        Self {
            screenshot_provider,
            analyzer,
            privacy_guard,
            context_log,
        }
    }

    pub async fn run(
        &self,
        config: EngineConfig,
        mut command_rx: Option<mpsc::UnboundedReceiver<ControlCommand>>,
        event_tx: Option<mpsc::UnboundedSender<EngineEvent>>,
    ) -> Result<EngineSummary> {
        std::fs::create_dir_all(&config.output_dir).with_context(|| {
            format!(
                "failed to create output directory {}",
                config.output_dir.display()
            )
        })?;

        let mut scheduler = Scheduler::new(config.schedule.clone()).map_err(anyhow::Error::msg)?;
        let start = tokio::time::Instant::now();
        let mut user_paused = false;
        let mut auto_pauses: BTreeSet<PauseReason> = BTreeSet::new();
        let mut summary = EngineSummary::default();
        let mut schedule_ticks: u64 = 0;
        let capture_stride = config.capture_stride.max(1);
        let mut bytes_written: u64 = 0;

        send_event(&event_tx, EngineEvent::Started);

        loop {
            while let Some(rx) = command_rx.as_mut() {
                match rx.try_recv() {
                    Ok(cmd) => {
                        let was_paused = effective_paused(user_paused, &auto_pauses);
                        let command_result =
                            handle_command(cmd, &mut user_paused, &mut auto_pauses, &event_tx);
                        if !effective_paused(user_paused, &auto_pauses) && was_paused {
                            scheduler.align_next_due(start.elapsed());
                        }

                        if command_result {
                            send_event(
                                &event_tx,
                                EngineEvent::Completed {
                                    total_ticks: summary.total_ticks,
                                    captures: summary.captures,
                                    skipped: summary.skipped,
                                    failures: summary.failures,
                                },
                            );
                            return Ok(summary);
                        }
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        command_rx = None;
                        break;
                    }
                }
            }

            if effective_paused(user_paused, &auto_pauses) {
                if let Some(rx) = command_rx.as_mut() {
                    match rx.recv().await {
                        Some(cmd) => {
                            let was_paused = effective_paused(user_paused, &auto_pauses);
                            let command_result =
                                handle_command(cmd, &mut user_paused, &mut auto_pauses, &event_tx);
                            if !effective_paused(user_paused, &auto_pauses) && was_paused {
                                scheduler.align_next_due(start.elapsed());
                            }

                            if command_result {
                                send_event(
                                    &event_tx,
                                    EngineEvent::Completed {
                                        total_ticks: summary.total_ticks,
                                        captures: summary.captures,
                                        skipped: summary.skipped,
                                        failures: summary.failures,
                                    },
                                );
                                return Ok(summary);
                            }
                        }
                        None => {
                            command_rx = None;
                            user_paused = false;
                            auto_pauses.clear();
                            scheduler.align_next_due(start.elapsed());
                        }
                    }
                } else {
                    // If there is no command channel, there is no way to resume. Prefer forward progress.
                    user_paused = false;
                    auto_pauses.clear();
                    scheduler.align_next_due(start.elapsed());
                }
                continue;
            }

            let elapsed = start.elapsed();
            if scheduler.is_finished(elapsed) {
                send_event(
                    &event_tx,
                    EngineEvent::Completed {
                        total_ticks: summary.total_ticks,
                        captures: summary.captures,
                        skipped: summary.skipped,
                        failures: summary.failures,
                    },
                );
                return Ok(summary);
            }

            if scheduler.should_capture(elapsed) {
                schedule_ticks += 1;
                if capture_stride > 1 && !(schedule_ticks - 1).is_multiple_of(capture_stride) {
                    scheduler.mark_captured();
                    continue;
                }

                summary.total_ticks += 1;
                let tick_index = summary.total_ticks;

                match self.privacy_guard.decision().await {
                    CaptureDecision::Allow => {
                        let capture_index = summary.captures + summary.failures + 1;
                        let capture_result =
                            self.capture_once(capture_index, &config, &event_tx).await;

                        match capture_result {
                            Ok(path) => {
                                summary.captures += 1;
                                if let Ok(metadata) = std::fs::metadata(&path) {
                                    bytes_written = bytes_written.saturating_add(metadata.len());
                                }
                                send_event(
                                    &event_tx,
                                    EngineEvent::CaptureSucceeded {
                                        capture_index,
                                        path,
                                    },
                                );

                                if let Some(limit) = config.max_session_bytes
                                    && bytes_written > limit
                                {
                                    send_event(
                                        &event_tx,
                                        EngineEvent::BudgetExceeded {
                                            bytes_written,
                                            limit_bytes: limit,
                                        },
                                    );
                                    send_event(
                                        &event_tx,
                                        EngineEvent::Completed {
                                            total_ticks: summary.total_ticks,
                                            captures: summary.captures,
                                            skipped: summary.skipped,
                                            failures: summary.failures,
                                        },
                                    );
                                    return Ok(summary);
                                }
                            }
                            Err(err) => {
                                summary.failures += 1;
                                send_event(
                                    &event_tx,
                                    EngineEvent::CaptureFailed {
                                        capture_index,
                                        message: err.to_string(),
                                    },
                                );
                            }
                        }
                    }
                    CaptureDecision::Skip { reason } => {
                        summary.skipped += 1;
                        let timestamp = Utc::now();
                        let _ = self
                            .context_log
                            .append_skipped(tick_index, timestamp, &reason);
                        send_event(
                            &event_tx,
                            EngineEvent::CaptureSkipped { tick_index, reason },
                        );
                    }
                }

                scheduler.mark_captured();
                continue;
            }

            if let Some(delay) = scheduler.time_until_next_capture(elapsed) {
                let delay = delay.min(Duration::from_millis(200));
                if let Some(rx) = command_rx.as_mut() {
                    tokio::select! {
                        cmd = rx.recv() => {
                            if let Some(cmd) = cmd {
                                let was_paused = effective_paused(user_paused, &auto_pauses);
                                let command_result = handle_command(
                                    cmd,
                                    &mut user_paused,
                                    &mut auto_pauses,
                                    &event_tx,
                                );
                                if !effective_paused(user_paused, &auto_pauses) && was_paused {
                                    scheduler.align_next_due(start.elapsed());
                                }

                                if command_result {
                                    send_event(&event_tx, EngineEvent::Completed {
                                        total_ticks: summary.total_ticks,
                                        captures: summary.captures,
                                        skipped: summary.skipped,
                                        failures: summary.failures,
                                    });
                                    return Ok(summary);
                                }
                            } else {
                                command_rx = None;
                            }
                        }
                        _ = tokio::time::sleep(delay) => {}
                    }
                } else {
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    async fn capture_once(
        &self,
        index: u64,
        config: &EngineConfig,
        event_tx: &Option<mpsc::UnboundedSender<EngineEvent>>,
    ) -> Result<PathBuf> {
        let cleanup = self.ensure_disk_guard(config)?;
        if let Some(outcome) = cleanup {
            if event_tx.is_some() {
                send_event(
                    event_tx,
                    EngineEvent::DiskCleanup {
                        deleted_files: outcome.deleted_files,
                        freed_bytes: outcome.freed_bytes,
                        remaining_bytes: outcome.remaining_bytes,
                    },
                );
            } else {
                eprintln!(
                    "Disk guard reclaimed {} files ({:.1} MB freed).",
                    outcome.deleted_files,
                    outcome.freed_bytes as f64 / (1024.0 * 1024.0)
                );
            }
        }
        let timestamp = Utc::now();
        let filename = format!(
            "{}-{}-{:06}.png",
            config.filename_prefix,
            timestamp.format("%Y%m%dT%H%M%S%.3fZ"),
            index
        );
        let path = config.output_dir.join(filename);

        self.screenshot_provider
            .capture(&path)
            .await
            .with_context(|| format!("capture {} failed", index))?;

        let analysis = self
            .analyzer
            .analyze(&path)
            .await
            .with_context(|| format!("analysis {} failed", index))
            .unwrap_or_else(|error| AnalysisResult {
                summary: format!("Analysis failed for {}: {}", path.display(), error),
            });

        self.context_log.append(&ContextEntry {
            capture_index: index,
            timestamp,
            image_path: path.clone(),
            summary: analysis.summary,
        })?;

        Ok(path)
    }
}

impl CaptureEngine {
    fn ensure_disk_guard(&self, config: &EngineConfig) -> Result<Option<ReclaimOutcome>> {
        match ensure_disk_headroom(&config.output_dir, config.min_free_disk_bytes) {
            Ok(()) => Ok(None),
            Err(err) => {
                if config.min_free_disk_bytes == 0 {
                    return Err(err);
                }

                match reclaim_disk_space(&config.output_dir, config.min_free_disk_bytes) {
                    Ok(outcome) => {
                        match ensure_disk_headroom(&config.output_dir, config.min_free_disk_bytes) {
                            Ok(()) => {
                                if outcome.deleted_files > 0 {
                                    Ok(Some(outcome))
                                } else {
                                    Ok(None)
                                }
                            }
                            Err(second_err) => Err(second_err),
                        }
                    }
                    Err(cleanup_err) => {
                        Err(err.context(format!("auto-cleanup attempt failed: {}", cleanup_err)))
                    }
                }
            }
        }
    }
}

fn handle_command(
    cmd: ControlCommand,
    user_paused: &mut bool,
    auto_pauses: &mut BTreeSet<PauseReason>,
    event_tx: &Option<mpsc::UnboundedSender<EngineEvent>>,
) -> bool {
    match cmd {
        ControlCommand::UserPause => {
            if !*user_paused {
                *user_paused = true;
                send_event(event_tx, EngineEvent::Paused);
            }
            false
        }
        ControlCommand::UserResume => {
            if *user_paused {
                *user_paused = false;
                send_event(event_tx, EngineEvent::Resumed);
            }
            false
        }
        ControlCommand::AutoPause(reason) => {
            if auto_pauses.insert(reason) {
                send_event(event_tx, EngineEvent::AutoPaused { reason });
            }
            false
        }
        ControlCommand::AutoResume(reason) => {
            if auto_pauses.remove(&reason) {
                send_event(event_tx, EngineEvent::AutoResumed { reason });
            }
            false
        }
        ControlCommand::Stop => {
            send_event(event_tx, EngineEvent::Stopped);
            true
        }
    }
}

fn effective_paused(user_paused: bool, auto_pauses: &BTreeSet<PauseReason>) -> bool {
    user_paused || !auto_pauses.is_empty()
}

fn send_event(event_tx: &Option<mpsc::UnboundedSender<EngineEvent>>, event: EngineEvent) {
    if let Some(tx) = event_tx {
        let _ = tx.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::{CaptureEngine, ControlCommand, EngineConfig};
    use crate::analysis::MetadataAnalyzer;
    use crate::context_log::ContextLog;
    use crate::privacy::{AllowAllPrivacyGuard, CaptureDecision, PrivacyGuard, PrivacyStatus};
    use crate::scheduler::CaptureSchedule;
    use crate::screenshot::{MockScreenshotProvider, ScreenshotProvider};
    use anyhow::{Result, anyhow};
    use async_trait::async_trait;
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Duration;
    use tempfile::tempdir;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn captures_expected_number_of_frames() {
        let temp = tempdir().expect("tempdir");
        let context = ContextLog::new(temp.path().join("context.md"));

        let engine = CaptureEngine::new(
            Arc::new(MockScreenshotProvider),
            Arc::new(MetadataAnalyzer),
            Arc::new(AllowAllPrivacyGuard::default()),
            context,
        );

        let summary = engine
            .run(
                EngineConfig {
                    output_dir: temp.path().join("captures"),
                    filename_prefix: "test".to_string(),
                    schedule: CaptureSchedule {
                        every: Duration::from_millis(80),
                        run_for: Duration::from_millis(330),
                    },
                    min_free_disk_bytes: 0,
                    capture_stride: 1,
                    max_session_bytes: None,
                },
                None,
                None,
            )
            .await
            .expect("engine run");

        assert_eq!(summary.total_ticks, 5);
        assert_eq!(summary.captures, 5);
        assert_eq!(summary.skipped, 0);
        assert_eq!(summary.failures, 0);

        let capture_count = std::fs::read_dir(temp.path().join("captures"))
            .expect("captures dir")
            .count();
        assert_eq!(capture_count, 5);
    }

    #[tokio::test]
    async fn capture_stride_throttles_capture_attempts() {
        let temp = tempdir().expect("tempdir");
        let context = ContextLog::new(temp.path().join("context.md"));

        let engine = CaptureEngine::new(
            Arc::new(MockScreenshotProvider),
            Arc::new(MetadataAnalyzer),
            Arc::new(AllowAllPrivacyGuard::default()),
            context,
        );

        let summary = engine
            .run(
                EngineConfig {
                    output_dir: temp.path().join("captures"),
                    filename_prefix: "test".to_string(),
                    schedule: CaptureSchedule {
                        every: Duration::from_millis(30),
                        run_for: Duration::from_millis(250),
                    },
                    min_free_disk_bytes: 0,
                    capture_stride: 10,
                    max_session_bytes: None,
                },
                None,
                None,
            )
            .await
            .expect("engine run");

        assert_eq!(summary.total_ticks, 1);
        assert_eq!(summary.captures, 1);
        assert_eq!(summary.skipped, 0);
        assert_eq!(summary.failures, 0);

        let capture_count = std::fs::read_dir(temp.path().join("captures"))
            .expect("captures dir")
            .count();
        assert_eq!(capture_count, 1);
    }

    #[derive(Debug, Default, Clone, Copy)]
    struct AlwaysSkipPrivacyGuard;

    #[async_trait]
    impl PrivacyGuard for AlwaysSkipPrivacyGuard {
        async fn decision(&self) -> CaptureDecision {
            CaptureDecision::Skip {
                reason: "privacy: test skip".to_string(),
            }
        }

        fn status(&self) -> PrivacyStatus {
            PrivacyStatus {
                config_path: std::path::PathBuf::from("privacy.toml"),
                enabled: true,
                rule_summary: "test".to_string(),
            }
        }

        fn reload(&self) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn skipped_ticks_do_not_create_capture_files() {
        let temp = tempdir().expect("tempdir");
        let context_path = temp.path().join("context.md");
        let context = ContextLog::new(&context_path);

        let engine = CaptureEngine::new(
            Arc::new(MockScreenshotProvider),
            Arc::new(MetadataAnalyzer),
            Arc::new(AlwaysSkipPrivacyGuard),
            context,
        );

        let summary = engine
            .run(
                EngineConfig {
                    output_dir: temp.path().join("captures"),
                    filename_prefix: "test".to_string(),
                    schedule: CaptureSchedule {
                        every: Duration::from_millis(60),
                        run_for: Duration::from_millis(190),
                    },
                    min_free_disk_bytes: 0,
                    capture_stride: 1,
                    max_session_bytes: None,
                },
                None,
                None,
            )
            .await
            .expect("engine run");

        assert_eq!(summary.total_ticks, 4);
        assert_eq!(summary.captures, 0);
        assert_eq!(summary.skipped, 4);
        assert_eq!(summary.failures, 0);

        let capture_dir = temp.path().join("captures");
        let capture_count = std::fs::read_dir(&capture_dir)
            .map(|dir| dir.count())
            .unwrap_or(0);
        assert_eq!(capture_count, 0);

        let content = std::fs::read_to_string(&context_path).expect("context exists");
        assert!(content.contains("## Skipped tick 1"));
        assert!(content.contains("Reason: privacy: test skip"));
    }

    #[tokio::test]
    async fn stop_command_ends_session() {
        let temp = tempdir().expect("tempdir");
        let context = ContextLog::new(temp.path().join("context.md"));

        let engine = CaptureEngine::new(
            Arc::new(MockScreenshotProvider),
            Arc::new(MetadataAnalyzer),
            Arc::new(AllowAllPrivacyGuard::default()),
            context,
        );

        let (tx, rx) = mpsc::unbounded_channel();
        let task = tokio::spawn(async move {
            engine
                .run(
                    EngineConfig {
                        output_dir: temp.path().join("captures"),
                        filename_prefix: "test".to_string(),
                        schedule: CaptureSchedule {
                            every: Duration::from_secs(1),
                            run_for: Duration::from_secs(30),
                        },
                        min_free_disk_bytes: 0,
                        capture_stride: 1,
                        max_session_bytes: None,
                    },
                    Some(rx),
                    None,
                )
                .await
                .expect("engine run")
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        tx.send(ControlCommand::Stop).expect("stop command");

        let summary = task.await.expect("task join");
        assert_eq!(summary.total_ticks, 1);
        assert_eq!(summary.captures, 1);
    }

    #[derive(Debug, Default, Clone, Copy)]
    struct FailingScreenshotProvider;

    #[async_trait]
    impl ScreenshotProvider for FailingScreenshotProvider {
        async fn capture(&self, _output_path: &Path) -> Result<()> {
            Err(anyhow!("intentional screenshot failure"))
        }
    }

    #[tokio::test]
    async fn screenshot_failures_are_counted_without_crashing_session() {
        let temp = tempdir().expect("tempdir");
        let context = ContextLog::new(temp.path().join("context.md"));
        let engine = CaptureEngine::new(
            Arc::new(FailingScreenshotProvider),
            Arc::new(MetadataAnalyzer),
            Arc::new(AllowAllPrivacyGuard::default()),
            context,
        );

        let summary = engine
            .run(
                EngineConfig {
                    output_dir: temp.path().join("captures"),
                    filename_prefix: "test".to_string(),
                    schedule: CaptureSchedule {
                        every: Duration::from_millis(40),
                        run_for: Duration::from_millis(130),
                    },
                    min_free_disk_bytes: 0,
                    capture_stride: 1,
                    max_session_bytes: None,
                },
                None,
                None,
            )
            .await
            .expect("engine run");

        assert_eq!(summary.total_ticks, 4);
        assert_eq!(summary.captures, 0);
        assert_eq!(summary.skipped, 0);
        assert_eq!(summary.failures, 4);
    }

    #[tokio::test]
    async fn context_log_write_failures_are_counted() {
        let temp = tempdir().expect("tempdir");
        let context_dir = temp.path().join("context.md");
        std::fs::create_dir_all(&context_dir).expect("context dir");
        let context = ContextLog::new(&context_dir);
        let engine = CaptureEngine::new(
            Arc::new(MockScreenshotProvider),
            Arc::new(MetadataAnalyzer),
            Arc::new(AllowAllPrivacyGuard::default()),
            context,
        );

        let summary = engine
            .run(
                EngineConfig {
                    output_dir: temp.path().join("captures"),
                    filename_prefix: "test".to_string(),
                    schedule: CaptureSchedule {
                        every: Duration::from_millis(60),
                        run_for: Duration::from_millis(125),
                    },
                    min_free_disk_bytes: 0,
                    capture_stride: 1,
                    max_session_bytes: None,
                },
                None,
                None,
            )
            .await
            .expect("engine run");

        assert!(summary.total_ticks >= 1);
        assert_eq!(summary.captures, 0);
        assert_eq!(summary.skipped, 0);
        assert_eq!(summary.failures, summary.total_ticks);
    }

    #[tokio::test]
    async fn max_session_bytes_stops_session_after_budget_is_exceeded() {
        let temp = tempdir().expect("tempdir");
        let context = ContextLog::new(temp.path().join("context.md"));

        let engine = CaptureEngine::new(
            Arc::new(MockScreenshotProvider),
            Arc::new(MetadataAnalyzer),
            Arc::new(AllowAllPrivacyGuard::default()),
            context,
        );

        // MockScreenshotProvider writes 10 bytes per capture. With a 15 byte cap,
        // the second capture will exceed the cap and stop the session.
        let summary = engine
            .run(
                EngineConfig {
                    output_dir: temp.path().join("captures"),
                    filename_prefix: "test".to_string(),
                    schedule: CaptureSchedule {
                        every: Duration::from_millis(30),
                        run_for: Duration::from_millis(250),
                    },
                    min_free_disk_bytes: 0,
                    capture_stride: 1,
                    max_session_bytes: Some(15),
                },
                None,
                None,
            )
            .await
            .expect("engine run");

        assert_eq!(summary.captures, 2);
        assert_eq!(summary.failures, 0);

        let capture_count = std::fs::read_dir(temp.path().join("captures"))
            .expect("captures dir")
            .count();
        assert_eq!(capture_count, 2);
    }

    #[tokio::test]
    async fn resume_does_not_burst_captures_after_long_pause() {
        tokio::time::pause();

        let temp = tempdir().expect("tempdir");
        let context = ContextLog::new(temp.path().join("context.md"));

        let engine = CaptureEngine::new(
            Arc::new(MockScreenshotProvider),
            Arc::new(MetadataAnalyzer),
            Arc::new(AllowAllPrivacyGuard::default()),
            context,
        );

        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        let task = tokio::spawn(async move {
            engine
                .run(
                    EngineConfig {
                        output_dir: temp.path().join("captures"),
                        filename_prefix: "test".to_string(),
                        schedule: CaptureSchedule {
                            every: Duration::from_secs(1),
                            run_for: Duration::from_secs(100),
                        },
                        min_free_disk_bytes: 0,
                        capture_stride: 1,
                        max_session_bytes: None,
                    },
                    Some(command_rx),
                    Some(event_tx),
                )
                .await
        });

        // First capture happens immediately at t=0.
        loop {
            match event_rx.recv().await {
                Some(super::EngineEvent::CaptureSucceeded { .. }) => break,
                Some(_) => continue,
                None => panic!("event channel closed early"),
            }
        }

        command_tx.send(ControlCommand::UserPause).expect("pause");
        tokio::task::yield_now().await;

        tokio::time::advance(Duration::from_secs(30)).await;

        command_tx.send(ControlCommand::UserResume).expect("resume");

        // On resume, we expect exactly one immediate capture, then no backlog burst without time advancing.
        loop {
            match event_rx.recv().await {
                Some(super::EngineEvent::CaptureSucceeded { .. }) => break,
                Some(_) => continue,
                None => panic!("event channel closed early"),
            }
        }

        // Drain any extra "catch-up" captures that would indicate burst behavior.
        let mut extra_captures = 0usize;
        while let Ok(event) = event_rx.try_recv() {
            if matches!(event, super::EngineEvent::CaptureSucceeded { .. }) {
                extra_captures += 1;
            }
        }
        assert_eq!(extra_captures, 0, "resume should not burst captures");

        command_tx.send(ControlCommand::Stop).expect("stop");
        let _ = task.await.expect("task join").expect("engine run");
    }
}
