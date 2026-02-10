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

        visuals.widgets.noninteractive.bg_fill = SlowColors::WHITE;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, SlowColors::BLACK);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, SlowColors::BLACK);
        visuals.widgets.noninteractive.rounding = Rounding::ZERO;

        visuals.widgets.inactive.bg_fill = SlowColors::WHITE;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, SlowColors::BLACK);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, SlowColors::BLACK);
        visuals.widgets.inactive.rounding = Rounding::ZERO;

        visuals.widgets.hovered.bg_fill = SlowColors::WHITE;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, SlowColors::BLACK);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, SlowColors::BLACK);
        visuals.widgets.hovered.rounding = Rounding::ZERO;

        visuals.widgets.active.bg_fill = SlowColors::WHITE;
        visuals.widgets.active.bg_stroke = Stroke::new(1.0, SlowColors::BLACK);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, SlowColors::BLACK);
        visuals.widgets.active.rounding = Rounding::ZERO;

        visuals.widgets.open.bg_fill = SlowColors::WHITE;
        visuals.widgets.open.bg_stroke = Stroke::new(1.0, SlowColors::BLACK);
        visuals.widgets.open.fg_stroke = Stroke::new(1.0, SlowColors::BLACK);
        visuals.widgets.open.rounding = Rounding::ZERO;

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
    // It sets internal focus_direction = Next, which causes the next widget that
    // calls interested_in_focus() to receive focus.
    //
    // Solution: Create an invisible "focus trap" widget that registers interest
    // in focus. When Tab is pressed, this trap widget will receive focus instead
    // of menu buttons. We then immediately surrender that focus.

    let tab_pressed = ctx.input(|i| {
        i.events.iter().any(|e| matches!(e, egui::Event::Key { key: egui::Key::Tab, pressed: true, .. }))
    });

    ctx.input_mut(|i| {
        // Remove Tab key events to prevent further processing
        i.events.retain(|e| {
            !matches!(e, egui::Event::Key { key: egui::Key::Tab, .. })
        });

        // Also remove any Tab character from Text events
        i.events.retain(|e| {
            if let egui::Event::Text(text) = e {
                return !text.contains('\t');
            }
            true
        });

        // Remove Cmd+Plus/Minus to prevent zoom scaling
        i.events.retain(|e| {
            if let egui::Event::Key { key, modifiers, .. } = e {
                if modifiers.command && (*key == egui::Key::Plus || *key == egui::Key::Minus || *key == egui::Key::Equals) {
                    return false;
                }
            }
            true
        });
    });

    // Create invisible focus trap widget that intercepts Tab navigation.
    // The trap must be the FIRST widget to register interest in focus each frame,
    // so it captures Tab navigation before any menu buttons can.
    let trap_id = egui::Id::new("__slowcore_focus_trap__");

    ctx.memory_mut(|mem| {
        // Register the trap as interested in focus FIRST (before any menus render)
        mem.interested_in_focus(trap_id);

        // If Tab was pressed:
        // 1. If trap got focus, lock it with an event filter that captures Tab
        // 2. If another widget somehow got focus, surrender it
        if tab_pressed {
            let current_focus = mem.focused();
            if current_focus == Some(trap_id) {
                // Lock focus to trap so Tab can't move it to menus
                mem.set_focus_lock_filter(trap_id, egui::EventFilter {
                    tab: true,  // Capture Tab so it doesn't move focus
                    horizontal_arrows: false,
                    vertical_arrows: false,
                    escape: true,  // Also capture Escape
                });
            } else if let Some(focused) = current_focus {
                // Some other widget got focus - surrender it
                mem.surrender_focus(focused);
            }
        }
    });

    // After all widgets have rendered this frame, surrender trap focus
    // (This runs at start of NEXT frame, so it's always one frame behind)
    ctx.memory_mut(|mem| {
        if mem.focused() == Some(trap_id) && !tab_pressed {
            mem.surrender_focus(trap_id);
        }
    });
}

/// Consume Tab key events to prevent menu focus navigation.
/// Call this at the start of your app's update() function.
#[deprecated(note = "Use consume_special_keys instead")]
pub fn consume_tab_key(ctx: &egui::Context) {
    consume_special_keys(ctx);
}
