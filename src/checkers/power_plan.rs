use crate::checkers::CheckResult;
use crate::config::CheckConfig;
use windows::core::GUID;
use windows::Win32::System::Power::{PowerGetActiveScheme, PowerSetActiveScheme};

// Link to kernel32 for LocalFree
#[link(name = "kernel32")]
extern "system" {
    fn LocalFree(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
}

// Power overlay scheme functions - loaded dynamically since they're not in all SDK versions
use std::sync::OnceLock;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::core::PCWSTR;

type PowerGetActualOverlaySchemeFn = unsafe extern "system" fn(*mut GUID) -> u32;
type PowerSetActiveOverlaySchemeFn = unsafe extern "system" fn(*const GUID) -> u32;

static POWER_OVERLAY_FUNCS: OnceLock<Option<(PowerGetActualOverlaySchemeFn, PowerSetActiveOverlaySchemeFn)>> = OnceLock::new();

fn get_overlay_funcs() -> Option<(PowerGetActualOverlaySchemeFn, PowerSetActiveOverlaySchemeFn)> {
    *POWER_OVERLAY_FUNCS.get_or_init(|| {
        unsafe {
            let lib_name: Vec<u16> = "powrprof.dll\0".encode_utf16().collect();
            let lib = LoadLibraryW(PCWSTR::from_raw(lib_name.as_ptr())).ok()?;
            if lib == HMODULE::default() {
                return None;
            }

            let get_fn = GetProcAddress(lib, windows::core::s!("PowerGetActualOverlayScheme"))?;
            let set_fn = GetProcAddress(lib, windows::core::s!("PowerSetActiveOverlayScheme"))?;

            Some((
                std::mem::transmute::<_, PowerGetActualOverlaySchemeFn>(get_fn),
                std::mem::transmute::<_, PowerSetActiveOverlaySchemeFn>(set_fn),
            ))
        }
    })
}

// Well-known power scheme GUIDs
const GUID_HIGH_PERFORMANCE: GUID = GUID::from_u128(0x8c5e7fda_e8bf_4a96_9a85_a6e23a8c635c);
const GUID_BALANCED: GUID = GUID::from_u128(0x381b4222_f694_41f0_9685_ff5bb260df2e);
const GUID_POWER_SAVER: GUID = GUID::from_u128(0xa1841308_3541_4fab_bc81_f71556f20b4a);
// Ultimate Performance (may not exist on all systems)
const GUID_ULTIMATE_PERFORMANCE: GUID = GUID::from_u128(0xe9a42b02_d5df_448d_aa00_03f14749eb61);

/// Get human-readable name for a power scheme GUID
fn scheme_name(guid: &GUID) -> &'static str {
    if *guid == GUID_HIGH_PERFORMANCE {
        "High Performance"
    } else if *guid == GUID_BALANCED {
        "Balanced"
    } else if *guid == GUID_POWER_SAVER {
        "Power Saver"
    } else if *guid == GUID_ULTIMATE_PERFORMANCE {
        "Ultimate Performance"
    } else {
        "Custom/Unknown"
    }
}

/// Get the scheme key for comparison
fn scheme_key(guid: &GUID) -> &'static str {
    if *guid == GUID_HIGH_PERFORMANCE {
        "high_performance"
    } else if *guid == GUID_BALANCED {
        "balanced"
    } else if *guid == GUID_POWER_SAVER {
        "power_saver"
    } else if *guid == GUID_ULTIMATE_PERFORMANCE {
        "ultimate_performance"
    } else {
        "custom"
    }
}

/// Parse expected value to check against
fn parse_expected(expected: &str) -> Vec<&'static str> {
    match expected.to_lowercase().as_str() {
        "high_performance" | "high" => vec!["high_performance", "ultimate_performance"],
        "ultimate_performance" | "ultimate" => vec!["ultimate_performance"],
        "balanced" => vec!["balanced"],
        "power_saver" | "saver" => vec!["power_saver"],
        _ => vec!["custom"],
    }
}

/// Check the current power plan against expected
pub fn check(config: &CheckConfig) -> CheckResult {
    let expected = config.expected_value.as_deref().unwrap_or("high_performance");

    unsafe {
        let mut scheme_guid: *mut GUID = std::ptr::null_mut();

        let result = PowerGetActiveScheme(None, &mut scheme_guid);

        if result.is_err() {
            return CheckResult::error(
                &config.id,
                &config.name,
                &format!("Failed to get active power scheme: {:?}", result),
            );
        }

        if scheme_guid.is_null() {
            return CheckResult::error(
                &config.id,
                &config.name,
                "PowerGetActiveScheme returned null",
            );
        }

        let current_guid = *scheme_guid;
        let current_key = scheme_key(&current_guid);
        let current_name = scheme_name(&current_guid);

        // Free the allocated GUID - Windows allocated this memory
        LocalFree(scheme_guid as *mut _);

        let acceptable = parse_expected(expected);

        if acceptable.contains(&current_key) {
            CheckResult::pass(&config.id, &config.name, current_name, expected)
        } else {
            CheckResult::fail(&config.id, &config.name, current_name, expected)
        }
    }
}

/// Set the active power scheme by key name
/// Returns Ok(()) on success, Err with message on failure
pub fn set_power_scheme(scheme_key: &str) -> Result<(), String> {
    use windows::Win32::Foundation::ERROR_SUCCESS;

    let guid = match scheme_key.to_lowercase().as_str() {
        "high_performance" | "high" => GUID_HIGH_PERFORMANCE,
        "ultimate_performance" | "ultimate" => GUID_ULTIMATE_PERFORMANCE,
        "balanced" => GUID_BALANCED,
        "power_saver" | "saver" => GUID_POWER_SAVER,
        _ => return Err(format!("Unknown power scheme: {}", scheme_key)),
    };

    unsafe {
        let result = PowerSetActiveScheme(None, Some(&guid));
        if result == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("Failed to set power scheme (error {})", result.0))
        }
    }
}

// ===== Power Mode (Overlay Scheme) Support =====
// Power Mode is the slider in Windows 10/11 Settings > Power & battery
// It's separate from Power Plans and controls performance overlay

// Power mode overlay GUIDs (Windows 10 1709+)
const GUID_POWER_MODE_BETTER_BATTERY: GUID = GUID::from_u128(0x961cc777_2547_4f9d_8174_7d86181b8a7a);
const GUID_POWER_MODE_BALANCED: GUID = GUID::from_u128(0x00000000_0000_0000_0000_000000000000); // All zeros = balanced/default
const GUID_POWER_MODE_BETTER_PERFORMANCE: GUID = GUID::from_u128(0x3af9B8d9_7c97_431d_ad78_34a8bfea439f);
const GUID_POWER_MODE_BEST_PERFORMANCE: GUID = GUID::from_u128(0xded574b5_45a0_4f42_8737_46345c09c238);

/// Get human-readable name for a power mode GUID
fn power_mode_name(guid: &GUID) -> &'static str {
    if *guid == GUID_POWER_MODE_BEST_PERFORMANCE {
        "Best Performance"
    } else if *guid == GUID_POWER_MODE_BETTER_PERFORMANCE {
        "Better Performance"
    } else if *guid == GUID_POWER_MODE_BALANCED || guid.to_u128() == 0 {
        "Balanced"
    } else if *guid == GUID_POWER_MODE_BETTER_BATTERY {
        "Better Battery"
    } else {
        "Unknown"
    }
}

/// Get the power mode key for comparison
fn power_mode_key(guid: &GUID) -> &'static str {
    if *guid == GUID_POWER_MODE_BEST_PERFORMANCE {
        "best_performance"
    } else if *guid == GUID_POWER_MODE_BETTER_PERFORMANCE {
        "better_performance"
    } else if *guid == GUID_POWER_MODE_BALANCED || guid.to_u128() == 0 {
        "balanced"
    } else if *guid == GUID_POWER_MODE_BETTER_BATTERY {
        "better_battery"
    } else {
        "unknown"
    }
}

/// Parse expected power mode value
fn parse_expected_mode(expected: &str) -> Vec<&'static str> {
    match expected.to_lowercase().as_str() {
        "best_performance" | "best" | "max" => vec!["best_performance"],
        "better_performance" | "better" | "high" => vec!["better_performance", "best_performance"],
        "balanced" | "default" => vec!["balanced"],
        "better_battery" | "battery" | "saver" => vec!["better_battery"],
        _ => vec!["unknown"],
    }
}

/// Check the current power mode (overlay scheme) against expected
pub fn check_power_mode(config: &CheckConfig) -> CheckResult {
    let expected = config.expected_value.as_deref().unwrap_or("best_performance");

    let Some((get_fn, _)) = get_overlay_funcs() else {
        return CheckResult::error(
            &config.id,
            &config.name,
            "Power mode API not available on this Windows version",
        );
    };

    unsafe {
        let mut mode_guid: GUID = GUID::from_u128(0);

        let result = get_fn(&mut mode_guid);

        if result != 0 {
            return CheckResult::error(
                &config.id,
                &config.name,
                &format!("Failed to get power mode: error {}", result),
            );
        }

        let current_key = power_mode_key(&mode_guid);
        let current_name = power_mode_name(&mode_guid);

        let acceptable = parse_expected_mode(expected);

        if acceptable.contains(&current_key) {
            CheckResult::pass(&config.id, &config.name, current_name, expected)
        } else {
            CheckResult::fail(&config.id, &config.name, current_name, expected)
        }
    }
}

/// Set the active power mode (overlay scheme) by key name
/// Returns Ok(()) on success, Err with message on failure
pub fn set_power_mode(mode_key: &str) -> Result<(), String> {
    let Some((_, set_fn)) = get_overlay_funcs() else {
        return Err("Power mode API not available on this Windows version".to_string());
    };

    let guid = match mode_key.to_lowercase().as_str() {
        "best_performance" | "best" | "max" => GUID_POWER_MODE_BEST_PERFORMANCE,
        "better_performance" | "better" | "high" => GUID_POWER_MODE_BETTER_PERFORMANCE,
        "balanced" | "default" => GUID_POWER_MODE_BALANCED,
        "better_battery" | "battery" | "saver" => GUID_POWER_MODE_BETTER_BATTERY,
        _ => return Err(format!("Unknown power mode: {}", mode_key)),
    };

    unsafe {
        let result = set_fn(&guid);
        if result == 0 {
            Ok(())
        } else {
            Err(format!("Failed to set power mode (error {})", result))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheme_name() {
        assert_eq!(scheme_name(&GUID_HIGH_PERFORMANCE), "High Performance");
        assert_eq!(scheme_name(&GUID_BALANCED), "Balanced");
    }
}
