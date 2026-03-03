//! Custom widgets — pure black and white, dithered overlays
//!
//! v0.2.3: Sizes derived from type scale (Scale::*), not grid multiples.
//! Chrome widgets (buttons, controls) use Scale::UI (13px).
//! List rows are Scale::SMALL (11px) + breathing.

use egui::{Response, Ui, Widget};
use crate::theme::{Grid, Scale, SlowColors, BORDER};
use crate::dither;

/// Action returned by window control buttons.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowAction { None, Close, Minimize }

/// Close and minimize buttons for the per-app menu bar.
///
/// Button size derives from UI font + button_padding so it tracks the
/// theme's chrome density — not independently hardcoded.
pub fn window_control_buttons(ui: &mut Ui) -> WindowAction {
    let h      = ui.text_style_height(&egui::TextStyle::Button) + 2.0 * ui.spacing().button_padding.y;
    let btn_sz = egui::vec2(h, h);
    let stroke = egui::Stroke::new(BORDER, SlowColors::BLACK);
    let mut action = WindowAction::None;

    // ── Close [×] ─────────────────────────────────────────────────────────
    let (cr, close_resp) = ui.allocate_exact_size(btn_sz, egui::Sense::click());
    if ui.is_rect_visible(cr) {
        let p = ui.painter();
        p.rect_filled(cr, 0.0, SlowColors::WHITE);
        p.rect_stroke(cr, 0.0, stroke);
        if close_resp.hovered() { dither::draw_dither_hover(p, cr); }
        let m = (h * 0.28).round();
        p.line_segment([cr.left_top()  + egui::vec2(m, m),  cr.right_bottom() - egui::vec2(m, m)], stroke);
        p.line_segment([cr.right_top() + egui::vec2(-m, m), cr.left_bottom()  + egui::vec2(m, -m)], stroke);
    }
    if close_resp.clicked() { action = WindowAction::Close; }

    ui.add_space(Grid::XS - 2.0);

    // ── Minimize [–] ──────────────────────────────────────────────────────
    let (mr, min_resp) = ui.allocate_exact_size(btn_sz, egui::Sense::click());
    if ui.is_rect_visible(mr) {
        let p = ui.painter();
        p.rect_filled(mr, 0.0, SlowColors::WHITE);
        p.rect_stroke(mr, 0.0, stroke);
        if min_resp.hovered() { dither::draw_dither_hover(p, mr); }
        let m = (h * 0.28).round();
        p.line_segment(
            [egui::pos2(mr.left() + m, mr.center().y), egui::pos2(mr.right() - m, mr.center().y)],
            stroke,
        );
    }
    if min_resp.clicked() { action = WindowAction::Minimize; }

    // Separator: a small gap then a 1px vertical line
    ui.add_space(Grid::SM - 1.0);
    let (sr, _) = ui.allocate_exact_size(egui::vec2(1.0, h), egui::Sense::hover());
    if ui.is_rect_visible(sr) {
        ui.painter().vline(sr.center().x, sr.y_range(), stroke);
    }
    ui.add_space(Grid::SM - 1.0);

    action
}

// ─── SlowButton ──────────────────────────────────────────────────────────────

/// A labelled button: white bg, 1px outline, dithered on press/select.
///
/// Uses Scale::UI font so it reads as chrome, not as body content.
pub struct SlowButton<'a> {
    text:     &'a str,
    selected: bool,
}

impl<'a> SlowButton<'a> {
    pub fn new(text: &'a str) -> Self { Self { text, selected: false } }
    pub fn selected(mut self, v: bool) -> Self { self.selected = v; self }
}

impl<'a> Widget for SlowButton<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let font   = egui::FontId::proportional(Scale::UI);
        let pad_x  = ui.spacing().button_padding.x;
        let pad_y  = ui.spacing().button_padding.y;
        let text_w = ui.fonts(|f| f.layout_no_wrap(self.text.into(), font.clone(), SlowColors::BLACK).size().x);
        let height = Scale::UI + 2.0 * pad_y;
        let desired = egui::vec2(text_w + pad_x * 2.0, height);
        let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());

        if ui.is_rect_visible(rect) {
            let p = ui.painter();
            p.rect_filled(rect, 0.0, SlowColors::WHITE);
            p.rect_stroke(rect, 0.0, egui::Stroke::new(BORDER, SlowColors::BLACK));
            let active = resp.is_pointer_button_down_on() || self.selected;
            if active             { dither::draw_dither_selection(p, rect); }
            else if resp.hovered(){ dither::draw_dither_hover(p, rect); }
            p.text(rect.center(), egui::Align2::CENTER_CENTER, self.text, font,
                if active { SlowColors::WHITE } else { SlowColors::BLACK });
        }
        resp
    }
}

// ─── Toolbar separator ───────────────────────────────────────────────────────

/// 1px vertical separator for toolbars. Allocates a narrow sliver.
pub fn toolbar_separator(ui: &mut Ui) {
    let h = ui.text_style_height(&egui::TextStyle::Button) + 2.0 * ui.spacing().button_padding.y;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(Grid::SM, h), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().vline(rect.center().x, rect.y_range(), egui::Stroke::new(BORDER, SlowColors::BLACK));
    }
}

// ─── Status bar ──────────────────────────────────────────────────────────────

/// Inline status label at Scale::SMALL.
pub fn status_bar(ui: &mut Ui, text: &str) {
    egui::Frame::none()
        .fill(SlowColors::WHITE)
        .stroke(egui::Stroke::new(BORDER, SlowColors::BLACK))
        .inner_margin(egui::Margin::symmetric(Grid::SM, 2.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(Scale::SMALL));
        });
}

// ─── FileListItem ─────────────────────────────────────────────────────────────

/// File list row. Height = Scale::SMALL + breathing, not a grid multiple.
///
/// 11px text + 4.5px top + 4.5px bottom = 20px row.
/// Compact but touchable; matches the density of the rest of the chrome.
pub struct FileListItem<'a> {
    name:         &'a str,
    is_directory: bool,
    selected:     bool,
}

impl<'a> FileListItem<'a> {
    pub fn new(name: &'a str, is_dir: bool) -> Self {
        Self { name, is_directory: is_dir, selected: false }
    }
    pub fn selected(mut self, v: bool) -> Self { self.selected = v; self }
}

impl<'a> Widget for FileListItem<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        // Row height: SMALL font + symmetric padding → 20px
        let height = Scale::SMALL + 9.0;
        let (rect, resp) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), height),
            egui::Sense::click(),
        );

        if ui.is_rect_visible(rect) {
            let p = ui.painter();
            p.rect_filled(rect, 0.0, SlowColors::WHITE);

            let fg = if self.selected {
                dither::draw_dither_selection(p, rect);
                SlowColors::WHITE
            } else if resp.hovered() {
                dither::draw_dither_hover(p, rect);
                SlowColors::BLACK
            } else {
                SlowColors::BLACK
            };

            let icon = if self.is_directory { "📁" } else { "📄" };
            p.text(
                egui::pos2(rect.min.x + 14.0, rect.center().y),
                egui::Align2::CENTER_CENTER,
                icon,
                egui::FontId::proportional(Scale::SMALL),
                fg,
            );
            p.text(
                egui::pos2(rect.min.x + Grid::LG + 2.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                self.name,
                egui::FontId::proportional(Scale::SMALL),
                fg,
            );
        }
        resp
    }
}
