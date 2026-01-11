use egui::Color32;

/// Professional dark theme design system for Bench Checklist
/// Provides consistent colors, spacing, and typography
pub struct AppStyle;

#[allow(dead_code)]
impl AppStyle {
    // ===== Dark Theme Color Palette =====

    // Backgrounds
    /// Main window background - Deep dark
    pub const COLOR_BG_WINDOW: Color32 = Color32::from_rgb(18, 18, 22); // #121216
    /// Card/section background - Slightly elevated
    pub const COLOR_BG_CARD: Color32 = Color32::from_rgb(28, 28, 35); // #1C1C23
    /// Elevated/hover background
    pub const COLOR_BG_ELEVATED: Color32 = Color32::from_rgb(38, 38, 48); // #262630
    /// Text input background
    pub const COLOR_BG_INPUT: Color32 = Color32::from_rgb(22, 22, 28); // #16161C

    // Accent Colors (Blue theme)
    /// Primary action color - Blue for interactive elements
    pub const COLOR_PRIMARY: Color32 = Color32::from_rgb(59, 130, 246); // #3B82F6
    /// Primary hover state
    pub const COLOR_PRIMARY_HOVER: Color32 = Color32::from_rgb(96, 165, 250); // #60A5FA
    /// Primary dark/pressed state
    pub const COLOR_PRIMARY_DARK: Color32 = Color32::from_rgb(37, 99, 235); // #2563EB

    // Status Colors
    /// Success indicator - Green for passed checks
    pub const COLOR_SUCCESS: Color32 = Color32::from_rgb(34, 197, 94); // #22C55E
    /// Warning indicator - Amber for partial issues
    pub const COLOR_WARNING: Color32 = Color32::from_rgb(251, 191, 36); // #FBBF24
    /// Error indicator - Red for failed checks
    pub const COLOR_ERROR: Color32 = Color32::from_rgb(239, 68, 68); // #EF4444

    // Text Colors
    /// Primary text - High contrast white
    pub const COLOR_TEXT_PRIMARY: Color32 = Color32::from_rgb(248, 250, 252); // #F8FAFC
    /// Secondary text - Muted for descriptions
    pub const COLOR_TEXT_SECONDARY: Color32 = Color32::from_rgb(148, 163, 184); // #94A3B8
    /// Muted text - Low emphasis
    pub const COLOR_TEXT_MUTED: Color32 = Color32::from_rgb(100, 116, 139); // #64748B

    // Borders
    /// Default border color
    pub const COLOR_BORDER: Color32 = Color32::from_rgb(51, 51, 64); // #333340
    /// Hover border color
    pub const COLOR_BORDER_HOVER: Color32 = Color32::from_rgb(71, 71, 89); // #474759

    // ===== Spacing (8px Grid System) =====

    /// Extra small spacing - 4px
    pub const SPACING_XS: f32 = 4.0;
    /// Small spacing - 8px
    pub const SPACING_SM: f32 = 8.0;
    /// Medium spacing - 12px
    pub const SPACING_MD: f32 = 12.0;
    /// Large spacing - 16px
    pub const SPACING_LG: f32 = 16.0;
    /// Extra large spacing - 24px
    pub const SPACING_XL: f32 = 24.0;
    /// 2X large spacing - 32px
    pub const SPACING_2XL: f32 = 32.0;

    // ===== Corner Radius =====

    /// Small radius - 4px for buttons, inputs
    pub const RADIUS_SM: f32 = 4.0;
    /// Medium radius - 8px for cards
    pub const RADIUS_MD: f32 = 8.0;
    /// Large radius - 12px for modals
    pub const RADIUS_LG: f32 = 12.0;

    // ===== Typography =====

    /// Extra small font size
    pub const FONT_SIZE_XS: f32 = 11.0;
    /// Small font size
    pub const FONT_SIZE_SM: f32 = 13.0;
    /// Medium/body font size
    pub const FONT_SIZE_MD: f32 = 14.0;
    /// Large font size
    pub const FONT_SIZE_LG: f32 = 16.0;
    /// Extra large font size
    pub const FONT_SIZE_XL: f32 = 20.0;
    /// 2X large font size for titles
    pub const FONT_SIZE_2XL: f32 = 24.0;

    // Legacy aliases for compatibility
    pub const FONT_SIZE_HEADING: f32 = Self::FONT_SIZE_XL;
    pub const FONT_SIZE_BODY: f32 = Self::FONT_SIZE_MD;
    pub const FONT_SIZE_SMALL: f32 = Self::FONT_SIZE_SM;

    // ===== Component Sizes =====

    /// Minimum touch target height for accessibility
    pub const MIN_TOUCH_TARGET: f32 = 44.0;
    /// Standard button height
    pub const BUTTON_HEIGHT: f32 = 36.0;

    // ===== Helper Methods =====

    /// Apply dark theme visuals to egui context
    pub fn apply_dark_theme(ctx: &egui::Context) {
        let mut visuals = egui::Visuals::dark();

        // Window and panel backgrounds
        visuals.window_fill = Self::COLOR_BG_WINDOW;
        visuals.panel_fill = Self::COLOR_BG_WINDOW;
        visuals.extreme_bg_color = Self::COLOR_BG_INPUT;
        visuals.faint_bg_color = Self::COLOR_BG_CARD;

        // Widget colors
        visuals.widgets.noninteractive.bg_fill = Self::COLOR_BG_CARD;
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, Self::COLOR_TEXT_SECONDARY);
        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, Self::COLOR_BORDER);

        visuals.widgets.inactive.bg_fill = Self::COLOR_BG_ELEVATED;
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, Self::COLOR_TEXT_PRIMARY);
        visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, Self::COLOR_BORDER);

        visuals.widgets.hovered.bg_fill = Self::COLOR_BG_ELEVATED;
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, Self::COLOR_TEXT_PRIMARY);
        visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, Self::COLOR_PRIMARY);

        visuals.widgets.active.bg_fill = Self::COLOR_PRIMARY;
        visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, Self::COLOR_TEXT_PRIMARY);
        visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, Self::COLOR_PRIMARY);

        visuals.widgets.open.bg_fill = Self::COLOR_BG_ELEVATED;
        visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0, Self::COLOR_TEXT_PRIMARY);
        visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0, Self::COLOR_PRIMARY);

        // Selection
        visuals.selection.bg_fill = Self::COLOR_PRIMARY.gamma_multiply(0.3);
        visuals.selection.stroke = egui::Stroke::new(1.0, Self::COLOR_PRIMARY);

        // Window styling
        visuals.window_rounding = egui::Rounding::same(Self::RADIUS_LG);
        visuals.window_shadow = egui::epaint::Shadow {
            offset: egui::vec2(0.0, 4.0),
            blur: 16.0,
            spread: 0.0,
            color: Color32::from_black_alpha(80),
        };
        visuals.window_stroke = egui::Stroke::new(1.0, Self::COLOR_BORDER);

        // Popup styling
        visuals.popup_shadow = egui::epaint::Shadow {
            offset: egui::vec2(0.0, 2.0),
            blur: 8.0,
            spread: 0.0,
            color: Color32::from_black_alpha(60),
        };

        ctx.set_visuals(visuals);
    }

    /// Create a card-style frame with dark background and border
    pub fn card_frame() -> egui::Frame {
        egui::Frame::none()
            .fill(Self::COLOR_BG_CARD)
            .stroke(egui::Stroke::new(1.0, Self::COLOR_BORDER))
            .rounding(Self::RADIUS_MD)
            .inner_margin(Self::SPACING_LG)
            .outer_margin(egui::Margin::symmetric(0.0, Self::SPACING_SM))
    }

    /// Create a section header frame (no background, just spacing)
    pub fn section_frame() -> egui::Frame {
        egui::Frame::none()
            .inner_margin(egui::Margin {
                left: 0.0,
                right: 0.0,
                top: Self::SPACING_SM,
                bottom: Self::SPACING_XS,
            })
    }

    /// Get status color based on check result
    pub fn status_color(passed: bool) -> Color32 {
        if passed {
            Self::COLOR_SUCCESS
        } else {
            Self::COLOR_ERROR
        }
    }

    /// Get status icon based on check result
    pub fn status_icon(passed: bool) -> &'static str {
        if passed { "✓" } else { "✗" }
    }

    /// Create a primary button style
    pub fn primary_button() -> egui::Button<'static> {
        egui::Button::new("")
            .fill(Self::COLOR_PRIMARY)
            .min_size(egui::vec2(0.0, Self::BUTTON_HEIGHT))
    }

    /// Create a secondary button style (outline)
    pub fn secondary_button() -> egui::Button<'static> {
        egui::Button::new("")
            .fill(Self::COLOR_BG_ELEVATED)
            .stroke(egui::Stroke::new(1.0, Self::COLOR_BORDER))
            .min_size(egui::vec2(0.0, Self::BUTTON_HEIGHT))
    }

    /// Create a danger button style (for delete actions)
    pub fn danger_button() -> egui::Button<'static> {
        egui::Button::new("")
            .fill(Self::COLOR_ERROR)
            .min_size(egui::vec2(0.0, Self::BUTTON_HEIGHT))
    }
}
