pub mod display;
pub mod power_plan;
pub mod processes;
pub mod registry;

use crate::config::{CheckConfig, CheckType};
use thiserror::Error;

/// Result of a single check
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CheckResult {
    pub id: String,
    pub name: String,
    pub passed: bool,
    pub current_value: String,
    pub expected_value: String,
    pub message: String,
}

impl CheckResult {
    pub fn pass(id: &str, name: &str, current: &str, expected: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            passed: true,
            current_value: current.to_string(),
            expected_value: expected.to_string(),
            message: format!("{} is correctly set", name),
        }
    }

    pub fn fail(id: &str, name: &str, current: &str, expected: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            passed: false,
            current_value: current.to_string(),
            expected_value: expected.to_string(),
            message: format!("{}: expected '{}', got '{}'", name, expected, current),
        }
    }

    pub fn error(id: &str, name: &str, error: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            passed: false,
            current_value: "ERROR".to_string(),
            expected_value: String::new(),
            message: format!("{}: {}", name, error),
        }
    }
}

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum CheckError {
    #[error("Windows API error: {0}")]
    WindowsApi(String),

    #[error("Registry error: {0}")]
    Registry(String),

    #[error("Configuration error: {0}")]
    Config(String),
}

/// Run a check based on its configuration
pub fn run_check(config: &CheckConfig) -> CheckResult {
    match config.check_type {
        CheckType::PowerScheme => power_plan::check(config),
        CheckType::PowerMode => power_plan::check_power_mode(config),
        CheckType::RegistryDword => registry::check_dword(config),
        CheckType::RegistryString => registry::check_string(config),
        CheckType::ProcessAbsent => processes::check_absent(config),
        CheckType::ProcessPresent => processes::check_present(config),
        CheckType::DisplayResolution => display::check_resolution(config),
        CheckType::DisplayRefreshRate => display::check_refresh_rate(config),
        CheckType::HdrEnabled => display::check_hdr(config),
    }
}

/// Run all enabled checks and return results
pub fn run_all_checks(checks: &[CheckConfig]) -> Vec<CheckResult> {
    checks
        .iter()
        .filter(|c| c.enabled)
        .map(run_check)
        .collect()
}

/// Overall status derived from check results
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverallStatus {
    AllPassed,
    SomeFailed,
    AllFailed,
}

impl OverallStatus {
    pub fn from_results(results: &[CheckResult]) -> Self {
        if results.is_empty() {
            return Self::AllPassed;
        }

        let passed = results.iter().filter(|r| r.passed).count();
        let total = results.len();

        if passed == total {
            Self::AllPassed
        } else if passed == 0 {
            Self::AllFailed
        } else {
            Self::SomeFailed
        }
    }
}
