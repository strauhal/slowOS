//! Custom widgets — pure black and white, dithered overlays
//!
//! v0.2.3: All sizes derived from Grid/Scale tokens in theme.rs.
//! More breathing room, consistent proportions.

use egui::{Response, Ui, Widget};
use crate::theme::{Grid, Scale, SlowColors, BORDER};
use crate::dither;

/// Action returned by window control buttons.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowAction { None, Close, Minimize }

/// Draw close and minimize buttons at the left of the menu bar.
/// Returns the action the user clicked (Close, Minimize, or None).
pub fn window_control_buttons(ui: &mut Ui) -> WindowAction {
    let text_h = ui.text_style_height(&egui::TextStyle::Button);
    let pad_y  = ui.spacing().button_padding.y;
    let h      = text_h + 2.0 * pad_y;
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
        let m = (h * 0.27).round();
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
        let m = (h * 0.27).round();
        p.line_segment(
            [egui::pos2(mr.left() + m, mr.center().y), egui::pos2(mr.right() - m, mr.center().y)],
            stroke,
        );
    }
    if min_resp.clicked() { action = WindowAction::Minimize; }

    ui.add_space(Grid::SM - 2.0);

    // ── Separator ─────────────────────────────────────────────────────────
    let (sr, _) = ui.allocate_exact_size(egui::vec2(1.0, h), egui::Sense::hover());
    if ui.is_rect_visible(sr) {
        ui.painter().vline(sr.center().x, sr.y_range(), stroke);
    }

    ui.add_space(Grid::SM - 2.0);

    action
}

// ─── SlowButton ──────────────────────────────────────────────────────────────

/// A button: white bg, 1px outline. Dithered when pressed/selected.
pub struct SlowButton<'a> {
    text:     &'a str,
    selected: bool,
}

impl<'a> SlowButton<'a> {
    pub fn new(text: &'a str) -> Self { Self { text, selected: false } }
    pub fn selected(mut self, selected: bool) -> Self { self.selected = selected; self }
}

impl<'a> Widget for SlowButton<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let text_w = ui.fonts(|f| {
            f.glyph_width(&egui::FontId::proportional(Scale::BODY), ' ') * self.text.len() as f32
        });
        let desired = egui::vec2(text_w + Grid::MD * 2.0, ui.spacing().interact_size.y);
        let (rect, resp) = ui.allocate_exact_size(desired, egui::Sense::click());

        if ui.is_rect_visible(rect) {
            let p = ui.painter();
            p.rect_filled(rect, 0.0, SlowColors::WHITE);
            p.rect_stroke(rect, 0.0, egui::Stroke::new(BORDER, SlowColors::BLACK));

            let pressed = resp.is_pointer_button_down_on() || self.selected;
            if pressed      { dither::draw_dither_selection(p, rect); }
            else if resp.hovered() { dither::draw_dither_hover(p, rect); }

            p.text(
                rect.center(), egui::Align2::CENTER_CENTER,
                self.text, egui::FontId::proportional(Scale::BODY),
                if pressed { SlowColors::WHITE } else { SlowColors::BLACK },
            );
        }
        resp
    }
}

// ─── Toolbar separator ───────────────────────────────────────────────────────

/// Vertical 1px black line for use in toolbars.
pub fn toolbar_separator(ui: &mut Ui) {
    let h = ui.spacing().interact_size.y;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(Grid::SM, h), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().vline(rect.center().x, rect.y_range(), egui::Stroke::new(BORDER, SlowColors::BLACK));
    }
}

// ─── Status bar ──────────────────────────────────────────────────────────────

/// Status bar: white bg, 1px black border, small font.
pub fn status_bar(ui: &mut Ui, text: &str) {
    egui::Frame::none()
        .fill(SlowColors::WHITE)
        .stroke(egui::Stroke::new(BORDER, SlowColors::BLACK))
        .inner_margin(egui::Margin::symmetric(Grid::SM, Grid::XS - 1.0))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(Scale::SMALL));
        });
}

// ─── FileListItem ─────────────────────────────────────────────────────────────

/// File list item for open/save dialogs.
/// Selected items get a dithered overlay instead of solid black.
pub struct FileListItem<'a> {
    name:         &'a str,
    is_directory: bool,
    selected:     bool,
}

impl<'a> FileListItem<'a> {
    pub fn new(name: &'a str, is_directory: bool) -> Self {
        Self { name, is_directory, selected: false }
    }
    pub fn selected(mut self, selected: bool) -> Self { self.selected = selected; self }
}

impl<'a> Widget for FileListItem<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let height = Grid::LG;   // 24px — one grid unit of vertical breathing room
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
                egui::pos2(rect.min.x + Grid::MD, rect.center().y),
                egui::Align2::CENTER_CENTER,
                icon,
                egui::FontId::proportional(Scale::SMALL),
                fg,
            );
            p.text(
                egui::pos2(rect.min.x + Grid::LG, rect.center().y),
                egui::Align2::LEFT_CENTER,
                self.name,
                egui::FontId::proportional(Scale::SMALL + 1.0),
                fg,
            );
        }
        resp
    }
}
