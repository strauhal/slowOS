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
        // IBM Plex Sans family: regular, bold, italic, bold-italic
        // JetBrains Mono as monospace, Noto Sans CJK as CJK fallback
        let mut fonts = FontDefinitions::default();
        fonts.font_data.insert(
            "IBMPlexSans".to_owned(),
            FontData::from_static(include_bytes!("../fonts/IBMPlexSans-Text.otf")),
        );
        fonts.font_data.insert(
            "IBMPlexSansBold".to_owned(),
            FontData::from_static(include_bytes!("../fonts/IBMPlexSans-Bold.otf")),
        );
        fonts.font_data.insert(
            "IBMPlexSansItalic".to_owned(),
            FontData::from_static(include_bytes!("../fonts/IBMPlexSans-Italic.otf")),
        );
        fonts.font_data.insert(
            "IBMPlexSansBoldItalic".to_owned(),
            FontData::from_static(include_bytes!("../fonts/IBMPlexSans-BoldItalic.otf")),
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
        // Named families for bold, italic, bold-italic
        fonts.families
            .entry(FontFamily::Name("Bold".into()))
            .or_default()
            .extend(["IBMPlexSansBold".to_owned(), "NotoSansCJK".to_owned()]);
        fonts.families
            .entry(FontFamily::Name("Italic".into()))
            .or_default()
            .extend(["IBMPlexSansItalic".to_owned(), "NotoSansCJK".to_owned()]);
        fonts.families
            .entry(FontFamily::Name("BoldItalic".into()))
            .or_default()
            .extend(["IBMPlexSansBoldItalic".to_owned(), "NotoSansCJK".to_owned()]);
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

        // Disable smooth shadows (we draw dithered shadows manually)
        visuals.window_shadow = egui::epaint::Shadow::NONE;
        visuals.popup_shadow = egui::epaint::Shadow::NONE;

        // selection: pure white background — dithered overlays are drawn
        // separately via slowcore::dither so no alpha blending is needed.
        // Using alpha here would produce gray on the GPU before dithering runs.
        visuals.selection.bg_fill = SlowColors::WHITE;
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
/// - Tab: prevents menu focus navigation and focus cycling
/// - Cmd+/Cmd-: prevents zoom scaling
pub fn consume_special_keys(ctx: &egui::Context) {
    consume_special_keys_with_tab(ctx, 0);
}

/// Consume Tab and Cmd+/- key events.
/// Tab can optionally be replaced with spaces in text editors.
///
/// Note: egui processes Tab in begin_frame() to set focus_direction, which
/// causes focus to cycle between widgets. Since begin_frame() runs before
/// update(), we can't prevent it from setting focus_direction. Instead, we:
/// 1. Strip Tab events so no widget detects Tab being pressed
/// 2. Re-request focus on the currently focused widget, so any Tab-caused
///    focus change is reverted next frame
pub fn consume_special_keys_with_tab(ctx: &egui::Context, tab_spaces: usize) {
    // Detect Tab press before stripping events
    let tab_pressed = ctx.input(|i| {
        i.events.iter().any(|e| matches!(e,
            egui::Event::Key { key: egui::Key::Tab, pressed: true, .. }
        ))
    });

    // Save current focus so we can restore it after Tab cycling
    let focused_before = if tab_pressed {
        ctx.memory(|mem| mem.focused())
    } else {
        None
    };

    ctx.input_mut(|i| {
        let spaces: String = " ".repeat(tab_spaces);
        let mut new_events = Vec::new();
        for event in i.events.iter() {
            match event {
                // Strip Tab Key events entirely
                egui::Event::Key { key: egui::Key::Tab, .. } => {}
                // Replace tab characters with spaces in text input, or strip
                egui::Event::Text(text) if text.contains('\t') => {
                    if tab_spaces > 0 {
                        new_events.push(egui::Event::Text(text.replace('\t', &spaces)));
                    }
                }
                // Strip zoom keys
                egui::Event::Key { key, modifiers, .. }
                    if modifiers.command && matches!(key, egui::Key::Plus | egui::Key::Minus | egui::Key::Equals) => {}
                _ => { new_events.push(event.clone()); }
            }
        }
        i.events = new_events;
    });

    // Undo Tab-based focus cycling: re-request focus on whatever was focused
    // before Tab was pressed. This ensures focus doesn't jump to menu buttons
    // or other widgets when Tab is pressed.
    if tab_pressed {
        if let Some(id) = focused_before {
            ctx.memory_mut(|mem| mem.request_focus(id));
        } else {
            // Nothing was focused; surrender any focus that Tab cycling gave
            if let Some(id) = ctx.memory(|mem| mem.focused()) {
                ctx.memory_mut(|mem| mem.surrender_focus(id));
            }
        }
    }
}
