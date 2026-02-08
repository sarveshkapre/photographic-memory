use crate::analysis::{AnalysisResult, Analyzer};
use crate::context_log::{ContextEntry, ContextLog};
use crate::scheduler::{CaptureSchedule, Scheduler};
use crate::screenshot::ScreenshotProvider;
use crate::storage::{ReclaimOutcome, ensure_disk_headroom, reclaim_disk_space};
use anyhow::{Context, Result};
use chrono::Utc;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCommand {
    Pause,
    Resume,
    Stop,
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    Started,
    Paused,
    Resumed,
    CaptureSucceeded {
        index: u64,
        path: PathBuf,
    },
    CaptureFailed {
        index: u64,
        message: String,
    },
    DiskCleanup {
        deleted_files: usize,
        freed_bytes: u64,
        remaining_bytes: u64,
    },
    Stopped,
    Completed {
        total_captures: u64,
        failures: u64,
    },
}

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub output_dir: PathBuf,
    pub filename_prefix: String,
    pub schedule: CaptureSchedule,
    pub min_free_disk_bytes: u64,
}

pub const DEFAULT_MIN_FREE_DISK_BYTES: u64 = 1_073_741_824; // 1 GiB

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EngineSummary {
    pub total_captures: u64,
    pub failures: u64,
}

pub struct CaptureEngine {
    screenshot_provider: Arc<dyn ScreenshotProvider>,
    analyzer: Arc<dyn Analyzer>,
    context_log: ContextLog,
}

impl CaptureEngine {
    pub fn new(
        screenshot_provider: Arc<dyn ScreenshotProvider>,
        analyzer: Arc<dyn Analyzer>,
        context_log: ContextLog,
    ) -> Self {
        Self {
            screenshot_provider,
            analyzer,
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
        let mut paused = false;
        let mut summary = EngineSummary::default();

        send_event(&event_tx, EngineEvent::Started);

        loop {
            while let Some(rx) = command_rx.as_mut() {
                match rx.try_recv() {
                    Ok(cmd) => {
                        if handle_command(cmd, &mut paused, &event_tx) {
                            send_event(
                                &event_tx,
                                EngineEvent::Completed {
                                    total_captures: summary.total_captures,
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

            if paused {
                if let Some(rx) = command_rx.as_mut() {
                    match rx.recv().await {
                        Some(cmd) => {
                            if handle_command(cmd, &mut paused, &event_tx) {
                                send_event(
                                    &event_tx,
                                    EngineEvent::Completed {
                                        total_captures: summary.total_captures,
                                        failures: summary.failures,
                                    },
                                );
                                return Ok(summary);
                            }
                        }
                        None => {
                            command_rx = None;
                            paused = false;
                        }
                    }
                } else {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                continue;
            }

            let elapsed = start.elapsed();
            if scheduler.is_finished(elapsed) {
                send_event(
                    &event_tx,
                    EngineEvent::Completed {
                        total_captures: summary.total_captures,
                        failures: summary.failures,
                    },
                );
                return Ok(summary);
            }

            if scheduler.should_capture(elapsed) {
                summary.total_captures += 1;
                let index = summary.total_captures;
                let capture_result = self.capture_once(index, &config, &event_tx).await;

                match capture_result {
                    Ok(path) => {
                        send_event(&event_tx, EngineEvent::CaptureSucceeded { index, path })
                    }
                    Err(err) => {
                        summary.failures += 1;
                        send_event(
                            &event_tx,
                            EngineEvent::CaptureFailed {
                                index,
                                message: err.to_string(),
                            },
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
                                if handle_command(cmd, &mut paused, &event_tx) {
                                    send_event(&event_tx, EngineEvent::Completed {
                                        total_captures: summary.total_captures,
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
    paused: &mut bool,
    event_tx: &Option<mpsc::UnboundedSender<EngineEvent>>,
) -> bool {
    match cmd {
        ControlCommand::Pause => {
            if !*paused {
                *paused = true;
                send_event(event_tx, EngineEvent::Paused);
            }
            false
        }
        ControlCommand::Resume => {
            if *paused {
                *paused = false;
                send_event(event_tx, EngineEvent::Resumed);
            }
            false
        }
        ControlCommand::Stop => {
            send_event(event_tx, EngineEvent::Stopped);
            true
        }
    }
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
                },
                None,
                None,
            )
            .await
            .expect("engine run");

        assert_eq!(summary.total_captures, 5);
        assert_eq!(summary.failures, 0);

        let capture_count = std::fs::read_dir(temp.path().join("captures"))
            .expect("captures dir")
            .count();
        assert_eq!(capture_count, 5);
    }

    #[tokio::test]
    async fn stop_command_ends_session() {
        let temp = tempdir().expect("tempdir");
        let context = ContextLog::new(temp.path().join("context.md"));

        let engine = CaptureEngine::new(
            Arc::new(MockScreenshotProvider),
            Arc::new(MetadataAnalyzer),
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
        assert_eq!(summary.total_captures, 1);
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
                },
                None,
                None,
            )
            .await
            .expect("engine run");

        assert_eq!(summary.total_captures, 4);
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
                },
                None,
                None,
            )
            .await
            .expect("engine run");

        assert_eq!(summary.total_captures, 3);
        assert_eq!(summary.failures, 3);
    }
}
