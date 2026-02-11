use anyhow::{Context, Result};
use clap::{ArgAction, Args, Parser, Subcommand};
use photographic_memory::activity_watch::{ActivityEvent, spawn_activity_watch};
use photographic_memory::analysis::{Analyzer, MetadataAnalyzer, OpenAiAnalyzer};
use photographic_memory::context_log::ContextLog;
use photographic_memory::engine::{
    CaptureEngine, ControlCommand, DEFAULT_MIN_FREE_DISK_BYTES, EngineConfig, EngineEvent,
};
use photographic_memory::paths::{default_data_dir, default_privacy_config_path};
use photographic_memory::permission_watch::spawn_permission_watch;
use photographic_memory::permissions::{
    AccessibilityStatus, ScreenRecordingStatus, accessibility_help_message, accessibility_status,
    open_screen_recording_settings, screen_recording_help_message, screen_recording_status,
};
use photographic_memory::privacy::{
    AllowAllPrivacyGuard, ConfigPrivacyGuard, MacOsForegroundAppProvider, PrivacyGuard,
};
use photographic_memory::scheduler::CaptureSchedule;
use photographic_memory::screenshot::{
    MacOsScreenshotProvider, MockScreenshotProvider, ScreenshotProvider,
};
use photographic_memory::storage::available_bytes_under;
use photographic_memory::system_activity::{DisplaySleepStatus, ScreenLockStatus};
use std::io::{self, BufRead};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Parser)]
#[command(name = "photographic-memory")]
#[command(about = "Capture screenshots and append AI analysis to context.md")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Immediate(CommonArgs),
    Run(RunArgs),
    Plan,
    Doctor,
}

#[derive(Debug, Args, Clone)]
struct CommonArgs {
    #[arg(long, default_value = "captures")]
    output_dir: PathBuf,

    #[arg(long, default_value = "context.md")]
    context: PathBuf,

    #[arg(long, default_value = "gpt-5")]
    model: String,

    #[arg(
        long,
        default_value = "Describe what is visible in this screenshot and capture task progress, blockers, and user intent in concise bullet points."
    )]
    prompt: String,

    #[arg(long, action = ArgAction::SetTrue)]
    no_analyze: bool,

    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Use a mock screenshot provider and skip Screen Recording permission checks (for CI/smoke). Writes dummy .png files."
    )]
    mock_screenshot: bool,

    #[arg(long, default_value = "capture")]
    filename_prefix: String,

    #[arg(
        long,
        default_value_t = DEFAULT_MIN_FREE_DISK_BYTES,
        value_parser = parse_min_free_bytes,
        value_name = "BYTES",
        help = "Guardrail: abort session if capture directory freespace drops below this byte count (supports suffixes like 512MB, 2GB)."
    )]
    min_free_bytes: u64,

    #[arg(
        long,
        default_value_t = 1,
        value_parser = clap::value_parser!(u64).range(1..),
        value_name = "N",
        help = "Throttle: only attempt a real capture every N scheduler ticks (useful for high-frequency schedules like 30ms)."
    )]
    capture_stride: u64,

    #[arg(
        long,
        value_parser = parse_max_session_bytes,
        value_name = "BYTES",
        help = "Guardrail: stop session once total bytes written to output-dir exceeds this cap (supports suffixes like 200MB, 1GB)."
    )]
    max_session_bytes: Option<u64>,

    #[arg(
        long,
        value_name = "PATH",
        help = "Path to privacy policy TOML (deny apps/private windows). Defaults to app data dir."
    )]
    privacy_config: Option<PathBuf>,

    #[arg(long, action = ArgAction::SetTrue, help = "Disable privacy checks (unsafe).")]
    no_privacy: bool,
}

#[derive(Debug, Args, Clone)]
struct RunArgs {
    #[command(flatten)]
    common: CommonArgs,

    #[arg(long, default_value = "2s", value_parser = parse_duration)]
    every: Duration,

    #[arg(long = "for", default_value = "60m", value_parser = parse_duration)]
    run_for: Duration,

    #[arg(long, action = ArgAction::SetTrue)]
    interactive: bool,
}

fn parse_duration(value: &str) -> std::result::Result<Duration, String> {
    humantime::parse_duration(value).map_err(|e| e.to_string())
}

fn parse_min_free_bytes(value: &str) -> std::result::Result<u64, String> {
    parse_human_readable_bytes(value)
        .ok_or_else(|| "expected byte size such as 1073741824, 512MB, or 1.5GB".to_string())
}

fn parse_max_session_bytes(value: &str) -> std::result::Result<u64, String> {
    parse_human_readable_bytes(value)
        .ok_or_else(|| "expected byte size such as 200MB, 1GB, or 1073741824".to_string())
}

fn parse_human_readable_bytes(input: &str) -> Option<u64> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut split_index = trimmed.len();
    for (idx, ch) in trimmed.char_indices() {
        if !(ch.is_ascii_digit() || ch == '.' || ch == '_') {
            split_index = idx;
            break;
        }
    }

    let number_part = trimmed[..split_index].replace('_', "");
    if number_part.is_empty() {
        return None;
    }

    let value: f64 = number_part.parse().ok()?;
    let unit = trimmed[split_index..].trim().to_ascii_lowercase();

    let multiplier: u64 = match unit.as_str() {
        "" | "b" => 1,
        "kb" | "kib" => 1 << 10,
        "mb" | "mib" => 1 << 20,
        "gb" | "gib" => 1 << 30,
        "tb" | "tib" => 1 << 40,
        _ => return None,
    };

    let bytes = value * multiplier as f64;
    if bytes.is_finite() && bytes >= 0.0 {
        Some(bytes.round() as u64)
    } else {
        None
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Immediate(common) => {
            run_capture(
                common,
                Duration::from_secs(60),
                Duration::from_millis(1),
                false,
            )
            .await
        }
        Commands::Run(args) => {
            run_capture(args.common, args.every, args.run_for, args.interactive).await
        }
        Commands::Plan => {
            print_plan();
            Ok(())
        }
        Commands::Doctor => {
            print_doctor()?;
            Ok(())
        }
    }
}

async fn run_capture(
    common: CommonArgs,
    every: Duration,
    run_for: Duration,
    interactive: bool,
) -> Result<()> {
    if common.mock_screenshot {
        eprintln!("NOTE: running with --mock-screenshot (no real screenshots will be captured).");
    } else {
        ensure_screen_recording_permission()?;
    }

    let context_log = ContextLog::new(&common.context);
    let screenshot_provider: Arc<dyn ScreenshotProvider> = if common.mock_screenshot {
        Arc::new(MockScreenshotProvider)
    } else {
        Arc::new(MacOsScreenshotProvider)
    };
    let analyzer = build_analyzer(&common).context("failed to initialize analyzer")?;

    let privacy_config_path = common
        .privacy_config
        .clone()
        .unwrap_or_else(default_privacy_config_path);
    let privacy_guard: Arc<dyn PrivacyGuard> = if common.no_privacy {
        Arc::new(AllowAllPrivacyGuard::new(privacy_config_path))
    } else {
        Arc::new(ConfigPrivacyGuard::new(
            privacy_config_path,
            MacOsForegroundAppProvider,
        ))
    };
    if let Err(err) = privacy_guard.reload() {
        eprintln!("Privacy config error: {err}. Captures will be skipped until resolved.");
    }

    let engine = CaptureEngine::new(screenshot_provider, analyzer, privacy_guard, context_log);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();

    let event_handle = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                EngineEvent::Started => println!("session started"),
                EngineEvent::Paused => println!("session paused"),
                EngineEvent::Resumed => println!("session resumed"),
                EngineEvent::AutoPaused { reason } => println!("session auto-paused: {reason:?}"),
                EngineEvent::AutoResumed { reason } => {
                    println!("session auto-resumed: {reason:?}")
                }
                EngineEvent::CaptureSkipped { tick_index, reason } => {
                    eprintln!("tick #{tick_index} skipped: {reason}");
                }
                EngineEvent::CaptureSucceeded {
                    capture_index,
                    path,
                } => {
                    println!("capture #{capture_index} saved: {}", path.display())
                }
                EngineEvent::CaptureFailed {
                    capture_index,
                    message,
                } => {
                    eprintln!("capture #{capture_index} failed: {message}")
                }
                EngineEvent::DiskCleanup {
                    deleted_files,
                    freed_bytes,
                    remaining_bytes,
                } => {
                    println!(
                        "disk guard reclaimed {deleted_files} files ({:.1} MB freed, {:.1} MB remaining)",
                        freed_bytes as f64 / (1024.0 * 1024.0),
                        remaining_bytes as f64 / (1024.0 * 1024.0)
                    );
                }
                EngineEvent::BudgetExceeded {
                    bytes_written,
                    limit_bytes,
                } => {
                    eprintln!(
                        "session budget exceeded: wrote {} bytes (cap: {} bytes). stopping.",
                        bytes_written, limit_bytes
                    );
                }
                EngineEvent::Stopped => println!("session stopped"),
                EngineEvent::Completed {
                    total_ticks,
                    captures,
                    skipped,
                    failures,
                } => {
                    println!(
                        "session completed: {captures} captures, {skipped} skipped, {failures} failures ({total_ticks} ticks)"
                    )
                }
            }
        }
    });

    let (command_tx, command_rx) = mpsc::unbounded_channel();

    if interactive {
        let tx_clone = command_tx.clone();
        tokio::task::spawn_blocking(move || {
            eprintln!("interactive controls: pause | resume | stop");
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                let Ok(line) = line else {
                    break;
                };
                let command = match line.trim().to_ascii_lowercase().as_str() {
                    "pause" => Some(ControlCommand::UserPause),
                    "resume" => Some(ControlCommand::UserResume),
                    "stop" | "quit" | "exit" => Some(ControlCommand::Stop),
                    "" => None,
                    _ => {
                        eprintln!("unknown command. use: pause | resume | stop");
                        None
                    }
                };

                if let Some(command) = command
                    && tx_clone.send(command).is_err()
                {
                    break;
                }
            }
        });
    }

    // In mock mode, skip permission/activity auto-pause watchers so local smoke runs are
    // deterministic and never hang due host lock/sleep/permission state.
    let permission_guard = if common.mock_screenshot {
        None
    } else {
        spawn_permission_watch(command_tx.clone(), |status| match status {
            ScreenRecordingStatus::Denied => {
                eprintln!(
                    "Screen Recording permission revoked mid-session. Auto-pausing captures."
                );
            }
            ScreenRecordingStatus::Granted => {
                eprintln!("Screen Recording permission restored. Auto-resuming captures.");
            }
            ScreenRecordingStatus::NotSupported => {}
        })
    };

    let activity_guard = if common.mock_screenshot {
        None
    } else {
        spawn_activity_watch(command_tx.clone(), |event| match event {
            ActivityEvent::ScreenLock(status) => match status {
                ScreenLockStatus::Locked => {
                    eprintln!("Screen locked. Auto-pausing captures.");
                }
                ScreenLockStatus::Unlocked => {
                    eprintln!("Screen unlocked. Auto-resuming captures.");
                }
                ScreenLockStatus::Unknown | ScreenLockStatus::NotSupported => {}
            },
            ActivityEvent::DisplaySleep(status) => match status {
                DisplaySleepStatus::Asleep => {
                    eprintln!("Display asleep. Auto-pausing captures.");
                }
                DisplaySleepStatus::Awake => {
                    eprintln!("Display awake. Auto-resuming captures.");
                }
                DisplaySleepStatus::Unknown | DisplaySleepStatus::NotSupported => {}
            },
        })
    };

    let summary = engine
        .run(
            EngineConfig {
                output_dir: common.output_dir,
                filename_prefix: common.filename_prefix,
                schedule: CaptureSchedule { every, run_for },
                min_free_disk_bytes: common.min_free_bytes,
                capture_stride: common.capture_stride,
                max_session_bytes: common.max_session_bytes,
            },
            Some(command_rx),
            Some(event_tx),
        )
        .await?;

    drop(command_tx);

    if let Some(handle) = permission_guard {
        handle.abort();
        let _ = handle.await;
    }

    if let Some(handle) = activity_guard {
        handle.abort();
        let _ = handle.await;
    }

    event_handle.await.context("event task failed")?;

    if summary.failures > 0 || summary.skipped > 0 {
        eprintln!(
            "completed: {} captures, {} skipped, {} failures ({} ticks)",
            summary.captures, summary.skipped, summary.failures, summary.total_ticks
        );
    }

    Ok(())
}

fn build_analyzer(common: &CommonArgs) -> Result<Arc<dyn Analyzer>> {
    if common.no_analyze {
        return Ok(Arc::new(MetadataAnalyzer));
    }

    match std::env::var("OPENAI_API_KEY") {
        Ok(api_key) if !api_key.trim().is_empty() => Ok(Arc::new(OpenAiAnalyzer::new(
            api_key,
            common.model.clone(),
            common.prompt.clone(),
        ))),
        _ => {
            eprintln!("OPENAI_API_KEY is not set. Falling back to local metadata analyzer.");
            Ok(Arc::new(MetadataAnalyzer))
        }
    }
}

fn ensure_screen_recording_permission() -> Result<()> {
    match screen_recording_status() {
        ScreenRecordingStatus::Granted | ScreenRecordingStatus::NotSupported => Ok(()),
        ScreenRecordingStatus::Denied => {
            eprintln!(
                "Screen Recording permission is denied. {}",
                screen_recording_help_message()
            );
            if let Err(err) = open_screen_recording_settings() {
                eprintln!("Unable to auto-open System Settings: {err}");
            }
            anyhow::bail!("grant Screen Recording permission and re-run")
        }
    }
}

fn print_plan() {
    println!("Menu bar app roadmap (Rust):");
    println!(
        "1. Add a macOS status bar target with immediate/interval/pause/resume/stop controls."
    );
    println!("2. Keep capture engine shared between CLI and menu bar.");
    println!("3. Add global hotkey (Option+S) and live status text in menu bar.");
    println!("4. Persist session state so restart can recover safely.");
}

fn print_doctor() -> Result<()> {
    const AGENT_ID: &str = "com.sarvesh.photographic-memory";

    println!("Photographic Memory doctor");
    println!("Version: {}", env!("CARGO_PKG_VERSION"));

    let data_dir = default_data_dir();
    let captures_dir = data_dir.join("captures");
    let context_path = data_dir.join("context.md");
    let privacy_path = default_privacy_config_path();

    println!("Data dir: {}", data_dir.display());
    println!("Captures dir: {}", captures_dir.display());
    println!("Context log: {}", context_path.display());

    let permission = screen_recording_status();
    let permission_text = match permission {
        ScreenRecordingStatus::Granted => "Granted",
        ScreenRecordingStatus::Denied => "Denied",
        ScreenRecordingStatus::NotSupported => "Not required",
    };
    println!("Screen Recording: {permission_text}");
    if matches!(permission, ScreenRecordingStatus::Denied) {
        println!("Hint: {}", screen_recording_help_message());
    }

    let accessibility = accessibility_status();
    let accessibility_text = match accessibility {
        AccessibilityStatus::Granted => "Granted",
        AccessibilityStatus::Denied => "Denied",
        AccessibilityStatus::NotSupported => "Not required",
    };
    println!("Accessibility: {accessibility_text}");
    if matches!(accessibility, AccessibilityStatus::Denied) {
        println!("Hint: {}", accessibility_help_message());
    }

    let guard = ConfigPrivacyGuard::new(privacy_path.clone(), MacOsForegroundAppProvider);
    match guard.reload() {
        Ok(()) => {
            let status = guard.status();
            println!(
                "Privacy policy: {} ({}, {})",
                privacy_path.display(),
                if status.enabled { "active" } else { "disabled" },
                status.rule_summary
            );
        }
        Err(err) => {
            println!("Privacy policy: {} (error: {err})", privacy_path.display());
        }
    }

    let _ = std::fs::create_dir_all(&captures_dir);
    match available_bytes_under(&captures_dir) {
        Ok(bytes) => {
            println!(
                "Disk free under captures: {} bytes ({:.1} GB)",
                bytes,
                bytes as f64 / (1024.0 * 1024.0 * 1024.0)
            );
        }
        Err(err) => {
            println!("Disk free under captures: error ({err})");
        }
    }

    let home = std::env::var_os("HOME").map(PathBuf::from);
    let plist_path = home
        .as_ref()
        .map(|h| {
            h.join("Library/LaunchAgents")
                .join(format!("{AGENT_ID}.plist"))
        })
        .unwrap_or_else(|| PathBuf::from(format!("{AGENT_ID}.plist")));

    println!(
        "Launch Agent plist: {} ({})",
        plist_path.display(),
        if plist_path.exists() {
            "present"
        } else {
            "missing"
        }
    );

    let uid = unsafe { libc::geteuid() };
    let launch_domain = format!("gui/{uid}/{AGENT_ID}");
    match Command::new("launchctl")
        .arg("print")
        .arg(&launch_domain)
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                println!("Launch Agent status: loaded ({launch_domain})");
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let msg = stderr.trim();
                if msg.is_empty() {
                    println!("Launch Agent status: not loaded ({launch_domain})");
                } else {
                    println!("Launch Agent status: not loaded ({launch_domain}) ({msg})");
                }
                println!("Hint: run ./scripts/install-launch-agent.sh");
            }
        }
        Err(err) => {
            println!("Launch Agent status: unable to run launchctl ({err})");
        }
    }

    let log_path = home
        .as_ref()
        .map(|h| h.join("Library/Logs/photographic-memory.log"))
        .unwrap_or_else(|| PathBuf::from("photographic-memory.log"));
    let err_log_path = home
        .as_ref()
        .map(|h| h.join("Library/Logs/photographic-memory.err.log"))
        .unwrap_or_else(|| PathBuf::from("photographic-memory.err.log"));
    println!(
        "Logs: {} ({})",
        log_path.display(),
        if log_path.exists() {
            "present"
        } else {
            "missing"
        }
    );
    println!(
        "Error logs: {} ({})",
        err_log_path.display(),
        if err_log_path.exists() {
            "present"
        } else {
            "missing"
        }
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_human_readable_bytes, parse_min_free_bytes};

    #[test]
    fn parses_human_readable_byte_sizes() {
        assert_eq!(parse_human_readable_bytes("1.5GB"), Some(1_610_612_736));
        assert_eq!(parse_human_readable_bytes("512MB"), Some(536_870_912));
        assert_eq!(parse_human_readable_bytes("1_024kb"), Some(1_048_576));
        assert_eq!(parse_human_readable_bytes("2048"), Some(2048));
    }

    #[test]
    fn rejects_invalid_byte_sizes() {
        assert!(parse_human_readable_bytes("").is_none());
        assert!(parse_human_readable_bytes("abc").is_none());
        assert!(parse_human_readable_bytes("12PB").is_none());
        assert!(parse_min_free_bytes("invalid").is_err());
    }
}
