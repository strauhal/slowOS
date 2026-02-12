//! Slow Computer theme — e-ink optimized
//!
//! Pure black and white. No grays. 1px black outlines.
//! IBM Plex Sans as the system font.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, Rounding, Stroke, Style, TextStyle, Visuals};

/// Only two colors exist on this machine.
pub struct SlowColors;

impl SlowColors {
    pub const WHITE: Color32 = Color32::from_rgb(255, 255, 255);
    pub const BLACK: Color32 = Color32::from_rgb(0, 0, 0);
}

/// Theme configuration for slow computer apps
pub struct SlowTheme {
    pub font_size_body: f32,
    pub font_size_heading: f32,
    pub font_size_small: f32,
    pub window_padding: f32,
    pub item_spacing: f32,
}

impl Default for SlowTheme {
    fn default() -> Self {
        Self {
            font_size_body: 14.0,
            font_size_heading: 22.0,
            font_size_small: 11.0,
            window_padding: 8.0,
            item_spacing: 4.0,
        }
    }
}

impl SlowTheme {
    /// Apply the slow computer theme to an egui context
    pub fn apply(&self, ctx: &egui::Context) {
        // --- load fonts ---
        // IBM Plex Sans as primary proportional font, JetBrains Mono as monospace,
        // Noto Sans CJK as fallback for Chinese, Japanese, Korean, Greek, Cyrillic, etc.
        let mut fonts = FontDefinitions::default();
        fonts.font_data.insert(
            "IBMPlexSans".to_owned(),
            FontData::from_static(include_bytes!("../fonts/IBMPlexSans-Text.otf")),
        );
        fonts.font_data.insert(
            "JetBrainsMono".to_owned(),
            FontData::from_static(include_bytes!("../fonts/JetBrainsMono-Regular.ttf")),
        );
        fonts.font_data.insert(
            "NotoSansCJK".to_owned(),
            FontData::from_static(include_bytes!("../fonts/NotoSansCJK-Subset.otf")),
        );
        // Proportional: IBM Plex Sans with CJK fallback
        fonts.families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "IBMPlexSans".to_owned());
        fonts.families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(1, "NotoSansCJK".to_owned());
        // Monospace: JetBrains Mono with CJK fallback
        fonts.families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "JetBrainsMono".to_owned());
        fonts.families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(1, "NotoSansCJK".to_owned());
        ctx.set_fonts(fonts);

        // --- style ---
        let mut style = Style::default();

        style.text_styles = [
            (TextStyle::Small, FontId::new(self.font_size_small, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(self.font_size_body, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(self.font_size_body, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(self.font_size_heading, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(self.font_size_body, FontFamily::Proportional)),
        ]
        .into();

        // --- visuals: pure black & white ---
        let mut visuals = Visuals::light();

        visuals.window_fill = SlowColors::WHITE;
        visuals.panel_fill = SlowColors::WHITE;
        visuals.faint_bg_color = SlowColors::WHITE;
        visuals.extreme_bg_color = SlowColors::WHITE;

        visuals.window_rounding = Rounding::ZERO;
        visuals.menu_rounding = Rounding::ZERO;

        visuals.window_stroke = Stroke::new(1.0, SlowColors::BLACK);

        let bw = |ws: &mut egui::style::WidgetVisuals| {
            ws.bg_fill = SlowColors::WHITE;
            ws.bg_stroke = Stroke::new(1.0, SlowColors::BLACK);
            ws.fg_stroke = Stroke::new(1.0, SlowColors::BLACK);
            ws.rounding = Rounding::ZERO;
        };
        bw(&mut visuals.widgets.noninteractive);
        bw(&mut visuals.widgets.inactive);
        bw(&mut visuals.widgets.hovered);
        bw(&mut visuals.widgets.active);
        bw(&mut visuals.widgets.open);

        // selection: semi-transparent so dither overlay works
        visuals.selection.bg_fill = Color32::from_rgba_premultiplied(0, 0, 0, 80);
        visuals.selection.stroke = Stroke::new(1.0, SlowColors::BLACK);

        style.visuals = visuals;

        style.spacing.window_margin = egui::Margin::same(self.window_padding);
        style.spacing.item_spacing = egui::vec2(self.item_spacing, self.item_spacing);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);

        ctx.set_style(style);
    }

    /// Window frame: white fill, 1px black outline
    pub fn window_frame() -> egui::Frame {
        egui::Frame::none()
            .fill(SlowColors::WHITE)
            .stroke(Stroke::new(1.0, SlowColors::BLACK))
            .inner_margin(egui::Margin::same(1.0))
    }

    /// Title bar: white fill, 1px black bottom border
    pub fn title_bar_frame() -> egui::Frame {
        egui::Frame::none()
            .fill(SlowColors::WHITE)
            .stroke(Stroke::new(1.0, SlowColors::BLACK))
            .inner_margin(egui::Margin::symmetric(8.0, 4.0))
    }
}

/// Menu bar styling helper
pub fn menu_bar(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::none()
        .fill(SlowColors::WHITE)
        .stroke(Stroke::new(1.0, SlowColors::BLACK))
        .inner_margin(egui::Margin::symmetric(4.0, 2.0))
        .show(ui, |ui| {
            ui.horizontal(add_contents);
        });
}

/// Consume problematic key events to prevent unwanted egui behaviors.
/// Call this at the start of your app's update() function.
/// - Tab: prevents menu focus navigation
/// - Cmd+/Cmd-: prevents zoom scaling
pub fn consume_special_keys(ctx: &egui::Context) {
    // The core problem: egui's begin_frame() processes Tab BEFORE we get control.
    // When a focused widget doesn't have a Tab filter, pressing Tab sets
    // focus_direction = Next, causing focus to move to the next widget.
    //
    // Solution: Create a "focus trap" widget that ALWAYS has a Tab filter set.
    // The filter must be set on every frame so it's active for the NEXT frame's
    // begin_frame(). This prevents Tab from ever triggering focus navigation.

    ctx.input_mut(|i| {
        i.events.retain(|e| match e {
            egui::Event::Key { key: egui::Key::Tab, .. } => false,
            egui::Event::Text(text) if text.contains('\t') => false,
            egui::Event::Key { key, modifiers, .. }
                if modifiers.command && matches!(key, egui::Key::Plus | egui::Key::Minus | egui::Key::Equals) => false,
            _ => true,
        });
    });
}
