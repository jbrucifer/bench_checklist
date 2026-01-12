use crate::checkers::CheckResult;
use crate::config::CheckConfig;
use std::ptr;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{ERROR_SUCCESS, ERROR_FILE_NOT_FOUND, ERROR_ACCESS_DENIED};
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_DWORD, REG_SZ, REG_VALUE_TYPE,
};

/// Parse the root key from a registry path
pub fn parse_root_key(path: &str) -> Option<(HKEY, &str)> {
    if let Some(subpath) = path.strip_prefix("HKCU\\") {
        Some((HKEY_CURRENT_USER, subpath))
    } else if let Some(subpath) = path.strip_prefix("HKEY_CURRENT_USER\\") {
        Some((HKEY_CURRENT_USER, subpath))
    } else if let Some(subpath) = path.strip_prefix("HKLM\\") {
        Some((HKEY_LOCAL_MACHINE, subpath))
    } else if let Some(subpath) = path.strip_prefix("HKEY_LOCAL_MACHINE\\") {
        Some((HKEY_LOCAL_MACHINE, subpath))
    } else {
        None
    }
}

/// Convert a Rust string to a wide string (UTF-16)
pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Check if a registry path requires admin privileges (HKLM)
pub fn requires_admin(path: &str) -> bool {
    path.starts_with("HKLM\\") || path.starts_with("HKEY_LOCAL_MACHINE\\")
}

/// Read a DWORD value from the registry
fn read_dword(root: HKEY, subkey: &str, value_name: &str) -> Result<u32, String> {
    let subkey_wide = to_wide(subkey);
    let value_wide = to_wide(value_name);

    unsafe {
        let mut hkey = HKEY::default();

        let result = RegOpenKeyExW(
            root,
            PCWSTR::from_raw(subkey_wide.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        );

        if result == ERROR_FILE_NOT_FOUND {
            return Err("Key not found".to_string());
        } else if result == ERROR_ACCESS_DENIED {
            return Err("Access denied (run as admin?)".to_string());
        } else if result != ERROR_SUCCESS {
            return Err(format!("Failed to open key (error {})", result.0));
        }

        let mut data: u32 = 0;
        let mut data_size: u32 = std::mem::size_of::<u32>() as u32;
        let mut value_type: REG_VALUE_TYPE = REG_DWORD;

        let result = RegQueryValueExW(
            hkey,
            PCWSTR::from_raw(value_wide.as_ptr()),
            Some(ptr::null()),
            Some(&mut value_type),
            Some(ptr::addr_of_mut!(data) as *mut u8),
            Some(&mut data_size),
        );

        let _ = RegCloseKey(hkey);

        if result == ERROR_FILE_NOT_FOUND {
            return Err("Value not found".to_string());
        } else if result != ERROR_SUCCESS {
            return Err(format!("Failed to read value (error {})", result.0));
        }

        Ok(data)
    }
}

/// Read a string value from the registry
fn read_string(root: HKEY, subkey: &str, value_name: &str) -> Result<String, String> {
    let subkey_wide = to_wide(subkey);
    let value_wide = to_wide(value_name);

    unsafe {
        let mut hkey = HKEY::default();

        let result = RegOpenKeyExW(
            root,
            PCWSTR::from_raw(subkey_wide.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        );

        if result == ERROR_FILE_NOT_FOUND {
            return Err("Key not found".to_string());
        } else if result == ERROR_ACCESS_DENIED {
            return Err("Access denied (run as admin?)".to_string());
        } else if result != ERROR_SUCCESS {
            return Err(format!("Failed to open key (error {})", result.0));
        }

        // First, get the size needed
        let mut data_size: u32 = 0;
        let mut value_type: REG_VALUE_TYPE = REG_SZ;

        let result = RegQueryValueExW(
            hkey,
            PCWSTR::from_raw(value_wide.as_ptr()),
            Some(ptr::null()),
            Some(&mut value_type),
            None,
            Some(&mut data_size),
        );

        if result == ERROR_FILE_NOT_FOUND {
            let _ = RegCloseKey(hkey);
            return Err("Value not found".to_string());
        } else if result != ERROR_SUCCESS {
            let _ = RegCloseKey(hkey);
            return Err(format!("Failed to query value (error {})", result.0));
        }

        // Allocate buffer and read the value
        let mut buffer: Vec<u8> = vec![0; data_size as usize];

        let result = RegQueryValueExW(
            hkey,
            PCWSTR::from_raw(value_wide.as_ptr()),
            Some(ptr::null()),
            Some(&mut value_type),
            Some(buffer.as_mut_ptr()),
            Some(&mut data_size),
        );

        let _ = RegCloseKey(hkey);

        if result != ERROR_SUCCESS {
            return Err(format!("Failed to read value (error {})", result.0));
        }

        // Convert wide string to Rust string
        let wide_slice: &[u16] =
            std::slice::from_raw_parts(buffer.as_ptr() as *const u16, data_size as usize / 2);

        // Find null terminator and convert
        let end = wide_slice.iter().position(|&c| c == 0).unwrap_or(wide_slice.len());
        String::from_utf16(&wide_slice[..end])
            .map_err(|e| format!("Failed to decode string: {}", e))
    }
}

/// Check a DWORD registry value
pub fn check_dword(config: &CheckConfig) -> CheckResult {
    let path = match &config.registry_path {
        Some(p) => p,
        None => {
            return CheckResult::error(
                &config.id,
                &config.name,
                "Missing registry_path in config",
            )
        }
    };

    let key = match &config.registry_key {
        Some(k) => k,
        None => {
            return CheckResult::error(&config.id, &config.name, "Missing registry_key in config")
        }
    };

    let expected = config.expected_value.as_deref().unwrap_or("0");

    let (root, subkey) = match parse_root_key(path) {
        Some(v) => v,
        None => {
            return CheckResult::error(
                &config.id,
                &config.name,
                &format!("Invalid registry path: {}", path),
            )
        }
    };

    match read_dword(root, subkey, key) {
        Ok(value) => {
            let current = value.to_string();
            if current == expected {
                CheckResult::pass(&config.id, &config.name, &current, expected)
            } else {
                CheckResult::fail(&config.id, &config.name, &current, expected)
            }
        }
        Err(e) => CheckResult::error(&config.id, &config.name, &e),
    }
}

/// Check a string registry value
pub fn check_string(config: &CheckConfig) -> CheckResult {
    let path = match &config.registry_path {
        Some(p) => p,
        None => {
            return CheckResult::error(
                &config.id,
                &config.name,
                "Missing registry_path in config",
            )
        }
    };

    let key = match &config.registry_key {
        Some(k) => k,
        None => {
            return CheckResult::error(&config.id, &config.name, "Missing registry_key in config")
        }
    };

    let expected = config.expected_value.as_deref().unwrap_or("");

    let (root, subkey) = match parse_root_key(path) {
        Some(v) => v,
        None => {
            return CheckResult::error(
                &config.id,
                &config.name,
                &format!("Invalid registry path: {}", path),
            )
        }
    };

    match read_string(root, subkey, key) {
        Ok(value) => {
            if value == expected {
                CheckResult::pass(&config.id, &config.name, &value, expected)
            } else {
                CheckResult::fail(&config.id, &config.name, &value, expected)
            }
        }
        Err(e) => CheckResult::error(&config.id, &config.name, &e),
    }
}

/// Write a DWORD value to the registry
/// Returns Ok(()) on success, Err with message on failure
pub fn write_dword(path: &str, value_name: &str, data: u32) -> Result<(), String> {
    let (root, subkey) = match parse_root_key(path) {
        Some(v) => v,
        None => return Err(format!("Invalid registry path: {}", path)),
    };

    let subkey_wide = to_wide(subkey);
    let value_wide = to_wide(value_name);

    unsafe {
        let mut hkey = HKEY::default();

        let result = RegOpenKeyExW(
            root,
            PCWSTR::from_raw(subkey_wide.as_ptr()),
            0,
            KEY_WRITE,
            &mut hkey,
        );

        if result == ERROR_FILE_NOT_FOUND {
            return Err("Key not found".to_string());
        } else if result == ERROR_ACCESS_DENIED {
            return Err("Access denied - admin required".to_string());
        } else if result != ERROR_SUCCESS {
            return Err(format!("Failed to open key (error {})", result.0));
        }

        let data_bytes = data.to_le_bytes();
        let result = RegSetValueExW(
            hkey,
            PCWSTR::from_raw(value_wide.as_ptr()),
            0,
            REG_DWORD,
            Some(&data_bytes),
        );

        let _ = RegCloseKey(hkey);

        if result != ERROR_SUCCESS {
            return Err(format!("Failed to write value (error {})", result.0));
        }

        Ok(())
    }
}

/// Read a DWORD value from the registry using full path
/// This is a public wrapper for use by other modules
pub fn read_dword_value(path: &str, value_name: &str) -> Result<u32, String> {
    let (root, subkey) = match parse_root_key(path) {
        Some(v) => v,
        None => return Err(format!("Invalid registry path: {}", path)),
    };

    read_dword(root, subkey, value_name)
}

/// Write a string value to the registry
/// Returns Ok(()) on success, Err with message on failure
pub fn write_string(path: &str, value_name: &str, data: &str) -> Result<(), String> {
    let (root, subkey) = match parse_root_key(path) {
        Some(v) => v,
        None => return Err(format!("Invalid registry path: {}", path)),
    };

    let subkey_wide = to_wide(subkey);
    let value_wide = to_wide(value_name);
    let data_wide = to_wide(data);

    unsafe {
        let mut hkey = HKEY::default();

        let result = RegOpenKeyExW(
            root,
            PCWSTR::from_raw(subkey_wide.as_ptr()),
            0,
            KEY_WRITE,
            &mut hkey,
        );

        if result == ERROR_FILE_NOT_FOUND {
            return Err("Key not found".to_string());
        } else if result == ERROR_ACCESS_DENIED {
            return Err("Access denied - admin required".to_string());
        } else if result != ERROR_SUCCESS {
            return Err(format!("Failed to open key (error {})", result.0));
        }

        // Convert wide string to bytes (including null terminator)
        let data_bytes: &[u8] = std::slice::from_raw_parts(
            data_wide.as_ptr() as *const u8,
            data_wide.len() * 2,
        );

        let result = RegSetValueExW(
            hkey,
            PCWSTR::from_raw(value_wide.as_ptr()),
            0,
            REG_SZ,
            Some(data_bytes),
        );

        let _ = RegCloseKey(hkey);

        if result != ERROR_SUCCESS {
            return Err(format!("Failed to write value (error {})", result.0));
        }

        Ok(())
    }
}
