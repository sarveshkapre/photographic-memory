use anyhow::Result;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use opener::open;
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
    open_accessibility_settings, open_screen_recording_settings, screen_recording_help_message,
    screen_recording_status,
};
use photographic_memory::privacy::{
    ConfigPrivacyGuard, MacOsForegroundAppProvider, PrivacyGuard, ensure_sample_privacy_config,
};
use photographic_memory::scheduler::CaptureSchedule;
use photographic_memory::screenshot::MacOsScreenshotProvider;
use photographic_memory::system_activity::{DisplaySleepStatus, ScreenLockStatus};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
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
    PermissionStatus(ScreenRecordingStatus),
}

#[derive(Debug, Clone)]
struct SessionSpec {
    name: &'static str,
    every: Duration,
    run_for: Duration,
    ai_enabled: bool,
    capture_stride: u64,
    max_session_bytes: Option<u64>,
}

struct SessionController {
    tx: tokio::sync::mpsc::UnboundedSender<ControlCommand>,
}

struct AppState {
    session: Option<SessionController>,
    latest_capture: Option<PathBuf>,
    permission_status: ScreenRecordingStatus,
    accessibility_status: AccessibilityStatus,
    hotkey_enabled: bool,
    privacy_guard: Arc<dyn PrivacyGuard>,
    high_freq_confirm_until: Option<Instant>,
}

impl AppState {
    fn new() -> Self {
        let privacy_guard: Arc<dyn PrivacyGuard> = Arc::new(ConfigPrivacyGuard::new(
            default_privacy_config_path(),
            MacOsForegroundAppProvider,
        ));
        Self {
            session: None,
            latest_capture: None,
            permission_status: screen_recording_status(),
            accessibility_status: accessibility_status(),
            hotkey_enabled: false,
            privacy_guard,
            high_freq_confirm_until: None,
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

    fn accessibility_status(&self) -> AccessibilityStatus {
        self.accessibility_status
    }

    fn set_accessibility_status(&mut self, status: AccessibilityStatus) {
        self.accessibility_status = status;
    }

    fn hotkey_enabled(&self) -> bool {
        self.hotkey_enabled
    }

    fn set_hotkey_enabled(&mut self, enabled: bool) {
        self.hotkey_enabled = enabled;
    }

    fn privacy_guard(&self) -> Arc<dyn PrivacyGuard> {
        self.privacy_guard.clone()
    }
}

fn main() -> Result<()> {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let proxy_for_menu = proxy.clone();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy_for_menu.send_event(UserEvent::Menu(event));
    }));

    let mut app = AppState::new();

    let mut hotkey_error: Option<String> = None;
    let hotkey_manager = match GlobalHotKeyManager::new() {
        Ok(manager) => Some(manager),
        Err(err) => {
            hotkey_error = Some(format!("Global hotkey init failed: {err}"));
            None
        }
    };

    let mut hotkey_id = None;
    if let Some(manager) = hotkey_manager.as_ref() {
        let instant_capture_hotkey = HotKey::new(Some(Modifiers::ALT), Code::KeyS);
        let id = instant_capture_hotkey.id();
        match manager.register(instant_capture_hotkey) {
            Ok(()) => {
                hotkey_id = Some(id);
                app.set_hotkey_enabled(true);
            }
            Err(err) => {
                hotkey_error = Some(format!("Failed to register hotkey Option+S: {err}"));
            }
        }
    }

    let proxy_for_hotkey = proxy.clone();
    GlobalHotKeyEvent::set_event_handler(Some(move |event| {
        let _ = proxy_for_hotkey.send_event(UserEvent::Hotkey(event));
    }));

    let status_item = MenuItem::new("Status: Idle", false, None);
    let permission_status_item = MenuItem::new("Screen Recording: Checking status...", false, None);
    let permission_recheck_item = MenuItem::new("Recheck Screen Recording Permission", true, None);
    let permission_settings_item = MenuItem::new("Open Screen Recording Settings...", true, None);
    let hotkey_status_item = MenuItem::new("Hotkey (Option+S): Checking status...", false, None);
    let hotkey_recheck_item = MenuItem::new("Recheck Accessibility Permission", true, None);
    let hotkey_settings_item = MenuItem::new("Open Accessibility Settings...", true, None);
    let privacy_status_item = MenuItem::new("Privacy: Loading policy...", false, None);
    let privacy_open_item = MenuItem::new("Open privacy policy...", true, None);
    let privacy_reload_item = MenuItem::new("Reload privacy policy", true, None);
    let immediate_item = MenuItem::new("Immediate Screenshot (Option+S)", true, None);
    let run_normal_item = MenuItem::new("Take screenshot every 2s for next 60 mins", true, None);
    let run_fast_item = MenuItem::new(
        "High-frequency: 30ms for 10 mins (saved ~1/sec, local only)",
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
    menu.append(&hotkey_status_item)?;
    menu.append(&hotkey_recheck_item)?;
    menu.append(&hotkey_settings_item)?;
    menu.append(&privacy_status_item)?;
    menu.append(&privacy_open_item)?;
    menu.append(&privacy_reload_item)?;
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
    update_recent_capture_menu(&app, &recent_capture_item);
    update_permission_menu(&app, &permission_status_item);
    update_hotkey_menu(&app, &hotkey_status_item);
    update_privacy_menu(&app, &privacy_status_item);
    update_capture_menu(&mut app, &immediate_item, &run_normal_item, &run_fast_item);

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
                        status_item.set_text("Status: Failed to init tray icon");
                    }
                }

                // Refresh permission state on launch so first-run onboarding is accurate.
                let permission = screen_recording_status();
                app.set_permission_status(permission);
                update_permission_menu(&app, &permission_status_item);
                update_capture_menu(&mut app, &immediate_item, &run_normal_item, &run_fast_item);

                if let Some(message) = hotkey_error.take() {
                    app.set_accessibility_status(accessibility_status());
                    update_hotkey_menu(&app, &hotkey_status_item);
                    update_capture_menu(
                        &mut app,
                        &immediate_item,
                        &run_normal_item,
                        &run_fast_item,
                    );
                    let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status {
                        text: format!("{message}. {}", accessibility_help_message()),
                        indicator: SessionIndicator::Error,
                        latest_capture: None,
                    }));
                }

                update_idle_status(&app, &status_item, &mut tray_icon, &icons);
            }
            Event::UserEvent(UserEvent::Hotkey(hotkey_event)) => {
                let matches = hotkey_id.as_ref().is_some_and(|id| hotkey_event.id == *id);
                if matches && hotkey_event.state == HotKeyState::Pressed {
                    app.high_freq_confirm_until = None;
                    start_session(
                        &mut app,
                        &proxy,
                        &permission_status_item,
                        &privacy_status_item,
                        SessionSpec {
                            name: "Immediate",
                            every: Duration::from_secs(1),
                            run_for: Duration::from_millis(10),
                            ai_enabled: true,
                            capture_stride: 1,
                            max_session_bytes: None,
                        },
                        false,
                    );
                    refresh_controls(&app, &pause_item, &resume_item, &stop_item);
                }
            }
            Event::UserEvent(UserEvent::Menu(menu_event)) => {
                let is_fast_click = menu_event.id == run_fast_item.id();
                if !is_fast_click {
                    app.high_freq_confirm_until = None;
                }

                if menu_event.id == immediate_item.id() {
                    start_session(
                        &mut app,
                        &proxy,
                        &permission_status_item,
                        &privacy_status_item,
                        SessionSpec {
                            name: "Immediate",
                            every: Duration::from_secs(1),
                            run_for: Duration::from_millis(10),
                            ai_enabled: true,
                            capture_stride: 1,
                            max_session_bytes: None,
                        },
                        true,
                    );
                } else if menu_event.id == permission_recheck_item.id() {
                    let status = screen_recording_status();
                    app.set_permission_status(status);
                    update_permission_menu(&app, &permission_status_item);
                    update_capture_menu(
                        &mut app,
                        &immediate_item,
                        &run_normal_item,
                        &run_fast_item,
                    );
                    update_idle_status(&app, &status_item, &mut tray_icon, &icons);
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
                } else if menu_event.id == hotkey_recheck_item.id() {
                    let status = accessibility_status();
                    app.set_accessibility_status(status);
                    update_hotkey_menu(&app, &hotkey_status_item);
                    update_capture_menu(
                        &mut app,
                        &immediate_item,
                        &run_normal_item,
                        &run_fast_item,
                    );

                    if !app.hotkey_enabled()
                        && matches!(
                            status,
                            AccessibilityStatus::Granted | AccessibilityStatus::NotSupported
                        )
                        && hotkey_id.is_none()
                        && let Some(manager) = hotkey_manager.as_ref()
                    {
                        let hotkey = HotKey::new(Some(Modifiers::ALT), Code::KeyS);
                        let id = hotkey.id();
                        if manager.register(hotkey).is_ok() {
                            hotkey_id = Some(id);
                            app.set_hotkey_enabled(true);
                            update_hotkey_menu(&app, &hotkey_status_item);
                        }
                    }

                    let text = match status {
                        AccessibilityStatus::Granted => {
                            "Accessibility permission granted.".to_string()
                        }
                        AccessibilityStatus::NotSupported => {
                            "Accessibility permission not required.".to_string()
                        }
                        AccessibilityStatus::Denied => format!(
                            "Accessibility permission denied. {}",
                            accessibility_help_message()
                        ),
                    };
                    let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status {
                        text,
                        indicator: if matches!(status, AccessibilityStatus::Denied) {
                            SessionIndicator::Error
                        } else {
                            SessionIndicator::Idle
                        },
                        latest_capture: None,
                    }));
                } else if menu_event.id == hotkey_settings_item.id() {
                    let result = open_accessibility_settings();
                    let (text, indicator) = match result {
                        Ok(()) => (
                            "Opening Accessibility settings...".to_string(),
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
                        &privacy_status_item,
                        SessionSpec {
                            name: "2s/60m",
                            every: Duration::from_secs(2),
                            run_for: Duration::from_secs(60 * 60),
                            ai_enabled: true,
                            capture_stride: 1,
                            max_session_bytes: None,
                        },
                        true,
                    );
                } else if menu_event.id == run_fast_item.id() {
                    if confirm_high_frequency_start(&mut app, &proxy) {
                        start_session(
                            &mut app,
                            &proxy,
                            &permission_status_item,
                            &privacy_status_item,
                            SessionSpec {
                                name: "30ms/10m",
                                every: Duration::from_millis(30),
                                run_for: Duration::from_secs(10 * 60),
                                ai_enabled: false,
                                capture_stride: 34,
                                max_session_bytes: Some(512 * 1024 * 1024),
                            },
                            true,
                        );
                    }
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
                    app.send(ControlCommand::UserPause);
                } else if menu_event.id == resume_item.id() {
                    app.send(ControlCommand::UserResume);
                } else if menu_event.id == stop_item.id() {
                    app.send(ControlCommand::Stop);
                } else if menu_event.id == quit_item.id() {
                    app.send(ControlCommand::Stop);
                    *control_flow = ControlFlow::Exit;
                } else if menu_event.id == privacy_open_item.id() {
                    let config_path = default_privacy_config_path();
                    let _ = ensure_sample_privacy_config(&config_path);
                    open_path(config_path, app.is_running(), &proxy);
                } else if menu_event.id == privacy_reload_item.id() {
                    let (text, indicator) = match app.privacy_guard().reload() {
                        Ok(()) => (
                            "Privacy policy reloaded.".to_string(),
                            SessionIndicator::Idle,
                        ),
                        Err(err) => (
                            format!("Privacy policy error: {err}"),
                            SessionIndicator::Error,
                        ),
                    };
                    update_privacy_menu(&app, &privacy_status_item);
                    let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status {
                        text,
                        indicator,
                        latest_capture: None,
                    }));
                }
                refresh_controls(&app, &pause_item, &resume_item, &stop_item);
                update_capture_menu(&mut app, &immediate_item, &run_normal_item, &run_fast_item);
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
                    status_item.set_text(format!("Status: {text}"));
                    update_tray_icon(&mut tray_icon, &icons, indicator);
                    update_recent_capture_menu(&app, &recent_capture_item);
                }
                SessionEvent::Completed => {
                    app.session = None;
                    update_idle_status(&app, &status_item, &mut tray_icon, &icons);
                    refresh_controls(&app, &pause_item, &resume_item, &stop_item);
                    update_recent_capture_menu(&app, &recent_capture_item);
                    update_capture_menu(
                        &mut app,
                        &immediate_item,
                        &run_normal_item,
                        &run_fast_item,
                    );
                }
                SessionEvent::PermissionStatus(status) => {
                    app.set_permission_status(status);
                    update_permission_menu(&app, &permission_status_item);
                    update_capture_menu(
                        &mut app,
                        &immediate_item,
                        &run_normal_item,
                        &run_fast_item,
                    );
                    update_idle_status(&app, &status_item, &mut tray_icon, &icons);
                }
            },
            _ => {}
        }
    });
}

fn update_idle_status(
    app: &AppState,
    status_item: &MenuItem,
    tray_icon: &mut Option<TrayIcon>,
    icons: &IconSet,
) {
    if app.is_running() {
        return;
    }

    if matches!(app.permission_status(), ScreenRecordingStatus::Denied) {
        status_item.set_text("Status: Blocked (grant Screen Recording)");
        update_tray_icon(tray_icon, icons, SessionIndicator::Error);
        return;
    }

    status_item.set_text("Status: Idle");
    update_tray_icon(tray_icon, icons, SessionIndicator::Idle);
}

fn refresh_controls(
    app: &AppState,
    pause_item: &MenuItem,
    resume_item: &MenuItem,
    stop_item: &MenuItem,
) {
    let running = app.is_running();
    pause_item.set_enabled(running);
    resume_item.set_enabled(running);
    stop_item.set_enabled(running);
}

fn update_capture_menu(
    app: &mut AppState,
    immediate_item: &MenuItem,
    run_normal_item: &MenuItem,
    run_fast_item: &MenuItem,
) {
    let blocked = matches!(app.permission_status(), ScreenRecordingStatus::Denied);
    let running = app.is_running();
    let can_start = !blocked && !running;

    if blocked || running {
        app.high_freq_confirm_until = None;
    } else if let Some(until) = app.high_freq_confirm_until
        && Instant::now() >= until
    {
        app.high_freq_confirm_until = None;
    }

    immediate_item.set_enabled(can_start);
    run_normal_item.set_enabled(can_start);
    run_fast_item.set_enabled(can_start);

    let immediate_text = if blocked {
        "Immediate Screenshot (blocked: Screen Recording)".to_string()
    } else if app.hotkey_enabled() {
        "Immediate Screenshot (Option+S)".to_string()
    } else {
        "Immediate Screenshot (Option+S disabled)".to_string()
    };
    immediate_item.set_text(immediate_text);

    let fast_text = if blocked {
        "High-frequency: 30ms for 10 mins (blocked: Screen Recording)".to_string()
    } else if app.high_freq_confirm_until.is_some() {
        "Confirm high-frequency start (tap again within 10s)".to_string()
    } else {
        "High-frequency: 30ms for 10 mins (saved ~1/sec, local only)".to_string()
    };
    run_fast_item.set_text(fast_text);
}

fn confirm_high_frequency_start(app: &mut AppState, proxy: &EventLoopProxy<UserEvent>) -> bool {
    let now = Instant::now();
    if let Some(until) = app.high_freq_confirm_until
        && now < until
    {
        app.high_freq_confirm_until = None;
        return true;
    }

    app.high_freq_confirm_until = Some(now + Duration::from_secs(10));
    let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status {
        text: "High-frequency mode is local-only and sampled; select again within 10s to confirm."
            .to_string(),
        indicator: SessionIndicator::Idle,
        latest_capture: None,
    }));
    false
}

fn update_recent_capture_menu(app: &AppState, recent_capture_item: &MenuItem) {
    if let Some(path) = app.latest_capture() {
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("capture.png");
        recent_capture_item.set_enabled(true);
        recent_capture_item.set_text(format!("Open latest capture ({filename})"));
    } else {
        recent_capture_item.set_enabled(false);
        recent_capture_item.set_text("Open latest capture");
    }
}

fn update_permission_menu(app: &AppState, permission_status_item: &MenuItem) {
    let text = match app.permission_status() {
        ScreenRecordingStatus::Granted => "Screen Recording: Granted",
        ScreenRecordingStatus::Denied => "Screen Recording: Blocked (open System Settings)",
        ScreenRecordingStatus::NotSupported => "Screen Recording: Not required",
    };
    permission_status_item.set_text(text);
}

fn update_hotkey_menu(app: &AppState, hotkey_status_item: &MenuItem) {
    let accessibility = app.accessibility_status();
    let text = if app.hotkey_enabled() {
        "Hotkey (Option+S): Enabled".to_string()
    } else {
        match accessibility {
            AccessibilityStatus::Denied => {
                "Hotkey (Option+S): Disabled (grant Accessibility)".to_string()
            }
            AccessibilityStatus::Granted => {
                "Hotkey (Option+S): Disabled (recheck permission)".to_string()
            }
            AccessibilityStatus::NotSupported => "Hotkey (Option+S): Disabled".to_string(),
        }
    };
    hotkey_status_item.set_text(text);
}

fn update_privacy_menu(app: &AppState, privacy_status_item: &MenuItem) {
    let status = app.privacy_guard().status();
    let enabled_text = if status.enabled { "Active" } else { "Disabled" };
    let filename = status
        .config_path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("privacy.toml");
    privacy_status_item.set_text(format!(
        "Privacy: {enabled_text} ({}, {filename})",
        status.rule_summary
    ));
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
    privacy_status_item: &MenuItem,
    spec: SessionSpec,
    auto_open_permission_settings: bool,
) {
    app.high_freq_confirm_until = None;

    if app.is_running() {
        let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status {
            text: "Already running. Use Stop before starting a new session.".to_string(),
            indicator: SessionIndicator::Running,
            latest_capture: None,
        }));
        return;
    }

    if !ensure_screen_recording_permission(
        app,
        permission_status_item,
        proxy,
        auto_open_permission_settings,
    ) {
        return;
    }

    if let Err(err) = app.privacy_guard().reload() {
        update_privacy_menu(app, privacy_status_item);
        let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status {
            text: format!("Privacy policy invalid: {err}"),
            indicator: SessionIndicator::Error,
            latest_capture: None,
        }));
        return;
    }
    update_privacy_menu(app, privacy_status_item);

    let (control_tx, control_rx) = tokio::sync::mpsc::unbounded_channel();
    app.session = Some(SessionController {
        tx: control_tx.clone(),
    });

    let proxy = proxy.clone();
    let privacy_guard = app.privacy_guard();
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
                if spec.capture_stride > 1 {
                    let approx_ms =
                        spec.every.as_millis().saturating_mul(spec.capture_stride as u128);
                    let cap_text = spec
                        .max_session_bytes
                        .map(|bytes| format!(", cap {:.0} MB", bytes as f64 / (1024.0 * 1024.0)))
                        .unwrap_or_default();
                    let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status {
                        text: format!(
                            "High-frequency safeguards: local-only analysis and capture sampling (~{}ms){}",
                            approx_ms, cap_text
                        ),
                        indicator: SessionIndicator::Running,
                        latest_capture: None,
                    }));
                } else {
                    let _ = proxy.send_event(UserEvent::Session(SessionEvent::Status {
                        text: "Running high-frequency mode with local analysis only".to_string(),
                        indicator: SessionIndicator::Running,
                        latest_capture: None,
                    }));
                }
            }

            let engine = CaptureEngine::new(
                screenshot_provider,
                analyzer,
                privacy_guard,
                ContextLog::new(context_path),
            );
            let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<EngineEvent>();
            let session_control_tx = control_tx.clone();
            let permission_proxy = proxy.clone();
            let permission_guard = spawn_permission_watch(session_control_tx, move |status| {
                let _ = permission_proxy
                    .send_event(UserEvent::Session(SessionEvent::PermissionStatus(status)));

                if matches!(status, ScreenRecordingStatus::NotSupported) {
                    return;
                }

                let (text, indicator) = match status {
                    ScreenRecordingStatus::Denied => (
                        "Screen Recording permission revoked. Auto-pausing session.".to_string(),
                        SessionIndicator::Error,
                    ),
                    ScreenRecordingStatus::Granted => (
                        "Screen Recording permission restored. Auto-resuming session.".to_string(),
                        SessionIndicator::Running,
                    ),
                    ScreenRecordingStatus::NotSupported => unreachable!(),
                };

                let _ = permission_proxy.send_event(UserEvent::Session(SessionEvent::Status {
                    text,
                    indicator,
                    latest_capture: None,
                }));
            });

            let activity_proxy = proxy.clone();
            let activity_guard = spawn_activity_watch(control_tx.clone(), move |event| {
                let (text, indicator) = match event {
                    ActivityEvent::ScreenLock(status) => match status {
                        ScreenLockStatus::Locked => (
                            "Screen locked. Auto-pausing session.".to_string(),
                            SessionIndicator::Paused,
                        ),
                        ScreenLockStatus::Unlocked => (
                            "Screen unlocked. Auto-resuming session.".to_string(),
                            SessionIndicator::Running,
                        ),
                        ScreenLockStatus::Unknown | ScreenLockStatus::NotSupported => return,
                    },
                    ActivityEvent::DisplaySleep(status) => match status {
                        DisplaySleepStatus::Asleep => (
                            "Display asleep. Auto-pausing session.".to_string(),
                            SessionIndicator::Paused,
                        ),
                        DisplaySleepStatus::Awake => (
                            "Display awake. Auto-resuming session.".to_string(),
                            SessionIndicator::Running,
                        ),
                        DisplaySleepStatus::Unknown | DisplaySleepStatus::NotSupported => return,
                    },
                };

                let _ = activity_proxy.send_event(UserEvent::Session(SessionEvent::Status {
                    text,
                    indicator,
                    latest_capture: None,
                }));
            });

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
                        EngineEvent::AutoPaused { reason } => (
                            format!("Auto-paused: {reason:?}"),
                            SessionIndicator::Paused,
                        ),
                        EngineEvent::AutoResumed { reason } => (
                            format!("Auto-resumed: {reason:?}"),
                            SessionIndicator::Running,
                        ),
                        EngineEvent::CaptureSkipped { tick_index, reason } => (
                            format!("Running {session_name} (tick #{tick_index} skipped: {reason})"),
                            SessionIndicator::Running,
                        ),
                        EngineEvent::CaptureSucceeded {
                            capture_index,
                            path,
                        } => {
                            latest_capture = Some(path);
                            (
                                format!("Running {session_name} (capture #{capture_index})"),
                                SessionIndicator::Running,
                            )
                        }
                        EngineEvent::CaptureFailed { capture_index, .. } => (
                            format!("Running {session_name} (error at #{capture_index})"),
                            SessionIndicator::Error,
                        ),
                        EngineEvent::DiskCleanup {
                            deleted_files,
                            freed_bytes,
                            remaining_bytes,
                        } => (
                            format!(
                                "Disk cleanup: removed {deleted_files} files ({:.1} MB freed, {:.1} MB left)",
                                freed_bytes as f64 / (1024.0 * 1024.0),
                                remaining_bytes as f64 / (1024.0 * 1024.0)
                            ),
                            SessionIndicator::Running,
                        ),
                        EngineEvent::BudgetExceeded {
                            bytes_written,
                            limit_bytes,
                        } => (
                            format!(
                                "Storage cap reached: {:.1} MB > {:.1} MB (stopping)",
                                bytes_written as f64 / (1024.0 * 1024.0),
                                limit_bytes as f64 / (1024.0 * 1024.0)
                            ),
                            SessionIndicator::Idle,
                        ),
                        EngineEvent::Stopped => ("Stopped".to_string(), SessionIndicator::Idle),
                        EngineEvent::Completed {
                            total_ticks,
                            captures,
                            skipped,
                            failures,
                        } => (
                            format!(
                                "Done ({captures} captures, {skipped} skipped, {failures} failures, {total_ticks} ticks)"
                            ),
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
                        min_free_disk_bytes: DEFAULT_MIN_FREE_DISK_BYTES,
                        capture_stride: spec.capture_stride,
                        max_session_bytes: spec.max_session_bytes,
                    },
                    Some(control_rx),
                    Some(event_tx),
                )
                .await;

            if let Some(handle) = permission_guard {
                handle.abort();
                let _ = handle.await;
            }

            if let Some(handle) = activity_guard {
                handle.abort();
                let _ = handle.await;
            }

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
    auto_open_settings: bool,
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

    if auto_open_settings {
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
    }
    false
}
