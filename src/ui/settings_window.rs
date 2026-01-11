use crate::app::AppState;
use crate::check_library::{get_library, LibraryCheck, CATEGORIES};
use crate::checkers::OverallStatus;
use crate::config::{CheckConfig, CheckType, Scenario};
use crate::fixer;
use crate::ui::style::AppStyle;
use eframe::egui;
use std::collections::HashSet;

/// State for adding/editing a check
#[derive(Default)]
struct CheckEditor {
    visible: bool,
    editing_id: Option<String>, // None = adding new, Some(id) = editing existing
    id: String,
    name: String,
    check_type: usize, // Index into CHECK_TYPES
    enabled: bool,
    // Type-specific fields
    registry_path: String,
    registry_key: String,
    process_name: String,
    expected_value: String,
}

/// Available check types for the dropdown
const CHECK_TYPES: &[(&str, CheckType)] = &[
    ("Power Scheme", CheckType::PowerScheme),
    ("Power Mode", CheckType::PowerMode),
    ("Registry DWORD", CheckType::RegistryDword),
    ("Registry String", CheckType::RegistryString),
    ("Process Absent", CheckType::ProcessAbsent),
    ("Process Present", CheckType::ProcessPresent),
];

/// State for the Check Library popup
#[derive(Default)]
struct LibraryPopup {
    visible: bool,
    expanded_categories: HashSet<String>,
    search_query: String,
}

/// Filter tabs for the check list
#[derive(Clone, Copy, PartialEq, Default)]
enum CheckFilter {
    #[default]
    All,
    Failed,
    Passed,
}

pub struct SettingsWindow {
    app_state: AppState,
    current_scenario: String,
    poll_interval: u64,
    notify_on_drift: bool,
    status_message: Option<String>,
    status_message_time: Option<std::time::Instant>,
    check_editor: CheckEditor,
    confirm_delete: Option<String>, // ID of check pending deletion
    library_popup: LibraryPopup,
    check_filter: CheckFilter,
    fixing_in_progress: bool,
}

impl SettingsWindow {
    pub fn new(app_state: AppState) -> Self {
        let current_scenario = app_state.get_active_scenario();
        let poll_interval = app_state.get_poll_interval();
        let notify_on_drift = app_state.get_notify_on_drift();

        // Initialize library popup with first category expanded
        let mut expanded_categories = HashSet::new();
        if let Some(first_cat) = CATEGORIES.first() {
            expanded_categories.insert(first_cat.to_string());
        }

        Self {
            app_state,
            current_scenario,
            poll_interval,
            notify_on_drift,
            status_message: None,
            status_message_time: None,
            check_editor: CheckEditor::default(),
            confirm_delete: None,
            library_popup: LibraryPopup {
                visible: false,
                expanded_categories,
                search_query: String::new(),
            },
            check_filter: CheckFilter::default(),
            fixing_in_progress: false,
        }
    }

    /// Open the Check Library popup
    fn open_library(&mut self) {
        self.library_popup.visible = true;
        self.library_popup.search_query.clear();
    }

    /// Add a check from the library to the current scenario
    fn add_from_library(&mut self, check: &LibraryCheck) {
        let check_config = check.to_check_config();
        self.app_state.add_check(check_config);
        self.status_message = Some(format!("Added: {}", check.name));
    }

    /// Get set of check IDs already in the current scenario
    fn get_existing_check_ids(&self) -> HashSet<String> {
        let config = self.app_state.get_config();
        config
            .get_scenario_checks()
            .map(|checks| checks.iter().map(|c| c.id.clone()).collect())
            .unwrap_or_default()
    }

    /// Export current scenario to a JSON file
    fn export_scenario(&mut self) {
        use rfd::FileDialog;

        let config = self.app_state.get_config();
        if let Ok(scenario) = config.get_active_scenario_data() {
            // Build export structure
            let export = serde_json::json!({
                "export_version": 1,
                "exported_at": chrono::Utc::now().to_rfc3339(),
                "scenario": scenario
            });

            if let Some(path) = FileDialog::new()
                .add_filter("JSON", &["json"])
                .set_file_name(&format!("{}.json", self.current_scenario))
                .save_file()
            {
                match std::fs::write(&path, serde_json::to_string_pretty(&export).unwrap()) {
                    Ok(_) => {
                        self.status_message = Some(format!("Exported to: {}", path.display()));
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Export failed: {}", e));
                    }
                }
            }
        }
    }

    /// Import a scenario from a JSON file
    fn import_scenario(&mut self) {
        use rfd::FileDialog;

        if let Some(path) = FileDialog::new()
            .add_filter("JSON", &["json"])
            .pick_file()
        {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    // Try to parse as export format
                    if let Ok(export) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(scenario_value) = export.get("scenario") {
                            match serde_json::from_value::<Scenario>(scenario_value.clone()) {
                                Ok(scenario) => {
                                    // Generate unique ID for imported scenario
                                    let base_id = scenario.name.to_lowercase().replace(' ', "_");
                                    let scenario_id = self.generate_unique_scenario_id(&base_id);

                                    if let Err(e) = self.app_state.add_scenario(&scenario_id, scenario.clone()) {
                                        self.status_message = Some(format!("Import failed: {}", e));
                                    } else {
                                        self.status_message = Some(format!("Imported: {}", scenario.name));
                                    }
                                }
                                Err(e) => {
                                    self.status_message = Some(format!("Invalid scenario format: {}", e));
                                }
                            }
                        } else {
                            self.status_message = Some("Invalid export file: missing scenario".to_string());
                        }
                    } else {
                        self.status_message = Some("Invalid JSON file".to_string());
                    }
                }
                Err(e) => {
                    self.status_message = Some(format!("Failed to read file: {}", e));
                }
            }
        }
    }

    /// Generate a unique scenario ID
    fn generate_unique_scenario_id(&self, base: &str) -> String {
        let existing: HashSet<String> = self.app_state.get_scenarios()
            .iter()
            .map(|(id, _, _)| id.clone())
            .collect();

        if !existing.contains(base) {
            return base.to_string();
        }

        let mut counter = 2;
        loop {
            let candidate = format!("{}_{}", base, counter);
            if !existing.contains(&candidate) {
                return candidate;
            }
            counter += 1;
        }
    }

    /// Open editor to add a new check
    fn open_add_check(&mut self) {
        self.check_editor = CheckEditor {
            visible: true,
            editing_id: None,
            id: String::new(),
            name: String::new(),
            check_type: 0,
            enabled: true,
            registry_path: String::new(),
            registry_key: String::new(),
            process_name: String::new(),
            expected_value: String::new(),
        };
    }

    /// Open editor to edit an existing check
    fn open_edit_check(&mut self, check: &CheckConfig) {
        let check_type_idx = CHECK_TYPES
            .iter()
            .position(|(_, t)| *t == check.check_type)
            .unwrap_or(0);

        self.check_editor = CheckEditor {
            visible: true,
            editing_id: Some(check.id.clone()),
            id: check.id.clone(),
            name: check.name.clone(),
            check_type: check_type_idx,
            enabled: check.enabled,
            registry_path: check.registry_path.clone().unwrap_or_default(),
            registry_key: check.registry_key.clone().unwrap_or_default(),
            process_name: check.process_name.clone().unwrap_or_default(),
            expected_value: check.expected_value.clone().unwrap_or_default(),
        };
    }

    /// Build a CheckConfig from editor state
    fn build_check_from_editor(&self) -> CheckConfig {
        let check_type = CHECK_TYPES[self.check_editor.check_type].1.clone();

        CheckConfig {
            id: self.check_editor.id.clone(),
            name: self.check_editor.name.clone(),
            check_type: check_type.clone(),
            enabled: self.check_editor.enabled,
            registry_path: match check_type {
                CheckType::RegistryDword | CheckType::RegistryString => {
                    Some(self.check_editor.registry_path.clone())
                }
                _ => None,
            },
            registry_key: match check_type {
                CheckType::RegistryDword | CheckType::RegistryString => {
                    Some(self.check_editor.registry_key.clone())
                }
                _ => None,
            },
            process_name: match check_type {
                CheckType::ProcessAbsent | CheckType::ProcessPresent => {
                    Some(self.check_editor.process_name.clone())
                }
                _ => None,
            },
            expected_value: if self.check_editor.expected_value.is_empty() {
                None
            } else {
                Some(self.check_editor.expected_value.clone())
            },
        }
    }

    pub fn run(app_state: AppState) -> anyhow::Result<()> {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([520.0, 700.0])
                .with_min_inner_size([450.0, 500.0])
                .with_title("Bench Checklist"),
            ..Default::default()
        };

        eframe::run_native(
            "Bench Checklist",
            options,
            Box::new(|cc| {
                // Apply dark theme
                AppStyle::apply_dark_theme(&cc.egui_ctx);
                Ok(Box::new(SettingsWindow::new(app_state)))
            }),
        )
        .map_err(|e| anyhow::anyhow!("Failed to run settings window: {}", e))
    }
}

impl eframe::App for SettingsWindow {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check if app is exiting
        if self.app_state.should_exit() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // Handle keyboard shortcuts
        if ctx.input(|i| i.key_pressed(egui::Key::R) && i.modifiers.ctrl) {
            // Ctrl+R: Check Now
            self.app_state.run_checks();
            self.status_message = Some("✓ Checks completed".to_string());
            self.status_message_time = Some(std::time::Instant::now());
        }

        if ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.ctrl) {
            // Ctrl+S: Apply Settings
            self.app_state.set_poll_interval(self.poll_interval);
            self.app_state.set_notify_on_drift(self.notify_on_drift);

            if let Err(e) = self.app_state.save_config() {
                self.status_message = Some(format!("✗ Failed to save: {}", e));
            } else {
                self.status_message = Some("✓ Settings saved".to_string());
            }
            self.status_message_time = Some(std::time::Instant::now());
        }

        if ctx.input(|i| i.key_pressed(egui::Key::L) && i.modifiers.ctrl) {
            // Ctrl+L: Reload Config
            if let Err(e) = self.app_state.reload_config() {
                self.status_message = Some(format!("✗ Failed to reload: {}", e));
            } else {
                self.poll_interval = self.app_state.get_poll_interval();
                self.notify_on_drift = self.app_state.get_notify_on_drift();
                self.current_scenario = self.app_state.get_active_scenario();
                self.status_message = Some("✓ Config reloaded".to_string());
            }
            self.status_message_time = Some(std::time::Instant::now());
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(AppStyle::SPACING_SM);

            // Compact header with title and quick status
            ui.horizontal(|ui| {
                ui.heading(
                    egui::RichText::new("BENCH CHECKLIST")
                        .size(AppStyle::FONT_SIZE_XL)
                        .color(AppStyle::COLOR_PRIMARY)
                        .strong()
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Quick Check Now button in header
                    if ui.add(
                        egui::Button::new("▶ Check Now")
                            .fill(AppStyle::COLOR_PRIMARY)
                    ).on_hover_text("Run all checks immediately (Ctrl+R)").clicked() {
                        self.app_state.run_checks();
                        self.status_message = Some("✓ Checks completed".to_string());
                        self.status_message_time = Some(std::time::Instant::now());
                    }
                });
            });

            ui.add_space(AppStyle::SPACING_SM);

            // Large, prominent status card with actionable guidance
            let results = self.app_state.get_last_results();
            let status = OverallStatus::from_results(&results);
            let passed = results.iter().filter(|r| r.passed).count();
            let total = results.len();
            let failed_count = total - passed;

            let (status_color, status_icon, status_text, guidance) = match status {
                OverallStatus::AllPassed => (
                    AppStyle::COLOR_SUCCESS,
                    "✓",
                    "Ready to Benchmark".to_string(),
                    "All checks passed - your system is configured correctly."
                ),
                OverallStatus::SomeFailed => (
                    AppStyle::COLOR_WARNING,
                    "⚠",
                    format!("{} Issue{} Found", failed_count, if failed_count == 1 { "" } else { "s" }),
                    "Review the failed checks below and fix before benchmarking."
                ),
                OverallStatus::AllFailed => (
                    AppStyle::COLOR_ERROR,
                    "✗",
                    "Not Ready".to_string(),
                    "Multiple settings need attention before benchmarking."
                ),
            };

            egui::Frame::none()
                .fill(status_color.gamma_multiply(0.12))
                .stroke(egui::Stroke::new(1.5, status_color.gamma_multiply(0.5)))
                .rounding(AppStyle::RADIUS_MD)
                .inner_margin(egui::Margin::symmetric(AppStyle::SPACING_MD, AppStyle::SPACING_SM))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Status icon with glow effect
                        ui.label(
                            egui::RichText::new(status_icon)
                                .size(32.0)
                                .color(status_color)
                        );

                        ui.add_space(AppStyle::SPACING_SM);

                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(&status_text)
                                    .size(AppStyle::FONT_SIZE_LG)
                                    .color(status_color)
                                    .strong()
                            );
                            ui.label(
                                egui::RichText::new(guidance)
                                    .size(AppStyle::FONT_SIZE_SMALL)
                                    .color(AppStyle::COLOR_TEXT_SECONDARY)
                            );

                            // Progress bar
                            ui.add_space(AppStyle::SPACING_XS);
                            let progress = if total > 0 { passed as f32 / total as f32 } else { 0.0 };
                            let progress_bar = egui::ProgressBar::new(progress)
                                .fill(status_color)
                                .animate(false);
                            ui.add_sized([ui.available_width() - 80.0, 6.0], progress_bar);
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Fix All button (only show if there are fixable failures)
                            if failed_count > 0 {
                                let config = self.app_state.get_config();
                                let checks = config.get_scenario_checks()
                                    .cloned()
                                    .unwrap_or_default();
                                let failing_ids: Vec<String> = results.iter()
                                    .filter(|r| !r.passed)
                                    .map(|r| r.id.clone())
                                    .collect();
                                let (direct, admin, _manual) = fixer::get_fix_counts(&checks, &failing_ids);
                                let fixable = direct + admin;

                                if fixable > 0 {
                                    let button_text = if self.fixing_in_progress {
                                        "Fixing..."
                                    } else if admin > 0 {
                                        "Fix All 🔒"
                                    } else {
                                        "Fix All"
                                    };

                                    let button = egui::Button::new(
                                        egui::RichText::new(button_text)
                                            .color(egui::Color32::WHITE)
                                    )
                                    .fill(AppStyle::COLOR_PRIMARY)
                                    .rounding(AppStyle::RADIUS_SM);

                                    let tooltip = if admin > 0 {
                                        format!("{} fixes ({} need admin)", fixable, admin)
                                    } else {
                                        format!("{} fixes available", fixable)
                                    };

                                    if ui.add_enabled(!self.fixing_in_progress, button)
                                        .on_hover_text(&tooltip)
                                        .clicked()
                                    {
                                        self.fixing_in_progress = true;
                                        let fix_results = fixer::fix_all(&checks, &failing_ids);
                                        self.fixing_in_progress = false;

                                        let success_count = fix_results.iter().filter(|r| r.success).count();
                                        let fail_count = fix_results.len() - success_count;

                                        if fail_count == 0 {
                                            self.status_message = Some(format!("✓ Fixed {} issue{}", success_count, if success_count == 1 { "" } else { "s" }));
                                        } else if success_count > 0 {
                                            self.status_message = Some(format!("⚠ Fixed {}, {} failed", success_count, fail_count));
                                        } else {
                                            self.status_message = Some("✗ Could not fix issues".to_string());
                                        }
                                        self.status_message_time = Some(std::time::Instant::now());

                                        // Re-run checks to see updated status
                                        self.app_state.run_checks();
                                    }

                                    ui.add_space(AppStyle::SPACING_SM);
                                }
                            }

                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("{}", passed))
                                        .size(28.0)
                                        .color(status_color)
                                        .strong()
                                );
                                ui.label(
                                    egui::RichText::new(format!("of {}", total))
                                        .size(AppStyle::FONT_SIZE_SMALL)
                                        .color(AppStyle::COLOR_TEXT_MUTED)
                                );
                            });
                        });
                    });
                });

            ui.add_space(AppStyle::SPACING_MD);

            // Compact scenario and poll settings in one row
            AppStyle::card_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Scenario selector
                    ui.label(
                        egui::RichText::new("Scenario:")
                            .color(AppStyle::COLOR_TEXT_SECONDARY)
                    );

                    let scenarios = self.app_state.get_scenarios();
                    let current_scenario_name = self.app_state.get_active_scenario_name();

                    egui::ComboBox::from_id_source("scenario_combo")
                        .selected_text(&current_scenario_name)
                        .width(200.0)
                        .show_ui(ui, |ui| {
                            for (id, name, description) in &scenarios {
                                let mut response = ui.selectable_label(id == &self.current_scenario, name);
                                if !description.is_empty() {
                                    response = response.on_hover_text(description);
                                }
                                if response.clicked() {
                                    if let Err(e) = self.app_state.set_active_scenario(id) {
                                        self.status_message = Some(format!("Failed to switch: {}", e));
                                    } else {
                                        self.current_scenario = id.clone();
                                        self.poll_interval = self.app_state.get_poll_interval();
                                        self.notify_on_drift = self.app_state.get_notify_on_drift();
                                        self.app_state.run_checks();
                                        self.status_message = Some(format!("Switched to {}", name));
                                    }
                                }
                            }
                        });

                    ui.separator();

                    // Poll interval (compact)
                    ui.label(
                        egui::RichText::new("Poll:")
                            .color(AppStyle::COLOR_TEXT_SECONDARY)
                    );

                    // Simple button group for poll interval
                    for (secs, label) in [(5, "5s"), (10, "10s"), (30, "30s")] {
                        let is_selected = self.poll_interval == secs;
                        if ui.add(
                            egui::Button::new(label)
                                .fill(if is_selected { AppStyle::COLOR_PRIMARY } else { AppStyle::COLOR_BG_ELEVATED })
                                .min_size(egui::vec2(36.0, 20.0))
                        ).on_hover_text(match secs {
                            5 => "Fast updates (more CPU)",
                            10 => "Balanced (recommended)",
                            30 => "Battery saver",
                            _ => ""
                        }).clicked() {
                            self.poll_interval = secs;
                        }
                    }

                    ui.separator();

                    // Drift notifications toggle
                    ui.checkbox(&mut self.notify_on_drift, "")
                        .on_hover_text("Show Windows notifications when settings drift from expected values");
                    ui.label(
                        egui::RichText::new("Notify on drift")
                            .size(AppStyle::FONT_SIZE_SMALL)
                            .color(AppStyle::COLOR_TEXT_SECONDARY)
                    );
                });
            });

            ui.add_space(AppStyle::SPACING_MD);

            // Checks section header with filter tabs and actions
            ui.horizontal(|ui| {
                // All tab
                let all_text = format!("All ({})", total);
                if ui.add(
                    egui::Button::new(
                        egui::RichText::new(&all_text)
                            .size(AppStyle::FONT_SIZE_SMALL)
                            .color(if self.check_filter == CheckFilter::All { egui::Color32::WHITE } else { AppStyle::COLOR_TEXT_SECONDARY })
                    )
                    .fill(if self.check_filter == CheckFilter::All { AppStyle::COLOR_PRIMARY } else { AppStyle::COLOR_BG_ELEVATED })
                    .rounding(AppStyle::RADIUS_SM)
                ).clicked() {
                    self.check_filter = CheckFilter::All;
                }

                // Failed tab
                let failed_text = format!("Failed ({})", failed_count);
                if ui.add(
                    egui::Button::new(
                        egui::RichText::new(&failed_text)
                            .size(AppStyle::FONT_SIZE_SMALL)
                            .color(if self.check_filter == CheckFilter::Failed {
                                egui::Color32::WHITE
                            } else if failed_count > 0 {
                                AppStyle::COLOR_ERROR
                            } else {
                                AppStyle::COLOR_TEXT_MUTED
                            })
                    )
                    .fill(if self.check_filter == CheckFilter::Failed { AppStyle::COLOR_ERROR } else { AppStyle::COLOR_BG_ELEVATED })
                    .rounding(AppStyle::RADIUS_SM)
                ).clicked() {
                    self.check_filter = CheckFilter::Failed;
                }

                // Passed tab
                let passed_text = format!("Passed ({})", passed);
                if ui.add(
                    egui::Button::new(
                        egui::RichText::new(&passed_text)
                            .size(AppStyle::FONT_SIZE_SMALL)
                            .color(if self.check_filter == CheckFilter::Passed { egui::Color32::WHITE } else { AppStyle::COLOR_SUCCESS })
                    )
                    .fill(if self.check_filter == CheckFilter::Passed { AppStyle::COLOR_SUCCESS } else { AppStyle::COLOR_BG_ELEVATED })
                    .rounding(AppStyle::RADIUS_SM)
                ).clicked() {
                    self.check_filter = CheckFilter::Passed;
                }

                // Right-aligned action buttons
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(
                        egui::Button::new("+ Add")
                            .rounding(AppStyle::RADIUS_SM)
                    ).on_hover_text("Add a custom check").clicked() {
                        self.open_add_check();
                    }
                    if ui.add(
                        egui::Button::new(
                            egui::RichText::new("📚 Library")
                                .color(egui::Color32::WHITE)
                        )
                        .fill(AppStyle::COLOR_PRIMARY)
                        .rounding(AppStyle::RADIUS_SM)
                    ).on_hover_text("Browse pre-defined checks").clicked() {
                        self.open_library();
                    }
                });
            });
            ui.add_space(AppStyle::SPACING_SM);

            let results = self.app_state.get_last_results();

            // Track actions to perform after iteration
            let mut check_to_edit: Option<CheckConfig> = None;
            let mut check_to_delete: Option<String> = None;

            AppStyle::card_frame().show(ui, |ui| {
                // Dynamic scroll height based on window height
                let available_height = ui.available_height();
                let scroll_height = (available_height - 250.0).max(150.0);

                egui::ScrollArea::vertical()
                    .max_height(scroll_height)
                    .show(ui, |ui| {
                        let config = self.app_state.get_config();
                        let checks = config.get_scenario_checks()
                            .map(|c| c.clone())
                            .unwrap_or_default();

                        if checks.is_empty() {
                            ui.vertical_centered(|ui| {
                                ui.add_space(AppStyle::SPACING_XL);
                                ui.label(
                                    egui::RichText::new("📋")
                                        .size(32.0)
                                        .color(AppStyle::COLOR_TEXT_MUTED)
                                );
                                ui.add_space(AppStyle::SPACING_SM);
                                ui.label(
                                    egui::RichText::new("No checks configured")
                                        .size(AppStyle::FONT_SIZE_MD)
                                        .color(AppStyle::COLOR_TEXT_SECONDARY)
                                );
                                ui.label(
                                    egui::RichText::new("Click 'Library' to add pre-configured checks")
                                        .size(AppStyle::FONT_SIZE_SMALL)
                                        .color(AppStyle::COLOR_TEXT_MUTED)
                                );
                                ui.add_space(AppStyle::SPACING_XL);
                            });
                        }

                        // Filter checks based on selected tab
                        let filtered_checks: Vec<_> = checks.iter().filter(|check| {
                            let result = results.iter().find(|r| r.id == check.id);
                            match self.check_filter {
                                CheckFilter::All => true,
                                CheckFilter::Failed => {
                                    check.enabled && result.map(|r| !r.passed).unwrap_or(false)
                                }
                                CheckFilter::Passed => {
                                    check.enabled && result.map(|r| r.passed).unwrap_or(false)
                                }
                            }
                        }).collect();

                        // Show empty state for filtered view
                        if !checks.is_empty() && filtered_checks.is_empty() {
                            ui.vertical_centered(|ui| {
                                ui.add_space(AppStyle::SPACING_LG);
                                let (icon, message) = match self.check_filter {
                                    CheckFilter::Failed => ("✓", "No failing checks!"),
                                    CheckFilter::Passed => ("○", "No passing checks yet"),
                                    CheckFilter::All => ("", ""),
                                };
                                ui.label(
                                    egui::RichText::new(icon)
                                        .size(24.0)
                                        .color(AppStyle::COLOR_SUCCESS)
                                );
                                ui.label(
                                    egui::RichText::new(message)
                                        .color(AppStyle::COLOR_TEXT_SECONDARY)
                                );
                                ui.add_space(AppStyle::SPACING_LG);
                            });
                        }

                        for check in filtered_checks {
                            let result = results.iter().find(|r| r.id == check.id);

                            // Card-style check row with colored left border
                            let (border_color, bg_alpha) = match result {
                                Some(r) if r.passed && check.enabled => (AppStyle::COLOR_SUCCESS, 0.05),
                                Some(_) if check.enabled => (AppStyle::COLOR_ERROR, 0.08),
                                _ => (AppStyle::COLOR_TEXT_MUTED, 0.02),
                            };

                            egui::Frame::none()
                                .fill(border_color.gamma_multiply(bg_alpha))
                                .rounding(AppStyle::RADIUS_SM)
                                .inner_margin(egui::Margin::symmetric(AppStyle::SPACING_SM, AppStyle::SPACING_XS))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        // Colored status indicator bar
                                        let (rect, _response) = ui.allocate_exact_size(
                                            egui::vec2(4.0, 20.0),
                                            egui::Sense::hover()
                                        );
                                        ui.painter().rect_filled(
                                            rect,
                                            AppStyle::RADIUS_SM,
                                            if check.enabled { border_color } else { AppStyle::COLOR_TEXT_MUTED }
                                        );

                                        ui.add_space(AppStyle::SPACING_SM);

                                        // Status icon with meaning
                                        let (indicator_text, tooltip) = match result {
                                            Some(r) if r.passed => ("✓", "Passing - configured correctly"),
                                            Some(_) => ("✗", "Failing - needs attention"),
                                            None => ("○", "Not checked yet"),
                                        };

                                        if check.enabled {
                                            ui.label(
                                                egui::RichText::new(indicator_text)
                                                    .color(border_color)
                                                    .size(AppStyle::FONT_SIZE_MD)
                                            ).on_hover_text(tooltip);
                                        } else {
                                            ui.label(
                                                egui::RichText::new("—")
                                                    .color(AppStyle::COLOR_TEXT_MUTED)
                                            ).on_hover_text("Check is disabled");
                                        }

                                        // Check name with toggle
                                        let mut enabled = check.enabled;
                                        let response = ui.checkbox(&mut enabled, "");
                                        if response.changed() {
                                            self.app_state.toggle_check(&check.id);
                                        }
                                        response.on_hover_text(if enabled { "Click to disable this check" } else { "Click to enable this check" });

                                        // Check name (clickable to show details)
                                        ui.label(
                                            egui::RichText::new(&check.name)
                                                .color(if check.enabled { AppStyle::COLOR_TEXT_PRIMARY } else { AppStyle::COLOR_TEXT_MUTED })
                                        );

                                        // Edit and Delete buttons (right-aligned)
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            // Delete button
                                            if self.confirm_delete.as_ref() == Some(&check.id) {
                                                // Confirm deletion
                                                if ui.button("Cancel").clicked() {
                                                    self.confirm_delete = None;
                                                }
                                                if ui.add(
                                                    egui::Button::new(
                                                        egui::RichText::new("Delete")
                                                            .color(egui::Color32::WHITE)
                                                    ).fill(AppStyle::COLOR_ERROR)
                                                ).clicked() {
                                                    check_to_delete = Some(check.id.clone());
                                                    self.confirm_delete = None;
                                                }
                                            } else {
                                                if ui.small_button("🗑")
                                                    .on_hover_text("Remove this check")
                                                    .clicked()
                                                {
                                                    self.confirm_delete = Some(check.id.clone());
                                                }
                                                if ui.small_button("✎")
                                                    .on_hover_text("Edit check settings")
                                                    .clicked()
                                                {
                                                    check_to_edit = Some(check.clone());
                                                }
                                            }
                                        });
                                    });

                                    // Show current value and change indicator (indented)
                                    if let Some(r) = result {
                                        if check.enabled {
                                            // Show current vs expected for failed checks
                                            if !r.passed {
                                                ui.horizontal(|ui| {
                                                    ui.add_space(AppStyle::SPACING_XL);
                                                    ui.label(
                                                        egui::RichText::new(format!("→ Current: {} (expected: {})", r.current_value, r.expected_value))
                                                            .size(AppStyle::FONT_SIZE_SMALL)
                                                            .color(AppStyle::COLOR_ERROR)
                                                    );
                                                });
                                            }

                                        }
                                    }
                                });

                            ui.add_space(AppStyle::SPACING_XS);
                        }
                    });
            });

            // Handle deferred actions
            if let Some(check) = check_to_edit {
                self.open_edit_check(&check);
            }
            if let Some(id) = check_to_delete {
                self.app_state.remove_check(&id);
                self.status_message = Some("✓ Check removed".to_string());
            }

            ui.add_space(AppStyle::SPACING_MD);

            // Collapsible settings section
            egui::CollapsingHeader::new(
                egui::RichText::new("⚙ Advanced Settings")
                    .size(AppStyle::FONT_SIZE_MD)
                    .color(AppStyle::COLOR_TEXT_SECONDARY)
            )
            .default_open(false)
            .show(ui, |ui| {
                ui.add_space(AppStyle::SPACING_SM);

                // Config actions in a subtle card
                AppStyle::card_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Save button (primary)
                        if ui.add(
                            egui::Button::new(
                                egui::RichText::new("💾 Save Config")
                                    .color(egui::Color32::WHITE)
                            )
                            .fill(AppStyle::COLOR_PRIMARY)
                        ).on_hover_text("Save all settings to config file (Ctrl+S)").clicked() {
                            self.app_state.set_poll_interval(self.poll_interval);
                            self.app_state.set_notify_on_drift(self.notify_on_drift);

                            if let Err(e) = self.app_state.save_config() {
                                self.status_message = Some(format!("✗ Failed to save: {}", e));
                            } else {
                                self.status_message = Some("✓ Settings saved".to_string());
                            }
                        }

                        ui.add_space(AppStyle::SPACING_SM);

                        // Reload button
                        if ui.button("↻ Reload").on_hover_text("Reload config from file (Ctrl+L)").clicked() {
                            if let Err(e) = self.app_state.reload_config() {
                                self.status_message = Some(format!("✗ Failed to reload: {}", e));
                            } else {
                                self.poll_interval = self.app_state.get_poll_interval();
                                self.notify_on_drift = self.app_state.get_notify_on_drift();
                                self.current_scenario = self.app_state.get_active_scenario();
                                self.status_message = Some("✓ Config reloaded".to_string());
                            }
                        }

                        ui.separator();

                        // Export/Import
                        if ui.button("📤 Export").on_hover_text("Export scenario to share with others").clicked() {
                            self.export_scenario();
                        }

                        if ui.button("📥 Import").on_hover_text("Import a scenario from file").clicked() {
                            self.import_scenario();
                        }
                    });

                    // Keyboard shortcuts help
                    ui.add_space(AppStyle::SPACING_SM);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Shortcuts:")
                                .size(AppStyle::FONT_SIZE_XS)
                                .color(AppStyle::COLOR_TEXT_MUTED)
                        );
                        ui.label(
                            egui::RichText::new("Ctrl+R Check  •  Ctrl+S Save  •  Ctrl+L Reload")
                                .size(AppStyle::FONT_SIZE_XS)
                                .color(AppStyle::COLOR_TEXT_MUTED)
                        );
                    });
                });
            });

            // Auto-clear status message after 5 seconds
            if let Some(time) = self.status_message_time {
                if time.elapsed().as_secs() > 5 {
                    self.status_message = None;
                    self.status_message_time = None;
                }
            }

            // Status message toast (subtle, at bottom)
            if let Some(msg) = &self.status_message {
                ui.add_space(AppStyle::SPACING_SM);
                let (msg_color, bg_color) = if msg.starts_with('✓') {
                    (AppStyle::COLOR_SUCCESS, AppStyle::COLOR_SUCCESS.gamma_multiply(0.15))
                } else if msg.starts_with('✗') {
                    (AppStyle::COLOR_ERROR, AppStyle::COLOR_ERROR.gamma_multiply(0.15))
                } else {
                    (AppStyle::COLOR_TEXT_SECONDARY, AppStyle::COLOR_BG_ELEVATED)
                };

                egui::Frame::none()
                    .fill(bg_color)
                    .rounding(AppStyle::RADIUS_SM)
                    .inner_margin(egui::Margin::symmetric(AppStyle::SPACING_SM, AppStyle::SPACING_XS))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(msg)
                                .size(AppStyle::FONT_SIZE_SMALL)
                                .color(msg_color)
                        );
                    });
            }

            ui.add_space(AppStyle::SPACING_SM);
        });

        // Check Editor Window (modal-like)
        if self.check_editor.visible {
            egui::Window::new(if self.check_editor.editing_id.is_some() { "Edit Check" } else { "Add Check" })
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_min_width(400.0);

                    egui::Grid::new("check_editor_grid")
                        .num_columns(2)
                        .spacing([10.0, 8.0])
                        .show(ui, |ui| {
                            // ID field (only editable when adding new)
                            ui.label("ID:");
                            ui.add_enabled(
                                self.check_editor.editing_id.is_none(),
                                egui::TextEdit::singleline(&mut self.check_editor.id)
                                    .hint_text("unique_id")
                            );
                            ui.end_row();

                            // Name field
                            ui.label("Name:");
                            ui.text_edit_singleline(&mut self.check_editor.name);
                            ui.end_row();

                            // Check type dropdown
                            ui.label("Type:");
                            egui::ComboBox::from_id_source("check_type_combo")
                                .selected_text(CHECK_TYPES[self.check_editor.check_type].0)
                                .show_ui(ui, |ui| {
                                    for (i, (name, _)) in CHECK_TYPES.iter().enumerate() {
                                        ui.selectable_value(&mut self.check_editor.check_type, i, *name);
                                    }
                                });
                            ui.end_row();

                            // Enabled checkbox
                            ui.label("Enabled:");
                            ui.checkbox(&mut self.check_editor.enabled, "");
                            ui.end_row();

                            // Type-specific fields
                            let check_type = &CHECK_TYPES[self.check_editor.check_type].1;

                            match check_type {
                                CheckType::PowerScheme => {
                                    ui.label("Expected:");
                                    egui::ComboBox::from_id_source("power_scheme_combo")
                                        .selected_text(&self.check_editor.expected_value)
                                        .show_ui(ui, |ui| {
                                            for scheme in &["high_performance", "balanced", "power_saver"] {
                                                ui.selectable_value(
                                                    &mut self.check_editor.expected_value,
                                                    scheme.to_string(),
                                                    *scheme
                                                );
                                            }
                                        });
                                    ui.end_row();
                                }
                                CheckType::PowerMode => {
                                    ui.label("Expected:");
                                    egui::ComboBox::from_id_source("power_mode_combo")
                                        .selected_text(&self.check_editor.expected_value)
                                        .show_ui(ui, |ui| {
                                            for mode in &["best_performance", "better_performance", "balanced", "better_battery"] {
                                                ui.selectable_value(
                                                    &mut self.check_editor.expected_value,
                                                    mode.to_string(),
                                                    *mode
                                                );
                                            }
                                        });
                                    ui.end_row();
                                }
                                CheckType::RegistryDword | CheckType::RegistryString => {
                                    ui.label("Registry Path:");
                                    ui.text_edit_singleline(&mut self.check_editor.registry_path);
                                    ui.end_row();

                                    ui.label("Registry Key:");
                                    ui.text_edit_singleline(&mut self.check_editor.registry_key);
                                    ui.end_row();

                                    ui.label("Expected Value:");
                                    ui.text_edit_singleline(&mut self.check_editor.expected_value);
                                    ui.end_row();
                                }
                                CheckType::ProcessAbsent | CheckType::ProcessPresent => {
                                    ui.label("Process Name:");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.check_editor.process_name)
                                            .hint_text("e.g., chrome.exe")
                                    );
                                    ui.end_row();
                                }
                            }
                        });

                    ui.add_space(AppStyle::SPACING_MD);
                    ui.separator();
                    ui.add_space(AppStyle::SPACING_SM);

                    // Buttons
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.check_editor.visible = false;
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let is_valid = !self.check_editor.id.is_empty()
                                && !self.check_editor.name.is_empty();

                            if ui.add_enabled(
                                is_valid,
                                egui::Button::new(
                                    egui::RichText::new(
                                        if self.check_editor.editing_id.is_some() { "Save" } else { "Add" }
                                    ).color(egui::Color32::WHITE)
                                ).fill(AppStyle::COLOR_PRIMARY)
                            ).clicked() {
                                let check = self.build_check_from_editor();
                                if self.check_editor.editing_id.is_some() {
                                    self.app_state.update_check(check);
                                    self.status_message = Some("✓ Check updated".to_string());
                                } else {
                                    self.app_state.add_check(check);
                                    self.status_message = Some("✓ Check added".to_string());
                                }
                                self.check_editor.visible = false;
                            }
                        });
                    });
                });
        }

        // Check Library Popup Window
        if self.library_popup.visible {
            let existing_ids = self.get_existing_check_ids();
            let library = get_library();
            let mut check_to_add: Option<LibraryCheck> = None;

            egui::Window::new("Check Library")
                .collapsible(false)
                .resizable(true)
                .default_size([500.0, 500.0])
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.set_min_width(450.0);
                    ui.set_min_height(400.0);

                    // Search bar
                    ui.horizontal(|ui| {
                        ui.label("Search:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.library_popup.search_query)
                                .hint_text("Filter checks...")
                                .desired_width(350.0)
                        );
                        if ui.small_button("Clear").clicked() {
                            self.library_popup.search_query.clear();
                        }
                    });

                    ui.add_space(AppStyle::SPACING_SM);
                    ui.separator();
                    ui.add_space(AppStyle::SPACING_SM);

                    // Category list with collapsible sections
                    egui::ScrollArea::vertical()
                        .max_height(350.0)
                        .show(ui, |ui| {
                            let search_lower = self.library_popup.search_query.to_lowercase();

                            for category in CATEGORIES {
                                // Filter checks for this category
                                let category_checks: Vec<&LibraryCheck> = library
                                    .iter()
                                    .filter(|c| c.category == *category)
                                    .filter(|c| {
                                        if search_lower.is_empty() {
                                            true
                                        } else {
                                            c.name.to_lowercase().contains(&search_lower)
                                                || c.description.to_lowercase().contains(&search_lower)
                                        }
                                    })
                                    .collect();

                                // Skip empty categories (due to search filter)
                                if category_checks.is_empty() {
                                    continue;
                                }

                                // Category header (collapsible)
                                let is_expanded = self.library_popup.expanded_categories.contains(*category);
                                let header_text = if is_expanded {
                                    format!("▼ {} ({})", category, category_checks.len())
                                } else {
                                    format!("▶ {} ({})", category, category_checks.len())
                                };

                                if ui.add(
                                    egui::Button::new(
                                        egui::RichText::new(&header_text)
                                            .color(AppStyle::COLOR_TEXT_PRIMARY)
                                            .strong()
                                    )
                                    .frame(false)
                                ).clicked() {
                                    if is_expanded {
                                        self.library_popup.expanded_categories.remove(*category);
                                    } else {
                                        self.library_popup.expanded_categories.insert(category.to_string());
                                    }
                                }

                                // Show checks if expanded
                                if is_expanded {
                                    ui.indent(format!("category_{}", category), |ui| {
                                        for check in category_checks {
                                            let already_added = existing_ids.contains(check.id);

                                            ui.horizontal(|ui| {
                                                // Laptop-only indicator
                                                if check.laptop_only {
                                                    ui.label(
                                                        egui::RichText::new("💻")
                                                            .size(AppStyle::FONT_SIZE_SMALL)
                                                    ).on_hover_text("Laptop-specific check");
                                                }

                                                // Check name
                                                ui.label(
                                                    egui::RichText::new(check.name)
                                                        .color(if already_added {
                                                            AppStyle::COLOR_TEXT_MUTED
                                                        } else {
                                                            AppStyle::COLOR_TEXT_PRIMARY
                                                        })
                                                );

                                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                    if already_added {
                                                        ui.label(
                                                            egui::RichText::new("Added")
                                                                .color(AppStyle::COLOR_TEXT_MUTED)
                                                                .size(AppStyle::FONT_SIZE_SMALL)
                                                        );
                                                    } else if ui.add(
                                                        egui::Button::new(
                                                            egui::RichText::new("+ Add")
                                                                .color(egui::Color32::WHITE)
                                                        ).fill(AppStyle::COLOR_SUCCESS)
                                                    ).clicked() {
                                                        check_to_add = Some(check.clone());
                                                    }
                                                });
                                            });

                                            // Description on hover or below
                                            ui.horizontal(|ui| {
                                                ui.add_space(20.0);
                                                ui.label(
                                                    egui::RichText::new(check.description)
                                                        .size(AppStyle::FONT_SIZE_SMALL)
                                                        .color(AppStyle::COLOR_TEXT_SECONDARY)
                                                        .italics()
                                                );
                                            });

                                            ui.add_space(AppStyle::SPACING_XS);
                                        }
                                    });
                                }

                                ui.add_space(AppStyle::SPACING_SM);
                            }
                        });

                    ui.add_space(AppStyle::SPACING_SM);
                    ui.separator();
                    ui.add_space(AppStyle::SPACING_SM);

                    // Close button
                    ui.horizontal(|ui| {
                        if ui.button("Close").clicked() {
                            self.library_popup.visible = false;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new("Sources: GamersNexus, Tom's Hardware, LTT Labs, Back2Gaming")
                                    .size(AppStyle::FONT_SIZE_XS)
                                    .color(AppStyle::COLOR_TEXT_MUTED)
                            );
                        });
                    });
                });

            // Handle deferred add action
            if let Some(check) = check_to_add {
                self.add_from_library(&check);
            }
        }

        // Request repaint every second to keep status updated
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }
}
