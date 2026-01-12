//! Display settings checks for resolution, refresh rate, and HDR status

use crate::checkers::CheckResult;
use crate::config::CheckConfig;
use windows::Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW, ENUM_CURRENT_SETTINGS};

/// Get current display settings (width, height, refresh rate)
fn get_current_display() -> Result<(u32, u32, u32), String> {
    let mut devmode = DEVMODEW {
        dmSize: std::mem::size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };

    unsafe {
        if EnumDisplaySettingsW(None, ENUM_CURRENT_SETTINGS, &mut devmode).as_bool() {
            Ok((
                devmode.dmPelsWidth,
                devmode.dmPelsHeight,
                devmode.dmDisplayFrequency,
            ))
        } else {
            Err("Failed to enumerate display settings".to_string())
        }
    }
}

/// Check display resolution against expected (e.g., "3840x2160")
pub fn check_resolution(config: &CheckConfig) -> CheckResult {
    let expected = config.expected_value.as_deref().unwrap_or("1920x1080");

    match get_current_display() {
        Ok((width, height, _)) => {
            let current = format!("{}x{}", width, height);
            if current == expected {
                CheckResult::pass(&config.id, &config.name, &current, expected)
            } else {
                CheckResult::fail(&config.id, &config.name, &current, expected)
            }
        }
        Err(e) => CheckResult::error(&config.id, &config.name, &e),
    }
}

/// Check refresh rate against minimum (e.g., "144")
pub fn check_refresh_rate(config: &CheckConfig) -> CheckResult {
    let expected_str = config.expected_value.as_deref().unwrap_or("60");
    let expected_hz: u32 = expected_str.parse().unwrap_or(60);

    match get_current_display() {
        Ok((_, _, hz)) => {
            let current = format!("{}Hz", hz);
            let expected_display = format!("{}Hz+", expected_hz);
            if hz >= expected_hz {
                CheckResult::pass(&config.id, &config.name, &current, &expected_display)
            } else {
                CheckResult::fail(&config.id, &config.name, &current, &expected_display)
            }
        }
        Err(e) => CheckResult::error(&config.id, &config.name, &e),
    }
}

/// Check if HDR is enabled (registry-based)
pub fn check_hdr(config: &CheckConfig) -> CheckResult {
    let expected = config.expected_value.as_deref().unwrap_or("1");

    // Try to read HDR status from registry
    let hdr_enabled = check_hdr_registry();

    let current = if hdr_enabled { "Enabled" } else { "Disabled" };
    let expected_display = if expected == "1" { "Enabled" } else { "Disabled" };

    if (expected == "1" && hdr_enabled) || (expected == "0" && !hdr_enabled) {
        CheckResult::pass(&config.id, &config.name, current, expected_display)
    } else {
        CheckResult::fail(&config.id, &config.name, current, expected_display)
    }
}

/// Check HDR status from Windows registry
fn check_hdr_registry() -> bool {
    use crate::checkers::registry;

    // Try the main HDR setting location (Windows 11)
    // Path: HKCU\Software\Microsoft\Windows\CurrentVersion\VideoSettings
    // Key: GlobalHDRState or EnableHDRForDisplay
    if let Ok(value) = registry::read_dword_value(
        "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\VideoSettings",
        "GlobalHDRState",
    ) {
        return value == 1;
    }

    // Alternative location for EnableHDRForDisplay
    if let Ok(value) = registry::read_dword_value(
        "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\VideoSettings",
        "EnableHDRForDisplay",
    ) {
        return value == 1;
    }

    // Try per-monitor HDR settings path
    // This is a fallback for systems with different registry structures
    false
}

/// Get current display info as a formatted string (for UI display)
#[allow(dead_code)]
pub fn get_display_info() -> String {
    match get_current_display() {
        Ok((width, height, hz)) => {
            format!("{}x{} @ {}Hz", width, height, hz)
        }
        Err(_) => "Unknown".to_string(),
    }
}
