use anyhow::{Context, Result};
use clap::{ArgAction, Args, Parser, Subcommand};
use photographic_memory::analysis::{Analyzer, MetadataAnalyzer, OpenAiAnalyzer};
use photographic_memory::context_log::ContextLog;
use photographic_memory::engine::{CaptureEngine, ControlCommand, EngineConfig, EngineEvent};
use photographic_memory::permissions::{
    ScreenRecordingStatus, open_screen_recording_settings, screen_recording_help_message,
    screen_recording_status,
};
use photographic_memory::scheduler::CaptureSchedule;
use photographic_memory::screenshot::MacOsScreenshotProvider;
use std::io::{self, BufRead};
use std::path::PathBuf;
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

    #[arg(long, default_value = "capture")]
    filename_prefix: String,
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
    }
}

async fn run_capture(
    common: CommonArgs,
    every: Duration,
    run_for: Duration,
    interactive: bool,
) -> Result<()> {
    ensure_screen_recording_permission()?;

    let context_log = ContextLog::new(&common.context);
    let screenshot_provider = Arc::new(MacOsScreenshotProvider);
    let analyzer = build_analyzer(&common).context("failed to initialize analyzer")?;

    let engine = CaptureEngine::new(screenshot_provider, analyzer, context_log);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();

    let event_handle = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                EngineEvent::Started => println!("session started"),
                EngineEvent::Paused => println!("session paused"),
                EngineEvent::Resumed => println!("session resumed"),
                EngineEvent::CaptureSucceeded { index, path } => {
                    println!("capture #{index} saved: {}", path.display())
                }
                EngineEvent::CaptureFailed { index, message } => {
                    eprintln!("capture #{index} failed: {message}")
                }
                EngineEvent::Stopped => println!("session stopped"),
                EngineEvent::Completed {
                    total_captures,
                    failures,
                } => {
                    println!(
                        "session completed with {total_captures} captures ({failures} failures)"
                    )
                }
            }
        }
    });

    let (command_tx, command_rx) = if interactive {
        let (tx, rx) = mpsc::unbounded_channel();
        let tx_clone = tx.clone();
        tokio::task::spawn_blocking(move || {
            eprintln!("interactive controls: pause | resume | stop");
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                let Ok(line) = line else {
                    break;
                };
                let command = match line.trim().to_ascii_lowercase().as_str() {
                    "pause" => Some(ControlCommand::Pause),
                    "resume" => Some(ControlCommand::Resume),
                    "stop" | "quit" | "exit" => Some(ControlCommand::Stop),
                    "" => None,
                    _ => {
                        eprintln!("unknown command. use: pause | resume | stop");
                        None
                    }
                };

                if let Some(command) = command {
                    if tx_clone.send(command).is_err() {
                        break;
                    }
                }
            }
        });
        (Some(tx), Some(rx))
    } else {
        (None, None)
    };

    let summary = engine
        .run(
            EngineConfig {
                output_dir: common.output_dir,
                filename_prefix: common.filename_prefix,
                schedule: CaptureSchedule { every, run_for },
            },
            command_rx,
            Some(event_tx),
        )
        .await?;

    if let Some(tx) = command_tx {
        drop(tx);
    }

    event_handle.await.context("event task failed")?;

    if summary.failures > 0 {
        eprintln!(
            "completed with {} failures out of {} captures",
            summary.failures, summary.total_captures
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
