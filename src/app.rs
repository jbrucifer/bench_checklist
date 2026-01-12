use crate::checkers::{run_all_checks, CheckResult, OverallStatus};
use crate::config::Config;
use crate::notifications;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Mutex<AppStateInner>>,
    /// Signal for windows to close when app is exiting
    should_exit: Arc<AtomicBool>,
}

struct AppStateInner {
    pub config: Config,
    pub config_path: PathBuf,
    pub last_results: Vec<CheckResult>,
    pub last_check_time: Option<Instant>,
    pub previous_status: HashMap<String, bool>,
    pub notify_on_drift: bool,
}

impl AppState {
    pub fn new(config: Config, config_path: PathBuf) -> Self {
        let notify_on_drift = config.get_notify_on_drift();
        Self {
            inner: Arc::new(Mutex::new(AppStateInner {
                config,
                config_path,
                last_results: Vec::new(),
                last_check_time: None,
                previous_status: HashMap::new(),
                notify_on_drift,
            })),
            should_exit: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Signal that the app should exit (closes any open windows)
    pub fn signal_exit(&self) {
        self.should_exit.store(true, Ordering::SeqCst);
    }

    /// Check if the app should exit
    pub fn should_exit(&self) -> bool {
        self.should_exit.load(Ordering::SeqCst)
    }

    /// Run all checks and update state
    pub fn run_checks(&self) -> (Vec<CheckResult>, OverallStatus) {
        let mut inner = self.inner.lock().unwrap();

        let checks = inner.config.get_scenario_checks()
            .map(|c| c.clone())
            .unwrap_or_default();

        let results = run_all_checks(&checks);
        let status = OverallStatus::from_results(&results);

        // Detect drift (settings that changed from passing to failing)
        let mut drifted: Vec<&CheckResult> = Vec::new();

        for result in &results {
            let was_passing = inner.previous_status.get(&result.id).copied().unwrap_or(true);

            if was_passing && !result.passed {
                drifted.push(result);
            }

            inner.previous_status.insert(result.id.clone(), result.passed);
        }

        // Notify on drift if enabled
        tracing::debug!("Drift detection: notify_on_drift={}, drifted_count={}", inner.notify_on_drift, drifted.len());
        if inner.notify_on_drift && !drifted.is_empty() {
            tracing::info!("Notifying about {} drifted checks", drifted.len());
            notifications::notify_drift(&drifted);
        }

        inner.last_results = results.clone();
        inner.last_check_time = Some(Instant::now());

        (results, status)
    }

    /// Get the last check results
    pub fn get_last_results(&self) -> Vec<CheckResult> {
        self.inner.lock().unwrap().last_results.clone()
    }

    /// Get the current overall status
    pub fn get_status(&self) -> OverallStatus {
        let inner = self.inner.lock().unwrap();
        OverallStatus::from_results(&inner.last_results)
    }

    /// Get poll interval in seconds from active scenario
    pub fn get_poll_interval(&self) -> u64 {
        self.inner.lock().unwrap().config.get_poll_interval()
    }

    /// Update poll interval for active scenario
    pub fn set_poll_interval(&self, seconds: u64) {
        let mut inner = self.inner.lock().unwrap();
        let active_id = inner.config.active_scenario.clone();
        if let Some(scenario) = inner.config.root.scenarios.get_mut(&active_id) {
            scenario.poll_interval_seconds = seconds;
        }
    }

    /// Toggle a check's enabled state in active scenario
    pub fn toggle_check(&self, id: &str) {
        let mut inner = self.inner.lock().unwrap();
        let active_id = inner.config.active_scenario.clone();
        if let Some(scenario) = inner.config.root.scenarios.get_mut(&active_id) {
            if let Some(check) = scenario.checks.iter_mut().find(|c| c.id == id) {
                check.enabled = !check.enabled;
            }
        }
    }

    /// Set notify on drift for active scenario
    pub fn set_notify_on_drift(&self, enabled: bool) {
        let mut inner = self.inner.lock().unwrap();
        inner.notify_on_drift = enabled;
        let active_id = inner.config.active_scenario.clone();
        if let Some(scenario) = inner.config.root.scenarios.get_mut(&active_id) {
            scenario.notify_on_drift = enabled;
        }
    }

    /// Get notify on drift setting
    pub fn get_notify_on_drift(&self) -> bool {
        self.inner.lock().unwrap().notify_on_drift
    }

    /// Get a copy of the config
    pub fn get_config(&self) -> Config {
        self.inner.lock().unwrap().config.clone()
    }

    /// Save config to file
    pub fn save_config(&self) -> anyhow::Result<()> {
        let inner = self.inner.lock().unwrap();
        inner.config.save(&inner.config_path)
    }

    /// Reload config from file
    pub fn reload_config(&self) -> anyhow::Result<()> {
        let mut inner = self.inner.lock().unwrap();
        let config = Config::load(&inner.config_path)?;
        inner.notify_on_drift = config.get_notify_on_drift();
        inner.config = config;
        Ok(())
    }

    /// Get list of available scenarios (id, name, description)
    pub fn get_scenarios(&self) -> Vec<(String, String, String)> {
        let inner = self.inner.lock().unwrap();
        let mut scenarios: Vec<(String, String, String)> = inner
            .config
            .root
            .scenarios
            .iter()
            .map(|(id, scenario)| {
                (
                    id.clone(),
                    scenario.name.clone(),
                    scenario.description.clone(),
                )
            })
            .collect();

        // Sort by ID for consistent ordering
        scenarios.sort_by(|a, b| a.0.cmp(&b.0));
        scenarios
    }

    /// Get current active scenario ID
    pub fn get_active_scenario(&self) -> String {
        self.inner.lock().unwrap().config.active_scenario.clone()
    }

    /// Get current active scenario name
    pub fn get_active_scenario_name(&self) -> String {
        let inner = self.inner.lock().unwrap();
        inner
            .config
            .root
            .scenarios
            .get(&inner.config.active_scenario)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    /// Switch to a different scenario
    pub fn set_active_scenario(&self, scenario_id: &str) -> anyhow::Result<()> {
        let mut inner = self.inner.lock().unwrap();

        // Validate scenario exists
        if !inner.config.root.scenarios.contains_key(scenario_id) {
            return Err(anyhow::anyhow!("Scenario '{}' not found", scenario_id));
        }

        // Update active scenario
        inner.config.active_scenario = scenario_id.to_string();

        // Reset drift detection (clear previous status)
        inner.previous_status.clear();

        // Update notify_on_drift from new scenario
        inner.notify_on_drift = inner
            .config
            .root
            .scenarios
            .get(scenario_id)
            .map(|s| s.notify_on_drift)
            .unwrap_or(true);

        tracing::info!("Switched to scenario: {}", scenario_id);

        Ok(())
    }

    /// Add a new check to the current scenario
    pub fn add_check(&self, check: crate::config::CheckConfig) {
        let mut inner = self.inner.lock().unwrap();
        let active_id = inner.config.active_scenario.clone();
        if let Some(scenario) = inner.config.root.scenarios.get_mut(&active_id) {
            scenario.checks.push(check);
        }
    }

    /// Remove a check from the current scenario by ID
    pub fn remove_check(&self, check_id: &str) {
        let mut inner = self.inner.lock().unwrap();
        let active_id = inner.config.active_scenario.clone();
        if let Some(scenario) = inner.config.root.scenarios.get_mut(&active_id) {
            scenario.checks.retain(|c| c.id != check_id);
        }
    }

    /// Update an existing check in the current scenario
    pub fn update_check(&self, check: crate::config::CheckConfig) {
        let mut inner = self.inner.lock().unwrap();
        let active_id = inner.config.active_scenario.clone();
        if let Some(scenario) = inner.config.root.scenarios.get_mut(&active_id) {
            if let Some(existing) = scenario.checks.iter_mut().find(|c| c.id == check.id) {
                *existing = check;
            }
        }
    }

    /// Add a new scenario to the config
    pub fn add_scenario(&self, id: &str, scenario: crate::config::Scenario) -> anyhow::Result<()> {
        let mut inner = self.inner.lock().unwrap();

        if inner.config.root.scenarios.contains_key(id) {
            return Err(anyhow::anyhow!("Scenario '{}' already exists", id));
        }

        inner.config.root.scenarios.insert(id.to_string(), scenario);
        tracing::info!("Added new scenario: {}", id);
        Ok(())
    }

    /// Generate tooltip text for tray icon
    pub fn get_tooltip(&self) -> String {
        let inner = self.inner.lock().unwrap();
        let results = &inner.last_results;

        // Get active scenario name
        let scenario_name = inner
            .config
            .root
            .scenarios
            .get(&inner.config.active_scenario)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        if results.is_empty() {
            return format!("Bench Checklist\n{}\nNo checks run yet", scenario_name);
        }

        let passed = results.iter().filter(|r| r.passed).count();
        let total = results.len();
        let status = OverallStatus::from_results(results);

        let status_text = match status {
            OverallStatus::AllPassed => "All OK",
            OverallStatus::SomeFailed => "Some Issues",
            OverallStatus::AllFailed => "Action Needed",
        };

        format!(
            "Bench Checklist\n{}\n{} ({}/{})",
            scenario_name, status_text, passed, total
        )
    }
}
