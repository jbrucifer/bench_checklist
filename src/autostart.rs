use std::ptr;
use windows::core::PCWSTR;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_SET_VALUE, REG_SZ,
};

const APP_NAME: &str = "BenchChecklist";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Convert a Rust string to a wide string (UTF-16)
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Check if auto-start is enabled
pub fn is_enabled() -> bool {
    let run_key_wide = to_wide(RUN_KEY);
    let app_name_wide = to_wide(APP_NAME);

    unsafe {
        let mut hkey = HKEY::default();

        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(run_key_wide.as_ptr()),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        );

        if result != ERROR_SUCCESS {
            return false;
        }

        // Try to query the value - if it exists, auto-start is enabled
        let mut data_size: u32 = 0;
        let result = windows::Win32::System::Registry::RegQueryValueExW(
            hkey,
            PCWSTR::from_raw(app_name_wide.as_ptr()),
            Some(ptr::null()),
            None,
            None,
            Some(&mut data_size),
        );

        let _ = RegCloseKey(hkey);

        result == ERROR_SUCCESS
    }
}

/// Enable auto-start by adding to registry Run key
pub fn enable() -> Result<(), String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?;

    let exe_path_str = exe_path
        .to_str()
        .ok_or_else(|| "Executable path contains invalid characters".to_string())?;

    let run_key_wide = to_wide(RUN_KEY);
    let app_name_wide = to_wide(APP_NAME);
    let exe_path_wide = to_wide(exe_path_str);

    unsafe {
        let mut hkey = HKEY::default();

        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(run_key_wide.as_ptr()),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        );

        if result != ERROR_SUCCESS {
            return Err(format!("Failed to open registry key: error {}", result.0));
        }

        // Set the value (path includes null terminator in byte count)
        let exe_bytes: &[u8] = std::slice::from_raw_parts(
            exe_path_wide.as_ptr() as *const u8,
            exe_path_wide.len() * 2,
        );

        let result = RegSetValueExW(
            hkey,
            PCWSTR::from_raw(app_name_wide.as_ptr()),
            0,
            REG_SZ,
            Some(exe_bytes),
        );

        let _ = RegCloseKey(hkey);

        if result != ERROR_SUCCESS {
            return Err(format!("Failed to set registry value: error {}", result.0));
        }

        Ok(())
    }
}

/// Disable auto-start by removing from registry Run key
pub fn disable() -> Result<(), String> {
    let run_key_wide = to_wide(RUN_KEY);
    let app_name_wide = to_wide(APP_NAME);

    unsafe {
        let mut hkey = HKEY::default();

        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR::from_raw(run_key_wide.as_ptr()),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        );

        if result != ERROR_SUCCESS {
            return Err(format!("Failed to open registry key: error {}", result.0));
        }

        let result = RegDeleteValueW(hkey, PCWSTR::from_raw(app_name_wide.as_ptr()));

        let _ = RegCloseKey(hkey);

        if result != ERROR_SUCCESS {
            return Err(format!("Failed to delete registry value: error {}", result.0));
        }

        Ok(())
    }
}

/// Toggle auto-start state
pub fn toggle() -> Result<bool, String> {
    if is_enabled() {
        disable()?;
        Ok(false)
    } else {
        enable()?;
        Ok(true)
    }
}
