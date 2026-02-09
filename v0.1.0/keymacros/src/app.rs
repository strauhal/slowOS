//! KeyMacros - Keyboard shortcuts reference for slowOS

use egui::{Context, ScrollArea};
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::status_bar;

pub struct KeyMacrosApp {
    selected_category: usize,
    show_about: bool,
}

impl KeyMacrosApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            selected_category: 0,
            show_about: false,
        }
    }
}

impl eframe::App for KeyMacrosApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            menu_bar(ui, |ui| {
                ui.menu_button("help", |ui| {
                    if ui.button("about").clicked() {
                        self.show_about = true;
                        ui.close_menu();
                    }
                });
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            status_bar(ui, "keyboard shortcuts reference");
        });

        // Category sidebar
        egui::SidePanel::left("categories").default_width(140.0).show(ctx, |ui| {
            ui.heading("categories");
            ui.add_space(8.0);

            let categories = [
                "system",
                "slowWrite",
                "slowPaint",
                "slowFiles",
                "slowReader",
                "slowSheets",
                "slowSlides",
            ];

            for (i, cat) in categories.iter().enumerate() {
                let selected = self.selected_category == i;
                if ui.selectable_label(selected, *cat).clicked() {
                    self.selected_category = i;
                }
            }
        });

        // Main content
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(12.0)))
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    match self.selected_category {
                        0 => render_system_shortcuts(ui),
                        1 => render_slowwrite_shortcuts(ui),
                        2 => render_slowpaint_shortcuts(ui),
                        3 => render_slowfiles_shortcuts(ui),
                        4 => render_slowreader_shortcuts(ui),
                        5 => render_slowsheets_shortcuts(ui),
                        6 => render_slowslides_shortcuts(ui),
                        _ => {}
                    }
                });
            });

        // About dialog
        if self.show_about {
            egui::Window::new("about keyMacros")
                .collapsible(false)
                .resizable(false)
                .default_width(280.0)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("keyMacros");
                        ui.label("version 0.1.0");
                        ui.add_space(8.0);
                        ui.label("keyboard shortcuts reference for slowOS");
                    });
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        if ui.button("ok").clicked() {
                            self.show_about = false;
                        }
                    });
                });
        }
    }
}

fn shortcut_row(ui: &mut egui::Ui, shortcut: &str, description: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(shortcut).monospace().strong());
        ui.add_space(20.0);
        ui.label(description);
    });
    ui.add_space(2.0);
}

fn section_header(ui: &mut egui::Ui, title: &str) {
    ui.add_space(8.0);
    ui.label(egui::RichText::new(title).strong().size(14.0));
    ui.separator();
    ui.add_space(4.0);
}

fn render_system_shortcuts(ui: &mut egui::Ui) {
    ui.heading("system shortcuts");
    ui.add_space(8.0);

    section_header(ui, "Desktop");
    shortcut_row(ui, "Cmd+Space", "Open spotlight search");
    shortcut_row(ui, "Cmd+Q", "Show shutdown dialog");
    shortcut_row(ui, "Escape", "Deselect all / close dialogs");
    shortcut_row(ui, "Enter", "Open selected app or folder");
    shortcut_row(ui, "Arrow keys", "Navigate between icons");
    shortcut_row(ui, "Shift+Click", "Select range of items");
    shortcut_row(ui, "Cmd+Click", "Toggle item selection");

    section_header(ui, "Window Management");
    shortcut_row(ui, "Cmd+W", "Close window");
    shortcut_row(ui, "Cmd+Q", "Quit application");

    section_header(ui, "General (most apps)");
    shortcut_row(ui, "Cmd+N", "New document/item");
    shortcut_row(ui, "Cmd+O", "Open file");
    shortcut_row(ui, "Cmd+S", "Save");
    shortcut_row(ui, "Cmd+Shift+S", "Save as");
    shortcut_row(ui, "Cmd+Z", "Undo");
    shortcut_row(ui, "Cmd+Shift+Z", "Redo");
    shortcut_row(ui, "Cmd+X", "Cut");
    shortcut_row(ui, "Cmd+C", "Copy");
    shortcut_row(ui, "Cmd+V", "Paste");
    shortcut_row(ui, "Cmd+A", "Select all");
}

fn render_slowwrite_shortcuts(ui: &mut egui::Ui) {
    ui.heading("slowWrite shortcuts");
    ui.add_space(8.0);

    section_header(ui, "File Operations");
    shortcut_row(ui, "Cmd+N", "New document");
    shortcut_row(ui, "Cmd+O", "Open file");
    shortcut_row(ui, "Cmd+S", "Save");
    shortcut_row(ui, "Cmd+Shift+S", "Save as");
    shortcut_row(ui, "Cmd+W", "Close");

    section_header(ui, "Editing");
    shortcut_row(ui, "Cmd+Z", "Undo");
    shortcut_row(ui, "Cmd+Shift+Z", "Redo");
    shortcut_row(ui, "Cmd+X", "Cut");
    shortcut_row(ui, "Cmd+C", "Copy");
    shortcut_row(ui, "Cmd+V", "Paste");
    shortcut_row(ui, "Cmd+A", "Select all");

    section_header(ui, "Text Formatting");
    shortcut_row(ui, "Cmd+B", "Bold");
    shortcut_row(ui, "Cmd+I", "Italic");
    shortcut_row(ui, "Cmd+U", "Underline");

    section_header(ui, "Navigation");
    shortcut_row(ui, "Cmd+F", "Find");
    shortcut_row(ui, "Cmd+G", "Find next");
    shortcut_row(ui, "Cmd+Home", "Go to beginning");
    shortcut_row(ui, "Cmd+End", "Go to end");
    shortcut_row(ui, "Cmd+Up", "Move to start of document");
    shortcut_row(ui, "Cmd+Down", "Move to end of document");

    section_header(ui, "View");
    shortcut_row(ui, "Cmd++", "Increase font size");
    shortcut_row(ui, "Cmd+-", "Decrease font size");
}

fn render_slowpaint_shortcuts(ui: &mut egui::Ui) {
    ui.heading("slowPaint shortcuts");
    ui.add_space(8.0);

    section_header(ui, "File Operations");
    shortcut_row(ui, "Cmd+N", "New canvas");
    shortcut_row(ui, "Cmd+O", "Open image");
    shortcut_row(ui, "Cmd+S", "Save");
    shortcut_row(ui, "Cmd+Shift+S", "Save as");

    section_header(ui, "Editing");
    shortcut_row(ui, "Cmd+Z", "Undo");
    shortcut_row(ui, "Cmd+Shift+Z", "Redo");
    shortcut_row(ui, "Cmd+X", "Cut selection");
    shortcut_row(ui, "Cmd+C", "Copy selection");
    shortcut_row(ui, "Cmd+V", "Paste (enters Select mode)");
    shortcut_row(ui, "Delete/Backspace", "Delete selection");
    shortcut_row(ui, "Cmd+A", "Select all");
    shortcut_row(ui, "Escape", "Deselect / cancel floating");

    section_header(ui, "Tools");
    shortcut_row(ui, "P", "Pencil tool");
    shortcut_row(ui, "B", "Brush tool");
    shortcut_row(ui, "E", "Eraser tool");
    shortcut_row(ui, "L", "Line tool");
    shortcut_row(ui, "R", "Rectangle tool");
    shortcut_row(ui, "G", "Fill (paint bucket) tool");
    shortcut_row(ui, "M", "Marquee selection tool");
    shortcut_row(ui, "X", "Swap foreground/background color");

    section_header(ui, "View");
    shortcut_row(ui, "Cmd++", "Zoom in");
    shortcut_row(ui, "Cmd+-", "Zoom out");
    shortcut_row(ui, "Middle drag", "Pan canvas");

    section_header(ui, "Selection");
    shortcut_row(ui, "Marquee", "Draw rectangle to select area");
    shortcut_row(ui, "Lasso", "Draw freeform to select area");
    shortcut_row(ui, "Select tool", "Move cut/copied content (auto-activated)");
}

fn render_slowfiles_shortcuts(ui: &mut egui::Ui) {
    ui.heading("slowFiles shortcuts");
    ui.add_space(8.0);

    section_header(ui, "Navigation");
    shortcut_row(ui, "Enter", "Open selected item");
    shortcut_row(ui, "Backspace", "Go to parent folder");
    shortcut_row(ui, "Cmd+Up", "Go up one folder");
    shortcut_row(ui, "Arrow keys", "Navigate between items");

    section_header(ui, "Selection");
    shortcut_row(ui, "Cmd+A", "Select all");
    shortcut_row(ui, "Shift+Click", "Select range");
    shortcut_row(ui, "Cmd+Click", "Toggle item selection");
    shortcut_row(ui, "Click+Drag", "Marquee select (icon view)");
    shortcut_row(ui, "Escape", "Deselect all");

    section_header(ui, "File Operations");
    shortcut_row(ui, "Cmd+N", "New folder");
    shortcut_row(ui, "Delete", "Move to trash");
    shortcut_row(ui, "Drag to folder", "Move files");

    section_header(ui, "View");
    shortcut_row(ui, "1", "Icon view");
    shortcut_row(ui, "2", "List view");
}

fn render_slowreader_shortcuts(ui: &mut egui::Ui) {
    ui.heading("slowReader shortcuts");
    ui.add_space(8.0);

    section_header(ui, "Navigation");
    shortcut_row(ui, "Right Arrow", "Next page/chapter");
    shortcut_row(ui, "Left Arrow", "Previous page/chapter");
    shortcut_row(ui, "Page Down", "Next page");
    shortcut_row(ui, "Page Up", "Previous page");
    shortcut_row(ui, "Home", "Go to beginning");
    shortcut_row(ui, "End", "Go to end");

    section_header(ui, "View");
    shortcut_row(ui, "Cmd++", "Increase font size");
    shortcut_row(ui, "Cmd+-", "Decrease font size");

    section_header(ui, "Library");
    shortcut_row(ui, "Cmd+O", "Open book");
    shortcut_row(ui, "Cmd+W", "Close book / return to library");
}

fn render_slowsheets_shortcuts(ui: &mut egui::Ui) {
    ui.heading("slowSheets shortcuts");
    ui.add_space(8.0);

    section_header(ui, "File Operations");
    shortcut_row(ui, "Cmd+N", "New spreadsheet");
    shortcut_row(ui, "Cmd+O", "Open file");
    shortcut_row(ui, "Cmd+S", "Save");
    shortcut_row(ui, "Cmd+Shift+S", "Save as");

    section_header(ui, "Navigation");
    shortcut_row(ui, "Arrow keys", "Move between cells");
    shortcut_row(ui, "Tab", "Move to next cell");
    shortcut_row(ui, "Shift+Tab", "Move to previous cell");
    shortcut_row(ui, "Enter", "Confirm edit and move down");
    shortcut_row(ui, "Cmd+Home", "Go to cell A1");

    section_header(ui, "Editing");
    shortcut_row(ui, "Cmd+Z", "Undo");
    shortcut_row(ui, "Cmd+Shift+Z", "Redo");
    shortcut_row(ui, "Cmd+X", "Cut");
    shortcut_row(ui, "Cmd+C", "Copy");
    shortcut_row(ui, "Cmd+V", "Paste");
    shortcut_row(ui, "Delete", "Clear cell contents");
    shortcut_row(ui, "F2", "Edit cell");

    section_header(ui, "Selection");
    shortcut_row(ui, "Shift+Arrow", "Extend selection");
    shortcut_row(ui, "Cmd+A", "Select all cells");
    shortcut_row(ui, "Cmd+Shift+End", "Select to last used cell");
}

fn render_slowslides_shortcuts(ui: &mut egui::Ui) {
    ui.heading("slowSlides shortcuts");
    ui.add_space(8.0);

    section_header(ui, "File Operations");
    shortcut_row(ui, "Cmd+N", "New presentation");
    shortcut_row(ui, "Cmd+O", "Open file");
    shortcut_row(ui, "Cmd+S", "Save");
    shortcut_row(ui, "Cmd+Shift+S", "Save as");

    section_header(ui, "Slides");
    shortcut_row(ui, "Cmd+M", "New slide");
    shortcut_row(ui, "Delete", "Delete selected slide");
    shortcut_row(ui, "Up/Down arrows", "Navigate slides in sidebar");

    section_header(ui, "Presentation");
    shortcut_row(ui, "Cmd+Enter", "Start presentation");
    shortcut_row(ui, "Escape", "Exit presentation");
    shortcut_row(ui, "Right Arrow", "Next slide");
    shortcut_row(ui, "Left Arrow", "Previous slide");
    shortcut_row(ui, "Space", "Next slide");

    section_header(ui, "Editing");
    shortcut_row(ui, "Cmd+Z", "Undo");
    shortcut_row(ui, "Cmd+Shift+Z", "Redo");
}
