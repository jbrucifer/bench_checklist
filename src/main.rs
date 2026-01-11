#![windows_subsystem = "windows"]

mod app;
mod autostart;
mod check_library;
mod checkers;
mod config;
mod fixer;
mod notifications;
mod ui;

use app::AppState;
use config::Config;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tray_icon::TrayIconEvent;
use ui::tray::{self, MENU_AUTOSTART, MENU_CHECK_NOW, MENU_EXIT, MENU_SETTINGS};

fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    tracing::info!("Starting Bench Checklist");

    // Load configuration
    let config_path = get_config_path();
    let config = load_config(&config_path)?;

    let check_count = config.get_scenario_checks().map(|c| c.len()).unwrap_or(0);
    tracing::info!("Loaded config with {} checks", check_count);

    // Create application state
    let app_state = AppState::new(config, config_path);

    // Create the system tray icon
    let tray = tray::create_tray_icon()?;

    // Run initial checks
    let (results, status) = app_state.run_checks();
    tracing::info!(
        "Initial check: {}/{} passed",
        results.iter().filter(|r| r.passed).count(),
        results.len()
    );

    tray::update_tray_icon(&tray, status, &app_state.get_tooltip());

    // Flag to control polling thread
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    // Spawn polling thread
    let app_state_polling = app_state.clone();
    let polling_handle = thread::spawn(move || {
        polling_loop(app_state_polling, running_clone);
    });

    // Flag to track if settings window is open
    let settings_open = Arc::new(AtomicBool::new(false));

    // Main event loop - Use Windows message pump for proper tray icon event handling
    let menu_receiver = tray::menu_channel();
    let tray_receiver = TrayIconEvent::receiver();

    use windows::Win32::UI::WindowsAndMessaging::{TranslateMessage, DispatchMessageW, MSG, PeekMessageW, PM_REMOVE, MessageBoxW, MB_YESNO, MB_ICONQUESTION, IDYES};
    use windows::Win32::Foundation::HWND;
    use windows::core::w;

    let mut msg: MSG = unsafe { std::mem::zeroed() };

    loop {
        // Process Windows messages (required for tray icon events on Windows)
        unsafe {
            while PeekMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        // Check for menu events (non-blocking)
        if let Ok(event) = menu_receiver.try_recv() {
            tracing::info!("Menu event received: {:?}", event);
            match event.id.0.as_str() {
                MENU_CHECK_NOW => {
                    tracing::info!("Manual check triggered");
                    let (_, status) = app_state.run_checks();
                    tray::update_tray_icon(&tray, status, &app_state.get_tooltip());
                }
                MENU_SETTINGS => {
                    open_settings(&settings_open, &app_state);
                }
                MENU_AUTOSTART => {
                    match autostart::toggle() {
                        Ok(enabled) => {
                            tracing::info!("Auto-start {}", if enabled { "enabled" } else { "disabled" });
                        }
                        Err(e) => {
                            tracing::error!("Failed to toggle auto-start: {}", e);
                        }
                    }
                }
                MENU_EXIT => {
                    tracing::info!("Exit requested");
                    // Show confirmation dialog
                    let result = unsafe {
                        MessageBoxW(
                            HWND::default(),
                            w!("Are you sure you want to exit Bench Checklist?"),
                            w!("Confirm Exit"),
                            MB_YESNO | MB_ICONQUESTION,
                        )
                    };
                    if result == IDYES {
                        // Save config before exiting (persists last used scenario)
                        if let Err(e) = app_state.save_config() {
                            tracing::error!("Failed to save config on exit: {}", e);
                        }
                        // Signal settings window to close
                        app_state.signal_exit();
                        running.store(false, Ordering::SeqCst);
                        break;
                    }
                }
                _ => {}
            }
        }

        // Check for tray icon click events
        if let Ok(event) = tray_receiver.try_recv() {
            tracing::info!("Tray event received: {:?}", event);
            match event {
                TrayIconEvent::DoubleClick { .. } => {
                    tracing::info!("Double-click detected, opening settings");
                    open_settings(&settings_open, &app_state);
                }
                TrayIconEvent::Click { button, .. } => {
                    tracing::info!("Single click detected with button: {:?}", button);
                    // Also open on single left click as a fallback
                    if matches!(button, tray_icon::MouseButton::Left) {
                        tracing::info!("Left click detected, opening settings");
                        open_settings(&settings_open, &app_state);
                    }
                }
                _ => {}
            }
        }

        // Update tray icon periodically
        let status = app_state.get_status();
        tray::update_tray_icon(&tray, status, &app_state.get_tooltip());

        // Small sleep to prevent busy-waiting
        thread::sleep(Duration::from_millis(100));
    }

    // Wait for polling thread to finish
    let _ = polling_handle.join();

    tracing::info!("Bench Checklist exiting");
    Ok(())
}

/// Polling loop that runs checks periodically
fn polling_loop(app_state: AppState, running: Arc<AtomicBool>) {
    while running.load(Ordering::SeqCst) {
        let interval = app_state.get_poll_interval();

        // Sleep in small increments to allow quick shutdown
        for _ in 0..(interval * 10) {
            if !running.load(Ordering::SeqCst) {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }

        if running.load(Ordering::SeqCst) {
            let (results, _status) = app_state.run_checks();
            tracing::debug!(
                "Periodic check: {}/{} passed",
                results.iter().filter(|r| r.passed).count(),
                results.len()
            );
        }
    }
}

/// Get the configuration file path
fn get_config_path() -> PathBuf {
    // First, check next to the executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let config_path = exe_dir.join("config").join("checklist.json");
            if config_path.exists() {
                return config_path;
            }
        }
    }

    // Then check current working directory
    let cwd_config = PathBuf::from("config").join("checklist.json");
    if cwd_config.exists() {
        return cwd_config;
    }

    // Default to exe directory
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("config")
        .join("checklist.json")
}

/// Load configuration, creating default if needed
fn load_config(path: &PathBuf) -> anyhow::Result<Config> {
    if path.exists() {
        Config::load(path)
    } else {
        tracing::info!("Config not found, creating default at {:?}", path);
        let config = Config::default();
        config.save(path)?;
        Ok(config)
    }
}

/// Open settings window if not already open
fn open_settings(settings_open: &Arc<AtomicBool>, app_state: &AppState) {
    if !settings_open.load(Ordering::SeqCst) {
        settings_open.store(true, Ordering::SeqCst);
        tracing::info!("Opening settings window...");
        let _ = ui::settings_window::SettingsWindow::run(app_state.clone());
        settings_open.store(false, Ordering::SeqCst);
        tracing::info!("Settings window closed");
    }
}
