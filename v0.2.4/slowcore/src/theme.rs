//! Slow Computer theme — e-ink optimized
//!
//! Pure black and white. 1px outlines. IBM Plex Sans.
//!
//! Design rationale:
//!
//! **Hierarchy through type, not size inflation.**
//! Chrome (menus, buttons, toolbars) uses a smaller type size than content.
//! This creates depth: UI recedes, content comes forward.
//!
//!   Small (11px)  — status bars, icon labels, captions
//!   UI    (13px)  — menus, buttons, toolbar chrome
//!   Body  (14px)  — document content, notes, reading
//!   Heading(20px) — dialog titles, section heads
//!   Mono  (13px)  — terminal, code (matched to UI x-height)
//!
//! **Asymmetric window padding.**
//! Content windows are 12px left/right, 8px top/bottom.
//! Horizontal breathing matters more than vertical — we read across.
//!
//! **Asymmetric item spacing.**
//! 8px between inline elements, 4px between stacked items.
//! Vertical rhythm comes from line height, not added gap.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, Rounding, Stroke, Style, TextStyle, Visuals};

// ─── Design tokens ───────────────────────────────────────────────────────────

/// The two colors.
pub struct SlowColors;
impl SlowColors {
    pub const WHITE: Color32 = Color32::from_rgb(255, 255, 255);
    pub const BLACK: Color32 = Color32::from_rgb(0, 0, 0);
}

/// Spacing grid.
pub struct Grid;
impl Grid {
    pub const XS: f32 = 4.0;   // minimal padding
    pub const SM: f32 = 8.0;   // standard gap
    pub const MD: f32 = 12.0;  // content horizontal padding
    pub const LG: f32 = 20.0;  // list row height, structural gap
    pub const XL: f32 = 24.0;  // section spacing
}

/// Type scale — each step has semantic purpose.
pub struct Scale;
impl Scale {
    /// Status bars, icon labels, file list, timestamps.
    pub const SMALL:   f32 = 11.0;
    /// Menu items, buttons, toolbar — chrome recedes behind content.
    pub const UI:      f32 = 13.0;
    /// Document body, notes, writing — where the user lives.
    pub const BODY:    f32 = 14.0;
    /// Dialog headings, section titles — distinct, not overwhelming.
    pub const HEADING: f32 = 20.0;
    /// Terminal, code — matched to UI x-height.
    pub const MONO:    f32 = 13.0;
}

pub const BORDER: f32 = 1.0;

// ─── Theme ───────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct SlowTheme;

impl SlowTheme {
    /// Load CJK font data from known system paths.
    /// Returns the font bytes if found, or None.
    pub fn load_cjk_font_data() -> Option<Vec<u8>> {
        Self::load_cjk_font()
    }

    fn load_cjk_font() -> Option<Vec<u8>> {
        let font_name = "NotoSansCJK-Subset.otf";
        let mut paths = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                paths.push(dir.join("fonts").join(font_name));
                paths.push(dir.join(font_name));
                if let Some(p) = dir.parent().and_then(|p| p.parent()) {
                    paths.push(p.join("slowcore/fonts").join(font_name));
                }
            }
        }
        paths.push(std::path::PathBuf::from("/usr/share/slowos/fonts").join(font_name));
        paths.push(std::path::PathBuf::from("/usr/share/fonts").join(font_name));
        paths.into_iter().find_map(|p| std::fs::read(&p).ok())
    }

    /// Load a font file, trying disk first so multiple processes share
    /// the kernel's page cache copy (saves ~400 KB RAM per running app
    /// versus every binary embedding its own copy). Falls back to the
    /// crate-embedded copy when the disk file is missing — this is the
    /// common case for `cargo run` during development.
    fn load_font_bytes(filename: &str, embedded: &'static [u8]) -> FontData {
        let disk_paths = [
            std::path::PathBuf::from("/usr/share/slowos/fonts").join(filename),
            std::path::PathBuf::from("/usr/local/share/slowos/fonts").join(filename),
        ];
        for p in &disk_paths {
            if let Ok(bytes) = std::fs::read(p) {
                return FontData::from_owned(bytes);
            }
        }
        FontData::from_static(embedded)
    }

    pub fn apply(&self, ctx: &egui::Context) {
        // Force 1:1 pixel mapping — e-ink needs exact pixel control.
        // Without this, HiDPI displays (Ubuntu, etc.) may scale the UI incorrectly.
        ctx.set_pixels_per_point(1.0);

        // ── fonts ─────────────────────────────────────────────────────────
        // We don't use egui's `default_fonts` feature, so FontDefinitions
        // starts empty — no wasted Hack / Ubuntu font atlases.
        let mut fonts = FontDefinitions::empty();
        fonts.font_data.insert("IBMPlexSans".into(),
            Self::load_font_bytes("IBMPlexSans-Text.otf",
                include_bytes!("../fonts/IBMPlexSans-Text.otf")));
        fonts.font_data.insert("JetBrainsMono".into(),
            Self::load_font_bytes("JetBrainsMono-Regular.ttf",
                include_bytes!("../fonts/JetBrainsMono-Regular.ttf")));
        if let Some(data) = Self::load_cjk_font() {
            fonts.font_data.insert("NotoSansCJK".into(), FontData::from_owned(data));
            for family in [FontFamily::Proportional, FontFamily::Monospace] {
                fonts.families.entry(family).or_default().insert(1, "NotoSansCJK".into());
            }
        }
        fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "IBMPlexSans".into());
        fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "JetBrainsMono".into());
        ctx.set_fonts(fonts);

        // ── style ─────────────────────────────────────────────────────────
        let mut style = Style::default();

        // Button ≠ Body: UI chrome is one step smaller than content.
        // This creates the hierarchy that makes content feel primary.
        style.text_styles = [
            (TextStyle::Small,     FontId::new(Scale::SMALL,   FontFamily::Proportional)),
            (TextStyle::Body,      FontId::new(Scale::BODY,    FontFamily::Proportional)),
            (TextStyle::Button,    FontId::new(Scale::UI,      FontFamily::Proportional)),
            (TextStyle::Heading,   FontId::new(Scale::HEADING, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(Scale::MONO,    FontFamily::Monospace)),
        ].into();

        // ── visuals ───────────────────────────────────────────────────────
        let mut v = Visuals::light();
        v.window_fill      = SlowColors::WHITE;
        v.panel_fill       = SlowColors::WHITE;
        v.faint_bg_color   = SlowColors::WHITE;
        v.extreme_bg_color = SlowColors::WHITE;
        v.window_rounding  = Rounding::ZERO;
        v.menu_rounding    = Rounding::ZERO;
        v.window_stroke    = Stroke::new(BORDER, SlowColors::BLACK);
        v.window_shadow    = egui::epaint::Shadow::NONE;
        v.popup_shadow     = egui::epaint::Shadow::NONE;

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
        // Horizontal padding > vertical: we read across, content needs
        // lateral room. Vertical rhythm comes from line height.
        style.spacing.window_margin  = egui::Margin { left: Grid::MD, right: Grid::MD, top: Grid::SM, bottom: Grid::SM };
        style.spacing.item_spacing   = egui::vec2(Grid::SM, Grid::XS);
        style.spacing.button_padding = egui::vec2(10.0, 3.0);

        ctx.set_style(style);
    }

    /// Window frame: white fill, 1px border.
    pub fn window_frame() -> egui::Frame {
        egui::Frame::none()
            .fill(SlowColors::WHITE)
            .stroke(Stroke::new(BORDER, SlowColors::BLACK))
            .inner_margin(egui::Margin::same(BORDER))
    }

    /// Title bar frame: white fill, 1px border, tight vertical padding.
    /// Used by apps that draw a centred document title in the menu bar area.
    pub fn title_bar_frame() -> egui::Frame {
        egui::Frame::none()
            .fill(SlowColors::WHITE)
            .stroke(Stroke::new(BORDER, SlowColors::BLACK))
            .inner_margin(egui::Margin::symmetric(Grid::SM, Grid::XS))
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Per-app window menu bar: white, 1px bottom border.
pub fn menu_bar<R>(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    let resp = egui::Frame::none()
        .fill(SlowColors::WHITE)
        .stroke(Stroke::new(BORDER, SlowColors::BLACK))
        .inner_margin(egui::Margin::symmetric(Grid::XS, 2.0))
        .show(ui, |ui| ui.horizontal(add_contents).inner);
    egui::InnerResponse { inner: resp.inner, response: resp.response }
}

/// Returns true when a text field has keyboard focus.
///
/// Use this to guard bare-key shortcuts so they don't fire while
/// the user is typing. Modifier-based shortcuts (Cmd+F etc.) are
/// always safe and don't need this guard.
///
/// ```
/// let typing = slowcore::theme::is_typing(ctx);
/// ctx.input(|i| {
///     if cmd && i.key_pressed(Key::O) { /* always fine */ }
///     if !typing && i.key_pressed(Key::T) { /* safe */ }
/// });
/// ```
pub fn is_typing(ctx: &egui::Context) -> bool {
    ctx.wants_keyboard_input()
}

/// Consume Cmd±/= so egui's default zoom gestures don't interfere with
/// our app-specific zoom handling. Tab is NOT consumed — egui's default
/// focus-cycling behaviour is preserved so users can tab through buttons
/// and text fields in dialogs.
pub fn consume_special_keys(ctx: &egui::Context) {
    let enter_pressed = ctx.input(|i| i.events.iter().any(|e| matches!(e,
        egui::Event::Key { key: egui::Key::Enter, pressed: true, .. }
    )));

    ctx.input_mut(|i| {
        i.events.retain(|e| !matches!(e,
            egui::Event::Key { key, modifiers, .. }
                if modifiers.command && matches!(key, egui::Key::Plus | egui::Key::Minus | egui::Key::Equals)
        ));
    });

    if enter_pressed && ctx.memory(|m| m.any_popup_open()) {
        ctx.memory_mut(|m| m.close_popup());
    }
}

/// Like `consume_special_keys`, but also strips Tab key events and
/// replaces literal `\t` characters in Text events with `tab_spaces`
/// spaces. For apps (like terminal / code editors) that want Tab to
/// mean "insert spaces" rather than "cycle focus".
pub fn consume_special_keys_with_tab(ctx: &egui::Context, tab_spaces: usize) {
    let tab_pressed = ctx.input(|i| i.events.iter().any(|e| matches!(e,
        egui::Event::Key { key: egui::Key::Tab, pressed: true, .. }
    )));
    let enter_pressed = ctx.input(|i| i.events.iter().any(|e| matches!(e,
        egui::Event::Key { key: egui::Key::Enter, pressed: true, .. }
    )));
    let focused_before = tab_pressed.then(|| ctx.memory(|m| m.focused())).flatten();

    ctx.input_mut(|i| {
        let spaces = " ".repeat(tab_spaces);
        i.events = i.events.iter().filter_map(|e| match e {
            egui::Event::Key { key: egui::Key::Tab, .. } => None,
            egui::Event::Text(t) if t.contains('\t') =>
                (tab_spaces > 0).then(|| egui::Event::Text(t.replace('\t', &spaces))),
            egui::Event::Key { key, modifiers, .. }
                if modifiers.command && matches!(key, egui::Key::Plus | egui::Key::Minus | egui::Key::Equals) => None,
            e => Some(e.clone()),
        }).collect();
    });

    if tab_pressed {
        if let Some(id) = focused_before { ctx.memory_mut(|m| m.request_focus(id)); }
        else if let Some(id) = ctx.memory(|m| m.focused()) { ctx.memory_mut(|m| m.surrender_focus(id)); }
    }
    if enter_pressed && ctx.memory(|m| m.any_popup_open()) {
        ctx.memory_mut(|m| m.close_popup());
    }
}
