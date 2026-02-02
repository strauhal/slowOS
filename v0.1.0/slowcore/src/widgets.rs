//! Custom widgets — pure black and white, dithered overlays

use egui::{Response, Ui, Widget};
use crate::theme::SlowColors;
use crate::dither;

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
        let desired_size = ui.spacing().interact_size;
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width().min(200.0), desired_size.y),
            egui::Sense::click(),
        );

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // white background, 1px outline
            painter.rect_filled(rect, 0.0, SlowColors::WHITE);
            painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.0, SlowColors::BLACK));

            if response.is_pointer_button_down_on() || self.selected {
                // dithered overlay
                dither::draw_dither_selection(painter, rect);
                // white text on dither
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    self.text,
                    egui::FontId::proportional(14.0),
                    SlowColors::WHITE,
                );
            } else if response.hovered() {
                dither::draw_dither_hover(painter, rect);
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    self.text,
                    egui::FontId::proportional(14.0),
                    SlowColors::BLACK,
                );
            } else {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    self.text,
                    egui::FontId::proportional(14.0),
                    SlowColors::BLACK,
                );
            }
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

/// Classic scrollbar styling (placeholder — uses default)
pub struct SlowScrollArea {
    pub max_height: f32,
}

impl SlowScrollArea {
    pub fn new(max_height: f32) -> Self {
        Self { max_height }
    }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        add_contents: impl FnOnce(&mut Ui) -> R,
    ) -> egui::scroll_area::ScrollAreaOutput<R> {
        egui::ScrollArea::vertical()
            .max_height(self.max_height)
            .show(ui, add_contents)
    }
}
