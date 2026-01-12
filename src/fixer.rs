//! Fix functionality for automatically resolving failing checks
//!
//! This module provides the ability to automatically fix certain types of
//! failing checks, including registry values, power plans, and processes.

use crate::checkers::{power_plan, processes, registry};
use crate::config::{CheckConfig, CheckType};

/// Result of a fix attempt
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct FixResult {
    pub check_id: String,
    pub check_name: String,
    pub success: bool,
    pub message: String,
}

/// Capability to fix a check
#[derive(Clone, Debug, PartialEq)]
pub enum FixCapability {
    /// Can fix without elevation
    Direct,
    /// Requires admin/UAC elevation
    RequiresAdmin,
    /// Cannot be automatically fixed
    Manual { reason: String },
}

impl Default for FixCapability {
    fn default() -> Self {
        FixCapability::Manual {
            reason: "Not supported".to_string(),
        }
    }
}

/// Determine the fix capability for a check config
pub fn get_fix_capability(config: &CheckConfig) -> FixCapability {
    match &config.check_type {
        CheckType::PowerScheme => FixCapability::Direct,
        CheckType::PowerMode => FixCapability::Direct,

        CheckType::RegistryDword | CheckType::RegistryString => {
            if let Some(path) = &config.registry_path {
                if registry::requires_admin(path) {
                    FixCapability::RequiresAdmin
                } else {
                    FixCapability::Direct
                }
            } else {
                FixCapability::Manual {
                    reason: "No registry path configured".to_string(),
                }
            }
        }

        CheckType::ProcessAbsent => FixCapability::Direct,

        CheckType::ProcessPresent => FixCapability::Manual {
            reason: "Cannot auto-start applications".to_string(),
        },

        CheckType::DisplayResolution | CheckType::DisplayRefreshRate | CheckType::HdrEnabled => {
            FixCapability::Manual {
                reason: "Display settings must be changed in Windows Settings".to_string(),
            }
        }
    }
}

/// Attempt to fix a single check
/// Returns FixResult with success/failure and message
pub fn fix_check(config: &CheckConfig) -> FixResult {
    let capability = get_fix_capability(config);

    match capability {
        FixCapability::Manual { reason } => FixResult {
            check_id: config.id.clone(),
            check_name: config.name.clone(),
            success: false,
            message: format!("Cannot auto-fix: {}", reason),
        },
        FixCapability::RequiresAdmin => {
            // For now, attempt the fix directly - it will fail with access denied
            // In the future, we could implement UAC elevation
            attempt_fix(config)
        }
        FixCapability::Direct => attempt_fix(config),
    }
}

/// Actually attempt to apply a fix
fn attempt_fix(config: &CheckConfig) -> FixResult {
    let result = match &config.check_type {
        CheckType::PowerScheme => fix_power_scheme(config),
        CheckType::PowerMode => fix_power_mode(config),
        CheckType::RegistryDword => fix_registry_dword(config),
        CheckType::RegistryString => fix_registry_string(config),
        CheckType::ProcessAbsent => fix_process_absent(config),
        CheckType::ProcessPresent => Err("Cannot auto-start applications".to_string()),
        CheckType::DisplayResolution | CheckType::DisplayRefreshRate | CheckType::HdrEnabled => {
            Err("Display settings cannot be auto-fixed".to_string())
        }
    };

    match result {
        Ok(msg) => FixResult {
            check_id: config.id.clone(),
            check_name: config.name.clone(),
            success: true,
            message: msg,
        },
        Err(msg) => FixResult {
            check_id: config.id.clone(),
            check_name: config.name.clone(),
            success: false,
            message: msg,
        },
    }
}

/// Fix a power scheme check by setting the expected power plan
fn fix_power_scheme(config: &CheckConfig) -> Result<String, String> {
    let expected = config.expected_value.as_deref().unwrap_or("high_performance");
    power_plan::set_power_scheme(expected)?;
    Ok(format!("Set power plan to {}", expected))
}

/// Fix a power mode check by setting the expected power mode
fn fix_power_mode(config: &CheckConfig) -> Result<String, String> {
    let expected = config.expected_value.as_deref().unwrap_or("best_performance");
    power_plan::set_power_mode(expected)?;
    Ok(format!("Set power mode to {}", expected))
}

/// Fix a registry DWORD check by setting the expected value
fn fix_registry_dword(config: &CheckConfig) -> Result<String, String> {
    let path = config
        .registry_path
        .as_ref()
        .ok_or("No registry path configured")?;
    let key = config
        .registry_key
        .as_ref()
        .ok_or("No registry key configured")?;
    let expected_str = config.expected_value.as_deref().unwrap_or("0");
    let expected: u32 = expected_str
        .parse()
        .map_err(|_| format!("Invalid DWORD value: {}", expected_str))?;

    registry::write_dword(path, key, expected)?;
    Ok(format!("Set {} to {}", key, expected))
}

/// Fix a registry string check by setting the expected value
fn fix_registry_string(config: &CheckConfig) -> Result<String, String> {
    let path = config
        .registry_path
        .as_ref()
        .ok_or("No registry path configured")?;
    let key = config
        .registry_key
        .as_ref()
        .ok_or("No registry key configured")?;
    let expected = config.expected_value.as_deref().unwrap_or("");

    registry::write_string(path, key, expected)?;
    Ok(format!("Set {} to '{}'", key, expected))
}

/// Fix a process absent check by terminating the process
fn fix_process_absent(config: &CheckConfig) -> Result<String, String> {
    let process_name = config
        .process_name
        .as_ref()
        .ok_or("No process name configured")?;

    let count = processes::terminate_process(process_name)?;
    if count > 0 {
        Ok(format!("Terminated {} instance(s) of {}", count, process_name))
    } else {
        Ok(format!("{} is not running", process_name))
    }
}

/// Fix all failing checks in a list
/// Returns a summary of results
pub fn fix_all(configs: &[CheckConfig], failing_ids: &[String]) -> Vec<FixResult> {
    let mut results = Vec::new();

    for config in configs {
        if failing_ids.contains(&config.id) && config.enabled {
            let result = fix_check(config);
            results.push(result);
        }
    }

    results
}

/// Check if any fixes in a list require admin privileges
#[allow(dead_code)]
pub fn any_require_admin(configs: &[CheckConfig], failing_ids: &[String]) -> bool {
    configs.iter().any(|config| {
        failing_ids.contains(&config.id)
            && config.enabled
            && get_fix_capability(config) == FixCapability::RequiresAdmin
    })
}

/// Get counts of fixable checks by type
pub fn get_fix_counts(configs: &[CheckConfig], failing_ids: &[String]) -> (usize, usize, usize) {
    let mut direct = 0;
    let mut admin = 0;
    let mut manual = 0;

    for config in configs {
        if failing_ids.contains(&config.id) && config.enabled {
            match get_fix_capability(config) {
                FixCapability::Direct => direct += 1,
                FixCapability::RequiresAdmin => admin += 1,
                FixCapability::Manual { .. } => manual += 1,
            }
        }
    }

    (direct, admin, manual)
}
