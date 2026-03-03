//! Slow Computer theme — e-ink optimized
//!
//! Pure black and white. No grays. 1px black outlines.
//! IBM Plex Sans as the system font.
//!
//! v0.2.3: Redesigned for space and contrast.
//! All sizing derives from an 8px grid — every margin, padding, and gap
//! is a multiple of 8 (or 4 at minimum scale).
//! Typography uses a deliberate scale: 12 / 15 / 24.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, Rounding, Stroke, Style, TextStyle, Visuals};

// ─── Design tokens ───────────────────────────────────────────────────────────

/// The two colors.
pub struct SlowColors;
impl SlowColors {
    pub const WHITE: Color32 = Color32::from_rgb(255, 255, 255);
    pub const BLACK: Color32 = Color32::from_rgb(0, 0, 0);
}

/// 8-pixel spacing grid.
pub struct Grid;
impl Grid {
    pub const XS: f32  =  4.0;
    pub const SM: f32  =  8.0;
    pub const MD: f32  = 16.0;
    pub const LG: f32  = 24.0;
    pub const XL: f32  = 32.0;
}

/// Deliberate type scale: 12 / 15 / 24.
pub struct Scale;
impl Scale {
    pub const SMALL:   f32 = 12.0;
    pub const BODY:    f32 = 15.0;
    pub const HEADING: f32 = 24.0;
    pub const MONO:    f32 = 14.0;
}

pub const BORDER: f32 = 1.0;

// ─── Theme ───────────────────────────────────────────────────────────────────

/// Theme configuration for slow computer apps.
/// All values derive from the tokens above — do not hardcode sizes elsewhere.
pub struct SlowTheme {
    pub font_size_body:    f32,
    pub font_size_heading: f32,
    pub font_size_small:   f32,
    pub window_padding:    f32,
    pub item_spacing:      f32,
}

impl Default for SlowTheme {
    fn default() -> Self {
        Self {
            font_size_body:    Scale::BODY,
            font_size_heading: Scale::HEADING,
            font_size_small:   Scale::SMALL,
            window_padding:    Grid::MD,
            item_spacing:      Grid::SM,
        }
    }
}

impl SlowTheme {
    /// Load NotoSansCJK font from disk (searched relative to exe and standard paths).
    fn load_cjk_font() -> Option<Vec<u8>> {
        let font_name = "NotoSansCJK-Subset.otf";
        let mut search_paths = Vec::new();

        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                search_paths.push(dir.join("fonts").join(font_name));
                search_paths.push(dir.join(font_name));
                if let Some(parent) = dir.parent() {
                    if let Some(grandparent) = parent.parent() {
                        search_paths.push(grandparent.join("slowcore/fonts").join(font_name));
                    }
                }
            }
        }
        search_paths.push(std::path::PathBuf::from("/usr/share/slowos/fonts").join(font_name));
        search_paths.push(std::path::PathBuf::from("/usr/share/fonts").join(font_name));

        search_paths.into_iter().find_map(|p| std::fs::read(&p).ok())
    }

    /// Apply the slow computer theme to an egui context.
    pub fn apply(&self, ctx: &egui::Context) {
        // ── fonts ─────────────────────────────────────────────────────────
        let mut fonts = FontDefinitions::default();
        fonts.font_data.insert(
            "IBMPlexSans".to_owned(),
            FontData::from_static(include_bytes!("../fonts/IBMPlexSans-Text.otf")),
        );
        fonts.font_data.insert(
            "JetBrainsMono".to_owned(),
            FontData::from_static(include_bytes!("../fonts/JetBrainsMono-Regular.ttf")),
        );
        if let Some(cjk_data) = Self::load_cjk_font() {
            fonts.font_data.insert("NotoSansCJK".to_owned(), FontData::from_owned(cjk_data));
            for family in [FontFamily::Proportional, FontFamily::Monospace] {
                fonts.families.entry(family).or_default().insert(1, "NotoSansCJK".to_owned());
            }
        }
        fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "IBMPlexSans".to_owned());
        fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "JetBrainsMono".to_owned());
        ctx.set_fonts(fonts);

        // ── style ─────────────────────────────────────────────────────────
        let mut style = Style::default();

        style.text_styles = [
            (TextStyle::Small,     FontId::new(self.font_size_small,   FontFamily::Proportional)),
            (TextStyle::Body,      FontId::new(self.font_size_body,    FontFamily::Proportional)),
            (TextStyle::Button,    FontId::new(self.font_size_body,    FontFamily::Proportional)),
            (TextStyle::Heading,   FontId::new(self.font_size_heading, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(Scale::MONO,            FontFamily::Monospace)),
        ].into();

        // ── visuals: pure black & white ───────────────────────────────────
        let mut v = Visuals::light();

        v.window_fill      = SlowColors::WHITE;
        v.panel_fill       = SlowColors::WHITE;
        v.faint_bg_color   = SlowColors::WHITE;
        v.extreme_bg_color = SlowColors::WHITE;

        v.window_rounding = Rounding::ZERO;
        v.menu_rounding   = Rounding::ZERO;
        v.window_stroke   = Stroke::new(BORDER, SlowColors::BLACK);
        v.window_shadow   = egui::epaint::Shadow::NONE;
        v.popup_shadow    = egui::epaint::Shadow::NONE;

        let bw = |ws: &mut egui::style::WidgetVisuals| {
            ws.bg_fill   = SlowColors::WHITE;
            ws.bg_stroke = Stroke::new(BORDER, SlowColors::BLACK);
            ws.fg_stroke = Stroke::new(BORDER, SlowColors::BLACK);
            ws.rounding  = Rounding::ZERO;
        };
        bw(&mut v.widgets.noninteractive);
        bw(&mut v.widgets.inactive);
        bw(&mut v.widgets.hovered);
        bw(&mut v.widgets.active);
        bw(&mut v.widgets.open);

        v.selection.bg_fill = Color32::from_rgb(160, 160, 160);
        v.selection.stroke  = Stroke::new(BORDER, SlowColors::BLACK);

        style.visuals = v;

        // ── spacing ───────────────────────────────────────────────────────
        style.spacing.window_margin  = egui::Margin::same(self.window_padding);
        style.spacing.item_spacing   = egui::vec2(self.item_spacing, self.item_spacing);
        style.spacing.button_padding = egui::vec2(Grid::MD, Grid::XS + 2.0);

        ctx.set_style(style);
    }

    /// Window frame: white fill, 1px black outline.
    pub fn window_frame() -> egui::Frame {
        egui::Frame::none()
            .fill(SlowColors::WHITE)
            .stroke(Stroke::new(BORDER, SlowColors::BLACK))
            .inner_margin(egui::Margin::same(BORDER))
    }

    /// Title bar: white fill, 1px outline, generous horizontal padding.
    pub fn title_bar_frame() -> egui::Frame {
        egui::Frame::none()
            .fill(SlowColors::WHITE)
            .stroke(Stroke::new(BORDER, SlowColors::BLACK))
            .inner_margin(egui::Margin::symmetric(Grid::SM, Grid::XS))
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Menu bar styling helper.
pub fn menu_bar<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    let frame_resp = egui::Frame::none()
        .fill(SlowColors::WHITE)
        .stroke(Stroke::new(BORDER, SlowColors::BLACK))
        .inner_margin(egui::Margin::symmetric(Grid::XS, Grid::XS - 2.0))
        .show(ui, |ui| ui.horizontal(add_contents).inner);
    egui::InnerResponse { inner: frame_resp.inner, response: frame_resp.response }
}

/// Consume Tab and Cmd±/= key events to prevent egui default navigation/zoom.
/// Call at the start of `update()`.
pub fn consume_special_keys(ctx: &egui::Context) {
    consume_special_keys_with_tab(ctx, 0);
}

/// Consume Tab (optionally replacing with spaces) and Cmd± zoom keys.
///
/// egui's `begin_frame()` processes Tab before `update()` runs, cycling widget
/// focus. We counteract this by:
/// 1. Stripping Tab events from the event list.
/// 2. Re-requesting focus on whatever widget was focused before Tab fired.
pub fn consume_special_keys_with_tab(ctx: &egui::Context, tab_spaces: usize) {
    let tab_pressed = ctx.input(|i| {
        i.events.iter().any(|e| matches!(e,
            egui::Event::Key { key: egui::Key::Tab, pressed: true, .. }
        ))
    });

    let enter_pressed = ctx.input(|i| {
        i.events.iter().any(|e| matches!(e,
            egui::Event::Key { key: egui::Key::Enter, pressed: true, .. }
        ))
    });

    let focused_before = tab_pressed.then(|| ctx.memory(|m| m.focused())).flatten();

    ctx.input_mut(|i| {
        let spaces = " ".repeat(tab_spaces);
        i.events = i.events.iter().filter_map(|event| match event {
            egui::Event::Key { key: egui::Key::Tab, .. } => None,
            egui::Event::Text(t) if t.contains('\t') => {
                (tab_spaces > 0).then(|| egui::Event::Text(t.replace('\t', &spaces)))
            }
            egui::Event::Key { key, modifiers, .. }
                if modifiers.command
                    && matches!(key, egui::Key::Plus | egui::Key::Minus | egui::Key::Equals) =>
            {
                None
            }
            e => Some(e.clone()),
        }).collect();
    });

    if tab_pressed {
        if let Some(id) = focused_before {
            ctx.memory_mut(|m| m.request_focus(id));
        } else if let Some(id) = ctx.memory(|m| m.focused()) {
            ctx.memory_mut(|m| m.surrender_focus(id));
        }
    }

    if enter_pressed && ctx.memory(|m| m.any_popup_open()) {
        ctx.memory_mut(|m| m.close_popup());
    }
}
