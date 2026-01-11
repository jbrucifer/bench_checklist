use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Configuration root - supports both v1 (flat) and v2 (nested scenarios) formats
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigRoot {
    V2(ConfigV2),
    V1(ConfigV1),
}

/// Legacy flat configuration (v1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigV1 {
    pub poll_interval_seconds: u64,
    pub notify_on_drift: bool,
    pub checks: Vec<CheckConfig>,
}

/// New scenario-based configuration (v2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigV2 {
    pub version: u32,
    pub default_scenario: String,
    pub scenarios: HashMap<String, Scenario>,
}

/// Individual scenario configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub description: String,
    pub poll_interval_seconds: u64,
    pub notify_on_drift: bool,
    pub checks: Vec<CheckConfig>,
}

/// Working configuration (what the application uses internally)
#[derive(Debug, Clone)]
pub struct Config {
    pub root: ConfigV2,
    pub active_scenario: String,
}

/// Individual check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckConfig {
    pub id: String,
    pub name: String,
    pub check_type: CheckType,
    #[serde(default)]
    pub enabled: bool,

    // Registry-specific fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry_key: Option<String>,

    // Process-specific fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_name: Option<String>,

    // Expected value (interpretation depends on check_type)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_value: Option<String>,
}

/// Types of checks supported
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckType {
    PowerScheme,
    PowerMode,
    RegistryDword,
    RegistryString,
    ProcessAbsent,
    ProcessPresent,
}

/// Helper functions to create default scenarios
fn create_gaming_scenario() -> Scenario {
    Scenario {
        name: "Gaming Benchmarks".to_string(),
        description: "Optimized for gaming performance testing".to_string(),
        poll_interval_seconds: 5,
        notify_on_drift: true,
        checks: vec![
            CheckConfig {
                id: "power_plan".to_string(),
                name: "Power Plan (High Performance)".to_string(),
                check_type: CheckType::PowerScheme,
                enabled: true,
                expected_value: Some("high_performance".to_string()),
                registry_path: None,
                registry_key: None,
                process_name: None,
            },
            CheckConfig {
                id: "power_mode".to_string(),
                name: "Power Mode (Best Performance)".to_string(),
                check_type: CheckType::PowerMode,
                enabled: true,
                expected_value: Some("best_performance".to_string()),
                registry_path: None,
                registry_key: None,
                process_name: None,
            },
            CheckConfig {
                id: "game_mode".to_string(),
                name: "Game Mode Enabled".to_string(),
                check_type: CheckType::RegistryDword,
                enabled: true,
                expected_value: Some("1".to_string()),
                registry_path: Some("HKCU\\Software\\Microsoft\\GameBar".to_string()),
                registry_key: Some("AutoGameModeEnabled".to_string()),
                process_name: None,
            },
            CheckConfig {
                id: "hardware_gpu_scheduling".to_string(),
                name: "Hardware GPU Scheduling".to_string(),
                check_type: CheckType::RegistryDword,
                enabled: true,
                expected_value: Some("2".to_string()),
                registry_path: Some("HKLM\\SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers".to_string()),
                registry_key: Some("HwSchMode".to_string()),
                process_name: None,
            },
            CheckConfig {
                id: "no_discord".to_string(),
                name: "Discord Not Running".to_string(),
                check_type: CheckType::ProcessAbsent,
                enabled: true,
                process_name: Some("Discord.exe".to_string()),
                expected_value: None,
                registry_path: None,
                registry_key: None,
            },
            CheckConfig {
                id: "no_chrome".to_string(),
                name: "Chrome Not Running".to_string(),
                check_type: CheckType::ProcessAbsent,
                enabled: true,
                process_name: Some("chrome.exe".to_string()),
                expected_value: None,
                registry_path: None,
                registry_key: None,
            },
        ],
    }
}

fn create_cpu_scenario() -> Scenario {
    Scenario {
        name: "CPU Benchmarks".to_string(),
        description: "Focused on CPU-intensive workloads".to_string(),
        poll_interval_seconds: 10,
        notify_on_drift: true,
        checks: vec![
            CheckConfig {
                id: "power_plan".to_string(),
                name: "Power Plan (High Performance)".to_string(),
                check_type: CheckType::PowerScheme,
                enabled: true,
                expected_value: Some("high_performance".to_string()),
                registry_path: None,
                registry_key: None,
                process_name: None,
            },
            CheckConfig {
                id: "power_mode".to_string(),
                name: "Power Mode (Best Performance)".to_string(),
                check_type: CheckType::PowerMode,
                enabled: true,
                expected_value: Some("best_performance".to_string()),
                registry_path: None,
                registry_key: None,
                process_name: None,
            },
            CheckConfig {
                id: "background_apps".to_string(),
                name: "Background Apps Disabled".to_string(),
                check_type: CheckType::RegistryDword,
                enabled: true,
                expected_value: Some("1".to_string()),
                registry_path: Some("HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\BackgroundAccessApplications".to_string()),
                registry_key: Some("GlobalUserDisabled".to_string()),
                process_name: None,
            },
            CheckConfig {
                id: "no_chrome".to_string(),
                name: "Chrome Not Running".to_string(),
                check_type: CheckType::ProcessAbsent,
                enabled: true,
                process_name: Some("chrome.exe".to_string()),
                expected_value: None,
                registry_path: None,
                registry_key: None,
            },
        ],
    }
}

fn create_gpu_scenario() -> Scenario {
    Scenario {
        name: "GPU Benchmarks".to_string(),
        description: "Optimized for GPU testing".to_string(),
        poll_interval_seconds: 5,
        notify_on_drift: true,
        checks: vec![
            CheckConfig {
                id: "power_plan".to_string(),
                name: "Power Plan (High Performance)".to_string(),
                check_type: CheckType::PowerScheme,
                enabled: true,
                expected_value: Some("high_performance".to_string()),
                registry_path: None,
                registry_key: None,
                process_name: None,
            },
            CheckConfig {
                id: "power_mode".to_string(),
                name: "Power Mode (Best Performance)".to_string(),
                check_type: CheckType::PowerMode,
                enabled: true,
                expected_value: Some("best_performance".to_string()),
                registry_path: None,
                registry_key: None,
                process_name: None,
            },
            CheckConfig {
                id: "hardware_gpu_scheduling".to_string(),
                name: "Hardware GPU Scheduling".to_string(),
                check_type: CheckType::RegistryDword,
                enabled: true,
                expected_value: Some("2".to_string()),
                registry_path: Some("HKLM\\SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers".to_string()),
                registry_key: Some("HwSchMode".to_string()),
                process_name: None,
            },
            CheckConfig {
                id: "visual_effects".to_string(),
                name: "Visual Effects (Best Performance)".to_string(),
                check_type: CheckType::RegistryDword,
                enabled: true,
                expected_value: Some("2".to_string()),
                registry_path: Some("HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\VisualEffects".to_string()),
                registry_key: Some("VisualFXSetting".to_string()),
                process_name: None,
            },
        ],
    }
}

fn create_productivity_scenario() -> Scenario {
    Scenario {
        name: "Productivity Workloads".to_string(),
        description: "For office and productivity testing".to_string(),
        poll_interval_seconds: 15,
        notify_on_drift: false,
        checks: vec![
            CheckConfig {
                id: "power_plan".to_string(),
                name: "Power Plan (Balanced)".to_string(),
                check_type: CheckType::PowerScheme,
                enabled: true,
                expected_value: Some("balanced".to_string()),
                registry_path: None,
                registry_key: None,
                process_name: None,
            },
        ],
    }
}

/// Migrate v1 config to v2 format
fn migrate_v1_to_v2(v1: ConfigV1) -> ConfigV2 {
    let scenario = Scenario {
        name: "Default".to_string(),
        description: "Migrated from legacy config".to_string(),
        poll_interval_seconds: v1.poll_interval_seconds,
        notify_on_drift: v1.notify_on_drift,
        checks: v1.checks,
    };

    let mut scenarios = HashMap::new();
    scenarios.insert("default".to_string(), scenario);

    ConfigV2 {
        version: 2,
        default_scenario: "default".to_string(),
        scenarios,
    }
}

impl Default for Config {
    fn default() -> Self {
        // Create all 4 default scenarios
        let mut scenarios = HashMap::new();

        scenarios.insert("gaming".to_string(), create_gaming_scenario());
        scenarios.insert("cpu_benchmark".to_string(), create_cpu_scenario());
        scenarios.insert("gpu_benchmark".to_string(), create_gpu_scenario());
        scenarios.insert("productivity".to_string(), create_productivity_scenario());

        let root = ConfigV2 {
            version: 2,
            default_scenario: "gaming".to_string(),
            scenarios,
        };

        Self {
            root,
            active_scenario: "gaming".to_string(),
        }
    }
}

#[allow(dead_code)]
impl Config {
    /// Get the default config file path
    pub fn default_path() -> PathBuf {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        exe_dir.join("config").join("checklist.json")
    }

    /// Load configuration from file (handles both v1 and v2 formats)
    pub fn load(path: &PathBuf) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {:?}", path))?;

        let root: ConfigRoot = serde_json::from_str(&content)
            .with_context(|| "Failed to parse config JSON")?;

        let config_v2 = match root {
            ConfigRoot::V1(v1) => {
                tracing::info!("Migrating v1 config to v2 format");
                migrate_v1_to_v2(v1)
            }
            ConfigRoot::V2(v2) => v2,
        };

        let active_scenario = config_v2.default_scenario.clone();

        // Validate active scenario exists
        if !config_v2.scenarios.contains_key(&active_scenario) {
            return Err(anyhow!(
                "Default scenario '{}' not found in config",
                active_scenario
            ));
        }

        Ok(Config {
            root: config_v2,
            active_scenario,
        })
    }

    /// Load from default path, or create default if not exists
    pub fn load_or_default() -> Result<Self> {
        let path = Self::default_path();

        if path.exists() {
            Self::load(&path)
        } else {
            let config = Config::default();
            config.save(&path)?;
            Ok(config)
        }
    }

    /// Save configuration to file (always saves as v2, creates backup)
    pub fn save(&self, path: &PathBuf) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {:?}", parent))?;
        }

        // Create backup if file exists
        if path.exists() {
            let backup_path = path.with_extension("json.backup");
            fs::copy(path, &backup_path)
                .with_context(|| format!("Failed to create backup at {:?}", backup_path))?;
            tracing::info!("Created config backup at {:?}", backup_path);
        }

        // Update default_scenario to persist active scenario
        let mut root = self.root.clone();
        root.default_scenario = self.active_scenario.clone();

        let content = serde_json::to_string_pretty(&root)
            .with_context(|| "Failed to serialize config")?;

        fs::write(path, content)
            .with_context(|| format!("Failed to write config file: {:?}", path))?;

        Ok(())
    }

    /// Get active scenario data
    pub fn get_active_scenario_data(&self) -> Result<&Scenario> {
        self.root
            .scenarios
            .get(&self.active_scenario)
            .ok_or_else(|| anyhow!("Active scenario '{}' not found", self.active_scenario))
    }

    /// Get checks from active scenario
    pub fn get_scenario_checks(&self) -> Result<&Vec<CheckConfig>> {
        Ok(&self.get_active_scenario_data()?.checks)
    }

    /// Get list of scenario IDs
    pub fn get_scenario_ids(&self) -> Vec<String> {
        self.root.scenarios.keys().cloned().collect()
    }

    /// Get only enabled checks from active scenario
    pub fn enabled_checks(&self) -> Vec<&CheckConfig> {
        if let Ok(checks) = self.get_scenario_checks() {
            checks.iter().filter(|c| c.enabled).collect()
        } else {
            vec![]
        }
    }

    /// Get poll interval from active scenario
    pub fn get_poll_interval(&self) -> u64 {
        self.get_active_scenario_data()
            .map(|s| s.poll_interval_seconds)
            .unwrap_or(10)
    }

    /// Get notify_on_drift from active scenario
    pub fn get_notify_on_drift(&self) -> bool {
        self.get_active_scenario_data()
            .map(|s| s.notify_on_drift)
            .unwrap_or(true)
    }
}
