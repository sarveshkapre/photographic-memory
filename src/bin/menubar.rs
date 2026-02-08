use anyhow::Result;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use photographic_memory::analysis::{Analyzer, MetadataAnalyzer, OpenAiAnalyzer};
use photographic_memory::context_log::ContextLog;
use photographic_memory::engine::{CaptureEngine, ControlCommand, EngineConfig, EngineEvent};
use photographic_memory::scheduler::CaptureSchedule;
use photographic_memory::screenshot::MacOsScreenshotProvider;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder};

#[derive(Debug, Clone)]
enum UserEvent {
    Menu(MenuEvent),
    Hotkey(GlobalHotKeyEvent),
    Session(SessionEvent),
}

#[derive(Debug, Clone)]
enum SessionEvent {
    Status(String),
    Completed,
}

#[derive(Debug, Clone)]
struct SessionSpec {
    name: &'static str,
    every: Duration,
    run_for: Duration,
    ai_enabled: bool,
}

struct SessionController {
    tx: tokio::sync::mpsc::UnboundedSender<ControlCommand>,
}

struct AppState {
    session: Option<SessionController>,
}

impl AppState {
    fn new() -> Self {
        Self { session: None }
    }

    fn is_running(&self) -> bool {
        self.session.is_some()
    }

    fn send(&self, cmd: ControlCommand) {
        if let Some(session) = &self.session {
            let _ = session.tx.send(cmd);
        }
    }
}

fn main() -> Result<()> {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let proxy_for_menu = proxy.clone();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy_for_menu.send_event(UserEvent::Menu(event));
    }));

    let hotkey_manager = GlobalHotKeyManager::new()?;
    let instant_capture_hotkey = HotKey::new(Some(Modifiers::ALT), Code::KeyS);
    hotkey_manager.register(instant_capture_hotkey)?;
    let hotkey_id = instant_capture_hotkey.id();

    let proxy_for_hotkey = proxy.clone();
    GlobalHotKeyEvent::set_event_handler(Some(move |event| {
        let _ = proxy_for_hotkey.send_event(UserEvent::Hotkey(event));
    }));

    let status_item = MenuItem::new("Status: Idle", false, None);
    let immediate_item = MenuItem::new("Immediate Screenshot (Option+S)", true, None);
    let run_normal_item = MenuItem::new("Take screenshot every 2s for next 60 mins", true, None);
    let run_fast_item = MenuItem::new(
        "Take screenshot every 30ms for next 10 mins (AI sampled)",
        true,
        None,
    );
    let pause_item = MenuItem::new("Pause", false, None);
    let resume_item = MenuItem::new("Resume", false, None);
    let stop_item = MenuItem::new("Stop", false, None);
    let quit_item = MenuItem::new("Quit", true, None);

    let menu = Menu::new();
    menu.append(&status_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&immediate_item)?;
    menu.append(&run_normal_item)?;
    menu.append(&run_fast_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&pause_item)?;
    menu.append(&resume_item)?;
    menu.append(&stop_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit_item)?;

    let mut tray_icon = None;
    let mut app = AppState::new();

    // Keep manager alive for the full event-loop lifetime.
    let _hotkey_manager = hotkey_manager;

    event_loop.run(move |event, _target, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {
                if tray_icon.is_none() {
                    let built = TrayIconBuilder::new()
                        .with_menu(Box::new(menu.clone()))
                        .with_tooltip("Photographic Memory")
                        .with_title("PM")
                        .with_icon(build_icon())
                        .build();

                    if let Ok(icon) = built {
                        tray_icon = Some(icon);
                    } else {
                        let _ = status_item.set_text("Status: Failed to init tray icon");
                    }
                }
            }
            Event::UserEvent(UserEvent::Hotkey(hotkey_event)) => {
                if hotkey_event.id == hotkey_id && hotkey_event.state == HotKeyState::Pressed {
                    start_session(
                        &mut app,
                        &proxy,
                        SessionSpec {
                            name: "Immediate",
                            every: Duration::from_secs(1),
                            run_for: Duration::from_millis(10),
                            ai_enabled: true,
                        },
                    );
                    refresh_controls(&app, &pause_item, &resume_item, &stop_item);
                }
            }
            Event::UserEvent(UserEvent::Menu(menu_event)) => {
                if menu_event.id == immediate_item.id() {
                    start_session(
                        &mut app,
                        &proxy,
                        SessionSpec {
                            name: "Immediate",
                            every: Duration::from_secs(1),
                            run_for: Duration::from_millis(10),
                            ai_enabled: true,
                        },
                    );
                } else if menu_event.id == run_normal_item.id() {
                    start_session(
                        &mut app,
                        &proxy,
                        SessionSpec {
                            name: "2s/60m",
                            every: Duration::from_secs(2),
                            run_for: Duration::from_secs(60 * 60),
                            ai_enabled: true,
                        },
                    );
                } else if menu_event.id == run_fast_item.id() {
                    start_session(
                        &mut app,
                        &proxy,
                        SessionSpec {
                            name: "30ms/10m",
                            every: Duration::from_millis(30),
                            run_for: Duration::from_secs(10 * 60),
                            ai_enabled: false,
                        },
                    );
                } else if menu_event.id == pause_item.id() {
                    app.send(ControlCommand::Pause);
                } else if menu_event.id == resume_item.id() {
                    app.send(ControlCommand::Resume);
                } else if menu_event.id == stop_item.id() {
                    app.send(ControlCommand::Stop);
                } else if menu_event.id == quit_item.id() {
                    app.send(ControlCommand::Stop);
                    *control_flow = ControlFlow::Exit;
                }
                refresh_controls(&app, &pause_item, &resume_item, &stop_item);
            }
            Event::UserEvent(UserEvent::Session(session_event)) => match session_event {
                SessionEvent::Status(text) => {
                    let _ = status_item.set_text(&format!("Status: {text}"));
                }
                SessionEvent::Completed => {
                    app.session = None;
                    let _ = status_item.set_text("Status: Idle");
                    refresh_controls(&app, &pause_item, &resume_item, &stop_item);
                }
            },
            _ => {}
        }
    });
}

fn refresh_controls(
    app: &AppState,
    pause_item: &MenuItem,
    resume_item: &MenuItem,
    stop_item: &MenuItem,
) {
    let running = app.is_running();
    let _ = pause_item.set_enabled(running);
    let _ = resume_item.set_enabled(running);
    let _ = stop_item.set_enabled(running);
}

fn start_session(app: &mut AppState, proxy: &EventLoopProxy<UserEvent>, spec: SessionSpec) {
    if app.is_running() {
        let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status(
            "Already running. Use Stop before starting a new session.".to_string(),
        )));
        return;
    }

    let (control_tx, control_rx) = tokio::sync::mpsc::unbounded_channel();
    app.session = Some(SessionController { tx: control_tx });

    let proxy = proxy.clone();
    thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(err) => {
                let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status(format!(
                    "Runtime error: {err}"
                ))));
                let _ = proxy.send_event(UserEvent::Session(SessionEvent::Completed));
                return;
            }
        };

        runtime.block_on(async move {
            let data_dir = default_data_dir();
            let output_dir = data_dir.join("captures");
            let context_path = data_dir.join("context.md");
            let screenshot_provider = Arc::new(MacOsScreenshotProvider);
            let analyzer = build_analyzer(spec.ai_enabled);

            if !spec.ai_enabled {
                let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status(
                    "Running high-frequency mode with local analysis only".to_string(),
                )));
            }

            let engine =
                CaptureEngine::new(screenshot_provider, analyzer, ContextLog::new(context_path));
            let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<EngineEvent>();

            let proxy_events = proxy.clone();
            let session_name = spec.name.to_string();
            let forward_task = tokio::spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    let text = match event {
                        EngineEvent::Started => format!("Running {session_name}"),
                        EngineEvent::Paused => "Paused".to_string(),
                        EngineEvent::Resumed => format!("Running {session_name}"),
                        EngineEvent::CaptureSucceeded { index, .. } => {
                            format!("Running {session_name} (capture #{index})")
                        }
                        EngineEvent::CaptureFailed { index, .. } => {
                            format!("Running {session_name} (error at #{index})")
                        }
                        EngineEvent::Stopped => "Stopped".to_string(),
                        EngineEvent::Completed {
                            total_captures,
                            failures,
                        } => {
                            format!("Done ({total_captures} captures, {failures} failures)")
                        }
                    };
                    let _ = proxy_events.send_event(UserEvent::Session(SessionEvent::Status(text)));
                }
            });

            let result = engine
                .run(
                    EngineConfig {
                        output_dir,
                        filename_prefix: "capture".to_string(),
                        schedule: CaptureSchedule {
                            every: spec.every,
                            run_for: spec.run_for,
                        },
                    },
                    Some(control_rx),
                    Some(event_tx),
                )
                .await;

            if let Err(err) = result {
                let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status(format!(
                    "Session failed: {err}"
                ))));
            }

            forward_task.abort();
            let _ = proxy.send_event(UserEvent::Session(SessionEvent::Completed));
        });
    });
}

fn build_analyzer(ai_enabled: bool) -> Arc<dyn Analyzer> {
    if !ai_enabled {
        return Arc::new(MetadataAnalyzer);
    }

    match std::env::var("OPENAI_API_KEY") {
        Ok(api_key) if !api_key.trim().is_empty() => Arc::new(OpenAiAnalyzer::new(
            api_key,
            "gpt-5".to_string(),
            "Describe what is visible and summarize likely user intent in concise bullet points."
                .to_string(),
        )),
        _ => Arc::new(MetadataAnalyzer),
    }
}

fn default_data_dir() -> PathBuf {
    match std::env::var_os("HOME") {
        Some(home) => {
            let path = PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("photographic-memory");
            let _ = std::fs::create_dir_all(&path);
            path
        }
        None => PathBuf::from("."),
    }
}

fn build_icon() -> Icon {
    let (width, height) = (18, 18);
    let mut rgba = Vec::with_capacity(width * height * 4);

    for y in 0..height {
        for x in 0..width {
            let is_border = x == 0 || y == 0 || x == width - 1 || y == height - 1;
            let is_center = (x > 5 && x < 12) && (y > 5 && y < 12);
            let alpha = if is_border || is_center { 255 } else { 180 };
            rgba.extend_from_slice(&[0, 0, 0, alpha]);
        }
    }

    Icon::from_rgba(rgba, width as u32, height as u32).expect("valid tray icon")
}
