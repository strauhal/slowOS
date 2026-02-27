//! Custom widgets — pure black and white, dithered overlays

use egui::{Response, Ui, Widget};
use crate::theme::SlowColors;
use crate::dither;

/// Action returned by window control buttons
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowAction {
    None,
    Close,
    Minimize,
}

/// Draw close and minimize buttons at the left of the menu bar.
/// Call this at the start of your `menu_bar` closure.
///
/// Returns the action the user clicked (Close, Minimize, or None).
pub fn window_control_buttons(ui: &mut Ui) -> WindowAction {
    let btn_size = egui::vec2(14.0, 14.0);
    let mut action = WindowAction::None;

    // Close button [X]
    let (close_rect, close_resp) = ui.allocate_exact_size(btn_size, egui::Sense::click());
    if ui.is_rect_visible(close_rect) {
        let painter = ui.painter();
        painter.rect_filled(close_rect, 0.0, SlowColors::WHITE);
        painter.rect_stroke(close_rect, 0.0, egui::Stroke::new(1.0, SlowColors::BLACK));
        if close_resp.hovered() {
            dither::draw_dither_hover(painter, close_rect);
        }
        // Draw X
        let m = 3.0;
        painter.line_segment(
            [
                close_rect.left_top() + egui::vec2(m, m),
                close_rect.right_bottom() - egui::vec2(m, m),
            ],
            egui::Stroke::new(1.0, SlowColors::BLACK),
        );
        painter.line_segment(
            [
                close_rect.right_top() + egui::vec2(-m, m),
                close_rect.left_bottom() + egui::vec2(m, -m),
            ],
            egui::Stroke::new(1.0, SlowColors::BLACK),
        );
    }
    if close_resp.clicked() {
        action = WindowAction::Close;
    }

    ui.add_space(2.0);

    // Minimize button [-]
    let (min_rect, min_resp) = ui.allocate_exact_size(btn_size, egui::Sense::click());
    if ui.is_rect_visible(min_rect) {
        let painter = ui.painter();
        painter.rect_filled(min_rect, 0.0, SlowColors::WHITE);
        painter.rect_stroke(min_rect, 0.0, egui::Stroke::new(1.0, SlowColors::BLACK));
        if min_resp.hovered() {
            dither::draw_dither_hover(painter, min_rect);
        }
        // Draw horizontal dash
        let m = 3.0;
        painter.line_segment(
            [
                egui::pos2(min_rect.left() + m, min_rect.center().y),
                egui::pos2(min_rect.right() - m, min_rect.center().y),
            ],
            egui::Stroke::new(1.0, SlowColors::BLACK),
        );
    }
    if min_resp.clicked() {
        action = WindowAction::Minimize;
    }

    ui.add_space(4.0);

    // Thin vertical separator after the buttons
    let sep_height = btn_size.y;
    let (sep_rect, _) = ui.allocate_exact_size(egui::vec2(4.0, sep_height), egui::Sense::hover());
    if ui.is_rect_visible(sep_rect) {
        ui.painter().vline(
            sep_rect.center().x,
            sep_rect.y_range(),
            egui::Stroke::new(1.0, SlowColors::BLACK),
        );
    }

    ui.add_space(4.0);

    action
}

/// A button: white bg, 1px outline. dithered when pressed/selected.
pub struct SlowButton<'a> {
    text: &'a str,
    selected: bool,
}

impl<'a> SlowButton<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text, selected: false }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl<'a> Widget for SlowButton<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        // Calculate button size based on text content
        let text_size = ui.fonts(|f| {
            f.glyph_width(&egui::FontId::proportional(14.0), ' ') * self.text.len() as f32
        });
        let padding = egui::vec2(16.0, 4.0);
        let desired_size = egui::vec2(
            text_size + padding.x * 2.0,
            ui.spacing().interact_size.y,
        );
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // white background, 1px outline
            painter.rect_filled(rect, 0.0, SlowColors::WHITE);
            painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.0, SlowColors::BLACK));

            let pressed = response.is_pointer_button_down_on() || self.selected;
            if pressed {
                dither::draw_dither_selection(painter, rect);
            } else if response.hovered() {
                dither::draw_dither_hover(painter, rect);
            }

            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                self.text,
                egui::FontId::proportional(14.0),
                if pressed { SlowColors::WHITE } else { SlowColors::BLACK },
            );
        }

        response
    }
}

/// Toolbar separator (vertical 1px black line)
pub fn toolbar_separator(ui: &mut Ui) {
    let height = ui.spacing().interact_size.y;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, height), egui::Sense::hover());

    if ui.is_rect_visible(rect) {
        ui.painter().vline(
            rect.center().x,
            rect.y_range(),
            egui::Stroke::new(1.0, SlowColors::BLACK),
        );
    }
}

/// Status bar: white bg, 1px black top border
pub fn status_bar(ui: &mut Ui, text: &str) {
    egui::Frame::none()
        .fill(SlowColors::WHITE)
        .stroke(egui::Stroke::new(1.0, SlowColors::BLACK))
        .inner_margin(egui::Margin::symmetric(8.0, 2.0))
        .show(ui, |ui| {
            ui.label(text);
        });
}

/// File list item for open/save dialogs.
/// Selected items get a dithered overlay instead of solid black.
pub struct FileListItem<'a> {
    name: &'a str,
    is_directory: bool,
    selected: bool,
}

impl<'a> FileListItem<'a> {
    pub fn new(name: &'a str, is_directory: bool) -> Self {
        Self { name, is_directory, selected: false }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl<'a> Widget for FileListItem<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let height = 20.0;
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), height),
            egui::Sense::click(),
        );

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // always start with white bg
            painter.rect_filled(rect, 0.0, SlowColors::WHITE);

            let text_color = if self.selected {
                dither::draw_dither_selection(painter, rect);
                SlowColors::WHITE
            } else if response.hovered() {
                dither::draw_dither_hover(painter, rect);
                SlowColors::BLACK
            } else {
                SlowColors::BLACK
            };

            // icon
            let icon = if self.is_directory { "📁" } else { "📄" };
            let icon_rect = egui::Rect::from_min_size(
                rect.min + egui::vec2(4.0, 0.0),
                egui::vec2(16.0, height),
            );
            painter.text(
                icon_rect.center(),
                egui::Align2::CENTER_CENTER,
                icon,
                egui::FontId::proportional(12.0),
                text_color,
            );

            // filename
            painter.text(
                egui::pos2(rect.min.x + 24.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                self.name,
                egui::FontId::proportional(12.0),
                text_color,
            );
        }

        response
    }
}
