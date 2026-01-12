//! Slint UI Bridge
//! Connects Slint UI components to the Rust AppState

use crate::app::AppState;
use crate::check_library::{get_library, CATEGORIES};
use crate::config::{CheckConfig, CheckType};
use crate::fixer;
use slint::{ModelRc, SharedString, VecModel};

// Include the generated Slint code
slint::include_modules!();

/// Run the Slint-based settings window
pub fn run(app_state: AppState) -> anyhow::Result<()> {
    let ui = MainWindow::new()?;

    // Initial data population
    refresh_all(&ui, &app_state);

    // Set up callbacks
    setup_callbacks(&ui, &app_state);

    // Run the UI event loop
    ui.run()?;

    Ok(())
}

/// Refresh all UI data from AppState
fn refresh_all(ui: &MainWindow, app_state: &AppState) {
    refresh_checks(ui, app_state);
    refresh_scenarios(ui, app_state);
    refresh_library(ui, app_state);
    refresh_settings(ui, app_state);
}

/// Refresh check list from AppState
fn refresh_checks(ui: &MainWindow, app_state: &AppState) {
    let results = app_state.get_last_results();
    let config = app_state.get_config();

    // Get checks from config to include enabled state
    let scenario_checks = config.get_scenario_checks().cloned().unwrap_or_default();

    let items: Vec<CheckItemData> = results
        .iter()
        .map(|r| {
            let enabled = scenario_checks
                .iter()
                .find(|c| c.id == r.id)
                .map(|c| c.enabled)
                .unwrap_or(true);

            CheckItemData {
                id: r.id.clone().into(),
                name: r.name.clone().into(),
                passed: r.passed,
                enabled,
                current_value: r.current_value.clone().into(),
                expected_value: r.expected_value.clone().into(),
                check_type: format!("{:?}", r.id).into(), // Placeholder
            }
        })
        .collect();

    // Calculate pass/fail counts
    let passed = items.iter().filter(|c| c.passed).count() as i32;
    let total = items.len() as i32;

    ui.set_checks(ModelRc::new(VecModel::from(items)));
    ui.set_passed_count(passed);
    ui.set_total_count(total);
}

/// Refresh scenario list from AppState
fn refresh_scenarios(ui: &MainWindow, app_state: &AppState) {
    let scenarios = app_state.get_scenarios();
    let _active = app_state.get_active_scenario();

    let scenario_data: Vec<ScenarioData> = scenarios
        .iter()
        .map(|(_id, name, desc)| ScenarioData {
            name: name.clone().into(),
            description: desc.clone().into(),
        })
        .collect();

    let scenario_names: Vec<SharedString> = scenarios
        .iter()
        .map(|(_, name, _)| name.clone().into())
        .collect();

    ui.set_scenarios(ModelRc::new(VecModel::from(scenario_data)));
    ui.set_scenario_names(ModelRc::new(VecModel::from(scenario_names)));
    ui.set_active_scenario(app_state.get_active_scenario_name().into());
}

/// Refresh check library data
fn refresh_library(ui: &MainWindow, app_state: &AppState) {
    let library = get_library();
    let config = app_state.get_config();
    let existing_ids: Vec<String> = config
        .get_scenario_checks()
        .map(|checks| checks.iter().map(|c| c.id.clone()).collect())
        .unwrap_or_default();

    let library_checks: Vec<LibraryCheckData> = library
        .iter()
        .map(|lc| LibraryCheckData {
            id: lc.id.into(),
            name: lc.name.into(),
            category: lc.category.into(),
            description: lc.description.into(),
            check_type: format!("{:?}", lc.check_type).into(),
            already_added: existing_ids.contains(&lc.id.to_string()),
        })
        .collect();

    let categories: Vec<SharedString> = CATEGORIES.iter().map(|&c| c.into()).collect();

    ui.set_library_checks(ModelRc::new(VecModel::from(library_checks)));
    ui.set_library_categories(ModelRc::new(VecModel::from(categories)));
}

/// Refresh settings from AppState
fn refresh_settings(ui: &MainWindow, app_state: &AppState) {
    ui.set_poll_interval(app_state.get_poll_interval() as i32);
    ui.set_notify_on_drift(app_state.get_notify_on_drift());
}

/// Set up all UI callbacks
fn setup_callbacks(ui: &MainWindow, app_state: &AppState) {
    // Check Now button
    ui.on_check_now_clicked({
        let app_state = app_state.clone();
        let ui_weak = ui.as_weak();
        move || {
            app_state.run_checks();
            if let Some(ui) = ui_weak.upgrade() {
                refresh_checks(&ui, &app_state);
                ui.set_status_message("Checks completed".into());
            }
        }
    });

    // Save button
    ui.on_save_clicked({
        let app_state = app_state.clone();
        let ui_weak = ui.as_weak();
        move || {
            match app_state.save_config() {
                Ok(()) => {
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.set_status_message("Configuration saved".into());
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to save config: {}", e);
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.set_status_message(format!("Error: {}", e).into());
                    }
                }
            }
        }
    });

    // Reload button
    ui.on_reload_clicked({
        let app_state = app_state.clone();
        let ui_weak = ui.as_weak();
        move || {
            match app_state.reload_config() {
                Ok(()) => {
                    if let Some(ui) = ui_weak.upgrade() {
                        refresh_all(&ui, &app_state);
                        ui.set_status_message("Configuration reloaded".into());
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to reload config: {}", e);
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.set_status_message(format!("Error: {}", e).into());
                    }
                }
            }
        }
    });

    // Scenario changed
    ui.on_scenario_changed({
        let app_state = app_state.clone();
        let ui_weak = ui.as_weak();
        move |name| {
            // Find scenario ID by name
            let scenarios = app_state.get_scenarios();
            if let Some((id, _, _)) = scenarios.iter().find(|(_, n, _)| n == name.as_str()) {
                if let Err(e) = app_state.set_active_scenario(id) {
                    tracing::error!("Failed to switch scenario: {}", e);
                }
                // Run checks for new scenario
                app_state.run_checks();
                if let Some(ui) = ui_weak.upgrade() {
                    refresh_all(&ui, &app_state);
                }
            }
        }
    });

    // Check toggled
    ui.on_check_toggled({
        let app_state = app_state.clone();
        let ui_weak = ui.as_weak();
        move |id, _enabled| {
            app_state.toggle_check(&id.to_string());
            if let Some(ui) = ui_weak.upgrade() {
                refresh_checks(&ui, &app_state);
            }
        }
    });

    // Add check clicked (opens editor)
    ui.on_add_check_clicked({
        let ui_weak = ui.as_weak();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_editor_data(CheckEditorData {
                    id: "".into(),
                    name: "".into(),
                    check_type: "PowerScheme".into(),
                    enabled: true,
                    expected_value: "high_performance".into(),
                    registry_path: "".into(),
                    registry_key: "".into(),
                    process_name: "".into(),
                    is_editing: false,
                });
                ui.set_show_check_editor(true);
            }
        }
    });

    // Edit check
    ui.on_edit_check({
        let app_state = app_state.clone();
        let ui_weak = ui.as_weak();
        move |id| {
            let config = app_state.get_config();
            if let Ok(checks) = config.get_scenario_checks() {
                if let Some(check) = checks.iter().find(|c| c.id == id.as_str()) {
                    if let Some(ui) = ui_weak.upgrade() {
                        ui.set_editor_data(check_to_editor_data(check));
                        ui.set_show_check_editor(true);
                    }
                }
            }
        }
    });

    // Delete check
    ui.on_delete_check({
        let app_state = app_state.clone();
        let ui_weak = ui.as_weak();
        move |id| {
            app_state.remove_check(&id.to_string());
            if let Some(ui) = ui_weak.upgrade() {
                refresh_checks(&ui, &app_state);
                ui.set_status_message("Check deleted".into());
            }
        }
    });

    // Save check from editor
    ui.on_save_check({
        let app_state = app_state.clone();
        let ui_weak = ui.as_weak();
        move |data| {
            let check = editor_data_to_check(&data);
            if data.is_editing {
                app_state.update_check(check);
            } else {
                app_state.add_check(check);
            }
            // Run checks to update results
            app_state.run_checks();
            if let Some(ui) = ui_weak.upgrade() {
                refresh_checks(&ui, &app_state);
                refresh_library(&ui, &app_state);
                ui.set_status_message(if data.is_editing {
                    "Check updated".into()
                } else {
                    "Check added".into()
                });
            }
        }
    });

    // Open library popup
    ui.on_open_library({
        let ui_weak = ui.as_weak();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_show_check_library(true);
            }
        }
    });

    // Add from library
    ui.on_add_from_library({
        let app_state = app_state.clone();
        let ui_weak = ui.as_weak();
        move |id| {
            let library = get_library();
            if let Some(lc) = library.iter().find(|l| l.id == id.as_str()) {
                let check = library_check_to_config(lc);
                app_state.add_check(check);
                app_state.run_checks();
                if let Some(ui) = ui_weak.upgrade() {
                    refresh_checks(&ui, &app_state);
                    refresh_library(&ui, &app_state);
                    ui.set_status_message(format!("Added: {}", lc.name).into());
                }
            }
        }
    });

    // Poll interval changed
    ui.on_poll_interval_changed({
        let app_state = app_state.clone();
        move |val| {
            app_state.set_poll_interval(val as u64);
        }
    });

    // Notify on drift changed
    ui.on_notify_drift_changed({
        let app_state = app_state.clone();
        move |val| {
            app_state.set_notify_on_drift(val);
        }
    });

    // Fix all clicked
    ui.on_fix_all_clicked({
        let app_state = app_state.clone();
        let ui_weak = ui.as_weak();
        move || {
            let config = app_state.get_config();
            if let Ok(checks) = config.get_scenario_checks() {
                let results = app_state.get_last_results();
                let failed_checks: Vec<&CheckConfig> = checks
                    .iter()
                    .filter(|c| {
                        results.iter().any(|r| r.id == c.id && !r.passed)
                    })
                    .collect();

                let mut fixed_count = 0;
                for check in failed_checks {
                    let fix_result = fixer::fix_check(check);
                    if fix_result.success {
                        fixed_count += 1;
                    }
                }

                // Re-run checks after fixing
                app_state.run_checks();

                if let Some(ui) = ui_weak.upgrade() {
                    refresh_checks(&ui, &app_state);
                    ui.set_status_message(format!("Fixed {} checks", fixed_count).into());
                }
            }
        }
    });
}

/// Convert CheckConfig to CheckEditorData
fn check_to_editor_data(check: &CheckConfig) -> CheckEditorData {
    CheckEditorData {
        id: check.id.clone().into(),
        name: check.name.clone().into(),
        check_type: format!("{:?}", check.check_type).into(),
        enabled: check.enabled,
        expected_value: check.expected_value.clone().unwrap_or_default().into(),
        registry_path: check.registry_path.clone().unwrap_or_default().into(),
        registry_key: check.registry_key.clone().unwrap_or_default().into(),
        process_name: check.process_name.clone().unwrap_or_default().into(),
        is_editing: true,
    }
}

/// Convert CheckEditorData to CheckConfig
fn editor_data_to_check(data: &CheckEditorData) -> CheckConfig {
    let check_type = parse_check_type(&data.check_type.to_string());

    CheckConfig {
        id: data.id.to_string(),
        name: data.name.to_string(),
        check_type,
        enabled: data.enabled,
        expected_value: if data.expected_value.is_empty() {
            None
        } else {
            Some(data.expected_value.to_string())
        },
        registry_path: if data.registry_path.is_empty() {
            None
        } else {
            Some(data.registry_path.to_string())
        },
        registry_key: if data.registry_key.is_empty() {
            None
        } else {
            Some(data.registry_key.to_string())
        },
        process_name: if data.process_name.is_empty() {
            None
        } else {
            Some(data.process_name.to_string())
        },
    }
}

/// Convert LibraryCheck to CheckConfig
fn library_check_to_config(lc: &crate::check_library::LibraryCheck) -> CheckConfig {
    CheckConfig {
        id: lc.id.to_string(),
        name: lc.name.to_string(),
        check_type: lc.check_type.clone(),
        enabled: true,
        expected_value: lc.expected_value.map(|s| s.to_string()),
        registry_path: lc.registry_path.map(|s| s.to_string()),
        registry_key: lc.registry_key.map(|s| s.to_string()),
        process_name: lc.process_name.map(|s| s.to_string()),
    }
}

/// Parse check type from string
fn parse_check_type(s: &str) -> CheckType {
    match s {
        "PowerScheme" => CheckType::PowerScheme,
        "PowerMode" => CheckType::PowerMode,
        "RegistryDword" => CheckType::RegistryDword,
        "RegistryString" => CheckType::RegistryString,
        "ProcessAbsent" => CheckType::ProcessAbsent,
        "ProcessPresent" => CheckType::ProcessPresent,
        "DisplayResolution" => CheckType::DisplayResolution,
        "DisplayRefreshRate" => CheckType::DisplayRefreshRate,
        "HdrEnabled" => CheckType::HdrEnabled,
        _ => CheckType::PowerScheme, // Default fallback
    }
}
