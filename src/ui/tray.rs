use crate::autostart;
use crate::checkers::OverallStatus;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

/// Menu item IDs
pub const MENU_CHECK_NOW: &str = "check_now";
pub const MENU_SETTINGS: &str = "settings";
pub const MENU_AUTOSTART: &str = "autostart";
pub const MENU_EXIT: &str = "exit";

/// Create the tray icon
pub fn create_tray_icon() -> anyhow::Result<TrayIcon> {
    let menu = create_menu()?;

    // Create a simple colored icon (green by default)
    let icon = create_status_icon(OverallStatus::AllPassed)?;

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)  // Allow double-click to work
        .with_tooltip("Bench Checklist - Starting...")
        .with_icon(icon)
        .build()?;

    Ok(tray)
}

/// Create the context menu
fn create_menu() -> anyhow::Result<Menu> {
    let menu = Menu::new();

    let check_now = MenuItem::with_id(MENU_CHECK_NOW, "Check Now", true, None);
    let settings = MenuItem::with_id(MENU_SETTINGS, "Settings...", true, None);
    let autostart_enabled = autostart::is_enabled();
    let autostart = CheckMenuItem::with_id(MENU_AUTOSTART, "Start with Windows", true, autostart_enabled, None);
    let separator = PredefinedMenuItem::separator();
    let exit = MenuItem::with_id(MENU_EXIT, "Exit", true, None);

    menu.append(&check_now)?;
    menu.append(&settings)?;
    menu.append(&autostart)?;
    menu.append(&separator)?;
    menu.append(&exit)?;

    Ok(menu)
}

/// Create a colored icon based on status with checkmark overlay
pub fn create_status_icon(status: OverallStatus) -> anyhow::Result<Icon> {
    let (r, g, b) = match status {
        OverallStatus::AllPassed => (0x10, 0xB9, 0x81),   // Green (#10B981)
        OverallStatus::SomeFailed => (0xF5, 0x9E, 0x0B),  // Amber (#F59E0B)
        OverallStatus::AllFailed => (0xEF, 0x44, 0x44),   // Red (#EF4444)
    };

    // Create a 32x32 icon with the status color and pattern
    let size = 32;
    let mut rgba = Vec::with_capacity(size * size * 4);

    for y in 0..size {
        for x in 0..size {
            // Create a filled circle
            let cx = (x as i32) - (size as i32 / 2);
            let cy = (y as i32) - (size as i32 / 2);
            let radius = size as i32 / 2 - 2;
            let in_circle = cx * cx + cy * cy <= radius * radius;

            // Add checkmark pattern for AllPassed, X pattern for AllFailed
            let is_pattern = match status {
                OverallStatus::AllPassed => {
                    // Simple checkmark (approximate)
                    let short_arm = x >= 8 && x <= 12 && y >= 14 && y <= 20 && (x as i32 - 10).abs() == (y as i32 - 17);
                    let long_arm = x >= 12 && x <= 24 && y >= 8 && y <= 18 && (x as i32 - 18).abs() * 2 == -(y as i32 - 13);
                    short_arm || long_arm
                }
                OverallStatus::AllFailed => {
                    // X pattern
                    ((x as i32 - y as i32).abs() <= 2 || (x as i32 + y as i32 - size as i32).abs() <= 2)
                        && (x >= 8 && x <= 24 && y >= 8 && y <= 24)
                }
                _ => false,
            };

            if in_circle {
                if is_pattern {
                    // White pattern overlay
                    rgba.push(255);
                    rgba.push(255);
                    rgba.push(255);
                    rgba.push(255);
                } else {
                    rgba.push(r);
                    rgba.push(g);
                    rgba.push(b);
                    rgba.push(255);
                }
            } else {
                // Transparent outside the circle
                rgba.push(0);
                rgba.push(0);
                rgba.push(0);
                rgba.push(0);
            }
        }
    }

    Icon::from_rgba(rgba, size as u32, size as u32).map_err(|e| anyhow::anyhow!("{}", e))
}

/// Update tray icon based on status
pub fn update_tray_icon(tray: &TrayIcon, status: OverallStatus, tooltip: &str) {
    if let Ok(icon) = create_status_icon(status) {
        let _ = tray.set_icon(Some(icon));
    }
    let _ = tray.set_tooltip(Some(tooltip));
}

/// Get menu events receiver
pub fn menu_channel() -> crossbeam_channel::Receiver<MenuEvent> {
    MenuEvent::receiver().clone()
}
