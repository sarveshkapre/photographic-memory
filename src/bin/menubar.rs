use anyhow::Result;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use opener::open;
use photographic_memory::analysis::{Analyzer, MetadataAnalyzer, OpenAiAnalyzer};
use photographic_memory::context_log::ContextLog;
use photographic_memory::engine::{CaptureEngine, ControlCommand, EngineConfig, EngineEvent};
use photographic_memory::permissions::{
    ScreenRecordingStatus, open_screen_recording_settings, screen_recording_help_message,
    screen_recording_status,
};
use photographic_memory::scheduler::CaptureSchedule;
use photographic_memory::screenshot::MacOsScreenshotProvider;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

#[derive(Debug, Clone)]
enum UserEvent {
    Menu(MenuEvent),
    Hotkey(GlobalHotKeyEvent),
    Session(SessionEvent),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionIndicator {
    Idle,
    Running,
    Paused,
    Error,
}

#[derive(Debug, Clone)]
enum SessionEvent {
    Status {
        text: String,
        indicator: SessionIndicator,
        latest_capture: Option<PathBuf>,
    },
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
    latest_capture: Option<PathBuf>,
    permission_status: ScreenRecordingStatus,
}

impl AppState {
    fn new() -> Self {
        Self {
            session: None,
            latest_capture: None,
            permission_status: screen_recording_status(),
        }
    }

    fn is_running(&self) -> bool {
        self.session.is_some()
    }

    fn send(&self, cmd: ControlCommand) {
        if let Some(session) = &self.session {
            let _ = session.tx.send(cmd);
        }
    }

    fn update_latest_capture(&mut self, path: PathBuf) {
        self.latest_capture = Some(path);
    }

    fn latest_capture(&self) -> Option<&PathBuf> {
        self.latest_capture.as_ref()
    }

    fn permission_status(&self) -> ScreenRecordingStatus {
        self.permission_status
    }

    fn set_permission_status(&mut self, status: ScreenRecordingStatus) {
        self.permission_status = status;
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
    let permission_status_item = MenuItem::new("Screen Recording: Checking status...", false, None);
    let permission_recheck_item = MenuItem::new("Recheck Screen Recording Permission", true, None);
    let permission_settings_item = MenuItem::new("Open Screen Recording Settings...", true, None);
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
    let open_context_item = MenuItem::new("Open context.md", true, None);
    let open_captures_item = MenuItem::new("Open captures folder", true, None);
    let recent_capture_item = MenuItem::new("Open latest capture", false, None);
    let quit_item = MenuItem::new("Quit", true, None);

    let menu = Menu::new();
    menu.append(&status_item)?;
    menu.append(&permission_status_item)?;
    menu.append(&permission_recheck_item)?;
    menu.append(&permission_settings_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&immediate_item)?;
    menu.append(&run_normal_item)?;
    menu.append(&run_fast_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&pause_item)?;
    menu.append(&resume_item)?;
    menu.append(&stop_item)?;
    menu.append(&open_context_item)?;
    menu.append(&open_captures_item)?;
    menu.append(&recent_capture_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit_item)?;

    let icons = IconSet::new();
    let mut tray_icon = None;
    let mut app = AppState::new();
    update_recent_capture_menu(&app, &recent_capture_item);
    update_permission_menu(&app, &permission_status_item);

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
                        .with_icon(icons.icon(SessionIndicator::Idle))
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
                        &permission_status_item,
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
                        &permission_status_item,
                        SessionSpec {
                            name: "Immediate",
                            every: Duration::from_secs(1),
                            run_for: Duration::from_millis(10),
                            ai_enabled: true,
                        },
                    );
                } else if menu_event.id == permission_recheck_item.id() {
                    let status = screen_recording_status();
                    app.set_permission_status(status);
                    update_permission_menu(&app, &permission_status_item);
                    let text = match status {
                        ScreenRecordingStatus::Granted => {
                            "Screen Recording permission granted.".to_string()
                        }
                        ScreenRecordingStatus::NotSupported => {
                            "Screen Recording permission not required.".to_string()
                        }
                        ScreenRecordingStatus::Denied => format!(
                            "Screen Recording permission denied. {}",
                            screen_recording_help_message()
                        ),
                    };
                    let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status {
                        text,
                        indicator: permission_indicator(status),
                        latest_capture: None,
                    }));
                } else if menu_event.id == permission_settings_item.id() {
                    let result = open_screen_recording_settings();
                    let (text, indicator) = match result {
                        Ok(()) => (
                            "Opening Screen Recording settings...".to_string(),
                            SessionIndicator::Idle,
                        ),
                        Err(err) => (
                            format!("Failed to open System Settings: {err}"),
                            SessionIndicator::Error,
                        ),
                    };
                    let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status {
                        text,
                        indicator,
                        latest_capture: None,
                    }));
                } else if menu_event.id == run_normal_item.id() {
                    start_session(
                        &mut app,
                        &proxy,
                        &permission_status_item,
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
                        &permission_status_item,
                        SessionSpec {
                            name: "30ms/10m",
                            every: Duration::from_millis(30),
                            run_for: Duration::from_secs(10 * 60),
                            ai_enabled: false,
                        },
                    );
                } else if menu_event.id == open_context_item.id() {
                    open_path(default_data_dir().join("context.md"), false, &proxy);
                } else if menu_event.id == open_captures_item.id() {
                    open_path(default_data_dir().join("captures"), true, &proxy);
                } else if menu_event.id == recent_capture_item.id() {
                    if let Some(path) = app.latest_capture().cloned() {
                        open_path(path, app.is_running(), &proxy);
                    } else {
                        let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status {
                            text: "No captures yet. Start a session to create one.".to_string(),
                            indicator: SessionIndicator::Idle,
                            latest_capture: None,
                        }));
                    }
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
                SessionEvent::Status {
                    text,
                    indicator,
                    latest_capture,
                } => {
                    if let Some(path) = latest_capture {
                        app.update_latest_capture(path);
                    }
                    let _ = status_item.set_text(&format!("Status: {text}"));
                    update_tray_icon(&mut tray_icon, &icons, indicator);
                    update_recent_capture_menu(&app, &recent_capture_item);
                }
                SessionEvent::Completed => {
                    app.session = None;
                    let _ = status_item.set_text("Status: Idle");
                    update_tray_icon(&mut tray_icon, &icons, SessionIndicator::Idle);
                    refresh_controls(&app, &pause_item, &resume_item, &stop_item);
                    update_recent_capture_menu(&app, &recent_capture_item);
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

fn update_recent_capture_menu(app: &AppState, recent_capture_item: &MenuItem) {
    if let Some(path) = app.latest_capture() {
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("capture.png");
        let _ = recent_capture_item.set_enabled(true);
        let _ = recent_capture_item.set_text(&format!("Open latest capture ({filename})"));
    } else {
        let _ = recent_capture_item.set_enabled(false);
        let _ = recent_capture_item.set_text("Open latest capture");
    }
}

fn update_permission_menu(app: &AppState, permission_status_item: &MenuItem) {
    let text = match app.permission_status() {
        ScreenRecordingStatus::Granted => "Screen Recording: Granted",
        ScreenRecordingStatus::Denied => "Screen Recording: Blocked (open System Settings)",
        ScreenRecordingStatus::NotSupported => "Screen Recording: Not required",
    };
    let _ = permission_status_item.set_text(text);
}

fn permission_indicator(status: ScreenRecordingStatus) -> SessionIndicator {
    match status {
        ScreenRecordingStatus::Granted | ScreenRecordingStatus::NotSupported => {
            SessionIndicator::Idle
        }
        ScreenRecordingStatus::Denied => SessionIndicator::Error,
    }
}

fn start_session(
    app: &mut AppState,
    proxy: &EventLoopProxy<UserEvent>,
    permission_status_item: &MenuItem,
    spec: SessionSpec,
) {
    if app.is_running() {
        let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status {
            text: "Already running. Use Stop before starting a new session.".to_string(),
            indicator: SessionIndicator::Running,
            latest_capture: None,
        }));
        return;
    }

    if !ensure_screen_recording_permission(app, permission_status_item, proxy) {
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
                let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status {
                    text: format!("Runtime error: {err}"),
                    indicator: SessionIndicator::Error,
                    latest_capture: None,
                }));
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
                let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status {
                    text: "Running high-frequency mode with local analysis only".to_string(),
                    indicator: SessionIndicator::Running,
                    latest_capture: None,
                }));
            }

            let engine =
                CaptureEngine::new(screenshot_provider, analyzer, ContextLog::new(context_path));
            let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<EngineEvent>();

            let proxy_events = proxy.clone();
            let session_name = spec.name.to_string();
            let forward_task = tokio::spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    let mut latest_capture = None;
                    let (text, indicator) = match event {
                        EngineEvent::Started => {
                            (format!("Running {session_name}"), SessionIndicator::Running)
                        }
                        EngineEvent::Paused => ("Paused".to_string(), SessionIndicator::Paused),
                        EngineEvent::Resumed => {
                            (format!("Running {session_name}"), SessionIndicator::Running)
                        }
                        EngineEvent::CaptureSucceeded { index, path } => {
                            latest_capture = Some(path);
                            (
                                format!("Running {session_name} (capture #{index})"),
                                SessionIndicator::Running,
                            )
                        }
                        EngineEvent::CaptureFailed { index, .. } => (
                            format!("Running {session_name} (error at #{index})"),
                            SessionIndicator::Error,
                        ),
                        EngineEvent::Stopped => ("Stopped".to_string(), SessionIndicator::Idle),
                        EngineEvent::Completed {
                            total_captures,
                            failures,
                        } => (
                            format!("Done ({total_captures} captures, {failures} failures)"),
                            SessionIndicator::Idle,
                        ),
                    };
                    let _ = proxy_events.send_event(UserEvent::Session(SessionEvent::Status {
                        text,
                        indicator,
                        latest_capture,
                    }));
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
                let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status {
                    text: format!("Session failed: {err}"),
                    indicator: SessionIndicator::Error,
                    latest_capture: None,
                }));
            }

            forward_task.abort();
            let _ = proxy.send_event(UserEvent::Session(SessionEvent::Completed));
        });
    });
}

fn open_path(path: PathBuf, highlight_running: bool, proxy: &EventLoopProxy<UserEvent>) {
    let target_exists = path.exists();
    let result = if target_exists {
        open(&path)
    } else {
        Err(opener::OpenError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "path missing",
        )))
    };

    let (text, indicator) = match result {
        Ok(()) => (
            format!("Opened {}", path.display()),
            if highlight_running {
                SessionIndicator::Running
            } else {
                SessionIndicator::Idle
            },
        ),
        Err(err) => (
            format!("Failed to open {}: {err}", path.display()),
            SessionIndicator::Error,
        ),
    };
    let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status {
        text,
        indicator,
        latest_capture: None,
    }));
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

struct IconSet {
    idle: Icon,
    running: Icon,
    paused: Icon,
    error: Icon,
}

impl IconSet {
    fn new() -> Self {
        Self {
            idle: build_state_icon([160, 160, 160]),
            running: build_state_icon([46, 204, 113]),
            paused: build_state_icon([255, 179, 0]),
            error: build_state_icon([231, 76, 60]),
        }
    }

    fn icon(&self, indicator: SessionIndicator) -> Icon {
        match indicator {
            SessionIndicator::Idle => self.idle.clone(),
            SessionIndicator::Running => self.running.clone(),
            SessionIndicator::Paused => self.paused.clone(),
            SessionIndicator::Error => self.error.clone(),
        }
    }
}

fn build_state_icon(fill_rgb: [u8; 3]) -> Icon {
    let (width, height) = (18, 18);
    let mut rgba = Vec::with_capacity(width * height * 4);
    let border = [40, 40, 40, 255];
    let fill = [fill_rgb[0], fill_rgb[1], fill_rgb[2], 255];
    let background = [0, 0, 0, 0];

    for y in 0..height {
        for x in 0..width {
            let is_border = x == 0 || y == 0 || x == width - 1 || y == height - 1;
            let is_center = (x > 4 && x < 13) && (y > 4 && y < 13);
            let pixel = if is_border {
                border
            } else if is_center {
                fill
            } else {
                background
            };
            rgba.extend_from_slice(&pixel);
        }
    }

    Icon::from_rgba(rgba, width as u32, height as u32).expect("valid tray icon")
}

fn update_tray_icon(
    tray_icon: &mut Option<TrayIcon>,
    icons: &IconSet,
    indicator: SessionIndicator,
) {
    if let Some(icon) = tray_icon.as_ref() {
        let _ = icon.set_icon(Some(icons.icon(indicator)));
    }
}

fn ensure_screen_recording_permission(
    app: &mut AppState,
    permission_status_item: &MenuItem,
    proxy: &EventLoopProxy<UserEvent>,
) -> bool {
    let status = screen_recording_status();
    app.set_permission_status(status);
    update_permission_menu(app, permission_status_item);

    if status.is_granted() {
        return true;
    }

    let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status {
        text: format!(
            "Screen Recording permission required. {}",
            screen_recording_help_message()
        ),
        indicator: SessionIndicator::Error,
        latest_capture: None,
    }));

    if let Err(err) = open_screen_recording_settings() {
        let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status {
            text: format!("Failed to open System Settings: {err}"),
            indicator: SessionIndicator::Error,
            latest_capture: None,
        }));
    } else {
        let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status {
            text: "Opening Screen Recording settings...".to_string(),
            indicator: SessionIndicator::Idle,
            latest_capture: None,
        }));
    }
    false
}
