//! SlowWrite v0.2.2 — plain text word processor
//!
//! Uses egui's built-in TextEdit::multiline for text editing, with custom
//! double-click-drag word selection.

use crate::rich_text::RichDocument;
use egui::{Align2, Context, Key};
use slowcore::repaint::RepaintController;
use slowcore::storage::{config_dir, documents_dir, FileBrowser, RecentFiles};
use slowcore::text_edit::WordDragState;
use slowcore::theme::{consume_special_keys, menu_bar, SlowColors};
use slowcore::widgets::{status_bar, window_control_buttons, WindowAction};
use std::path::PathBuf;

/// RTF stripping for importing existing .rtf files
fn strip_rtf(input: &str) -> String {
    let mut result = String::new();
    let mut depth: i32 = 0;
    let mut chars = input.chars().peekable();
    let mut skip_depth: i32 = 0;
    let mut in_fonttbl = false;
    let mut in_colortbl = false;
    let mut in_stylesheet = false;
    let mut in_info = false;
    let mut skip_to_space = false;

    while let Some(c) = chars.next() {
        if skip_to_space {
            if c == ' ' || c == '\\' || c == '{' || c == '}' {
                skip_to_space = false;
                if c != ' ' { /* fall through */ } else { continue; }
            } else {
                continue;
            }
        }
        match c {
            '{' => { depth += 1; }
            '}' => {
                if skip_depth > 0 && depth == skip_depth {
                    skip_depth = 0;
                    in_fonttbl = false;
                    in_colortbl = false;
                    in_stylesheet = false;
                    in_info = false;
                }
                depth -= 1;
                if depth <= 0 { break; }
            }
            '\\' => {
                let mut word = String::new();
                while let Some(&next) = chars.peek() {
                    if next.is_ascii_alphabetic() { word.push(chars.next().unwrap()); }
                    else { break; }
                }
                while let Some(&next) = chars.peek() {
                    if next.is_ascii_digit() || next == '-' { chars.next(); }
                    else { break; }
                }
                if chars.peek() == Some(&' ') { chars.next(); }

                if word.is_empty() {
                    if let Some(esc) = chars.next() {
                        match esc {
                            '\\' => { if skip_depth == 0 { result.push('\\'); } }
                            '{' => { if skip_depth == 0 { result.push('{'); } }
                            '}' => { if skip_depth == 0 { result.push('}'); } }
                            '\'' => {
                                let mut hex = String::new();
                                if let Some(h1) = chars.next() { hex.push(h1); }
                                if let Some(h2) = chars.next() { hex.push(h2); }
                                if skip_depth == 0 {
                                    if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                                        result.push(byte as char);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                } else {
                    match word.as_str() {
                        "fonttbl" => { in_fonttbl = true; skip_depth = depth; }
                        "colortbl" => { in_colortbl = true; skip_depth = depth; }
                        "stylesheet" => { in_stylesheet = true; skip_depth = depth; }
                        "info" => { in_info = true; skip_depth = depth; }
                        "pict" | "object" | "field" => { skip_depth = depth; }
                        "par" | "line" => { if skip_depth == 0 { result.push('\n'); } }
                        "tab" => { if skip_depth == 0 { result.push('\t'); } }
                        "HYPERLINK" => { skip_to_space = true; }
                        _ => {}
                    }
                }
            }
            '\n' | '\r' => {}
            ';' => {
                if skip_depth == 0 && !in_fonttbl && !in_colortbl && !in_stylesheet && !in_info {
                    result.push(c);
                }
            }
            '"' => {
                if skip_depth == 0 && !skip_to_space { result.push(c); }
            }
            _ => {
                if skip_depth == 0 { result.push(c); }
            }
        }
    }
    let cleaned: String = result.trim().to_string();
    let mut final_result = String::new();
    let mut prev_space = false;
    for c in cleaned.chars() {
        if c == ' ' {
            if !prev_space { final_result.push(c); prev_space = true; }
        } else {
            final_result.push(c);
            prev_space = false;
        }
    }
    final_result
}

#[derive(Clone, Copy, PartialEq)]
enum FileBrowserMode {
    Open,
    Save,
}

/// Application state
pub struct SlowWriteApp {
    doc: RichDocument,
    file_path: Option<PathBuf>,
    file_title: String,
    modified: bool,
    recent_files: RecentFiles,
    show_file_browser: bool,
    file_browser: FileBrowser,
    file_browser_mode: FileBrowserMode,
    save_filename: String,
    show_about: bool,
    show_close_confirm: bool,
    close_confirmed: bool,
    show_shortcuts: bool,
    /// Word-selection drag state
    word_drag: WordDragState,
    repaint: RepaintController,
}

impl SlowWriteApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config_path = config_dir("slowwrite").join("recent.json");
        let recent_files =
            RecentFiles::load(&config_path).unwrap_or_else(|_| RecentFiles::new(10));

        Self {
            doc: RichDocument::new(),
            file_path: None,
            file_title: "untitled".to_string(),
            modified: false,
            recent_files,
            show_file_browser: false,
            file_browser: FileBrowser::new(documents_dir())
                .with_filter(vec![
                    "txt".to_string(),
                    "md".to_string(),
                ]),
            file_browser_mode: FileBrowserMode::Open,
            save_filename: String::new(),
            show_about: false,
            show_close_confirm: false,
            close_confirmed: false,
            show_shortcuts: false,
            word_drag: WordDragState::new(),
            repaint: RepaintController::new(),
        }
    }

    fn new_document(&mut self) {
        self.doc = RichDocument::new();
        self.file_path = None;
        self.file_title = "untitled".to_string();
        self.modified = false;
        self.word_drag = WordDragState::new();
    }

    pub fn open_file(&mut self, path: PathBuf) {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        let text = match ext.as_str() {
            "rtf" => {
                match std::fs::read_to_string(&path) {
                    Ok(raw) => strip_rtf(&raw),
                    Err(e) => { eprintln!("failed to open: {}", e); return; }
                }
            }
            _ => {
                match std::fs::read_to_string(&path) {
                    Ok(t) => t,
                    Err(e) => { eprintln!("failed to open: {}", e); return; }
                }
            }
        };

        self.doc = RichDocument::from_plain_text(text);
        self.file_title = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or("untitled".to_string());
        self.file_path = Some(path.clone());
        self.modified = false;
        self.word_drag = WordDragState::new();
        self.recent_files.add(path);
        self.save_recent_files();
    }

    fn save_content_for_path(&self, _path: &std::path::Path) -> String {
        self.doc.text.clone()
    }

    fn save_document(&mut self) {
        if let Some(ref path) = self.file_path {
            let content = self.save_content_for_path(path);
            if let Err(e) = std::fs::write(path, &content) {
                eprintln!("failed to save: {}", e);
            } else {
                self.modified = false;
            }
        } else {
            self.show_save_as_dialog();
        }
    }

    fn save_document_as(&mut self, path: PathBuf) {
        let content = self.save_content_for_path(&path);
        if let Err(e) = std::fs::write(&path, &content) {
            eprintln!("failed to save: {}", e);
        } else {
            self.file_title = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or("untitled".to_string());
            self.file_path = Some(path.clone());
            self.modified = false;
            self.recent_files.add(path);
            self.save_recent_files();
        }
    }

    fn show_open_dialog(&mut self) {
        self.file_browser = FileBrowser::new(documents_dir()).with_filter(vec![
            "txt".to_string(),
            "md".to_string(),
        ]);
        self.file_browser_mode = FileBrowserMode::Open;
        self.show_file_browser = true;
    }

    fn show_save_as_dialog(&mut self) {
        self.file_browser = FileBrowser::new(documents_dir());
        self.file_browser_mode = FileBrowserMode::Save;
        self.save_filename = self.file_title.clone();
        let has_ext = self.save_filename.ends_with(".txt")
            || self.save_filename.ends_with(".md");
        if !has_ext {
            self.save_filename.push_str(".txt");
        }
        self.show_file_browser = true;
    }

    fn save_recent_files(&self) {
        let config_path = config_dir("slowwrite").join("recent.json");
        let _ = self.recent_files.save(&config_path);
    }

    fn display_title(&self) -> String {
        if self.modified {
            format!("{}*", self.file_title)
        } else {
            self.file_title.clone()
        }
    }

    /// Process keyboard shortcuts that should be handled before TextEdit consumes them.
    /// We only intercept Cmd+key shortcuts (file ops, formatting) here.
    /// TextEdit handles all text input, cursor movement, clipboard, and selection natively.
    fn handle_keyboard(&mut self, ctx: &Context) {
        consume_special_keys(ctx);

        let mut actions: Vec<Box<dyn FnOnce(&mut Self)>> = Vec::new();

        ctx.input_mut(|i| {
            let cmd = i.modifiers.command;
            let shift = i.modifiers.shift;

            let events = std::mem::take(&mut i.events);
            let mut remaining = Vec::new();

            for event in events {
                let mut handled = false;
                match &event {
                    egui::Event::Key { key, pressed: true, .. } => {
                        match key {
                            // File operations
                            Key::N if cmd => { handled = true; actions.push(Box::new(|s| s.new_document())); }
                            Key::O if cmd => { handled = true; actions.push(Box::new(|s| s.show_open_dialog())); }
                            Key::S if cmd && shift => { handled = true; actions.push(Box::new(|s| s.show_save_as_dialog())); }
                            Key::S if cmd => { handled = true; actions.push(Box::new(|s| s.save_document())); }
                            _ => {}
                        }
                    }
                    _ => {}
                }
                if !handled {
                    remaining.push(event);
                }
            }
            i.events = remaining;
        });

        for action in actions {
            action(self);
        }
    }

    fn render_menu_bar(&mut self, ui: &mut egui::Ui) -> WindowAction {
        let mut action = WindowAction::None;
        menu_bar(ui, |ui| {
            action = window_control_buttons(ui);
            ui.menu_button("file", |ui| {
                if ui.button("new        \u{2318}n").clicked() {
                    self.new_document();
                    ui.close_menu();
                }
                if ui.button("open...    \u{2318}o").clicked() {
                    self.show_open_dialog();
                    ui.close_menu();
                }
                ui.menu_button("open recent", |ui| {
                    if self.recent_files.files.is_empty() {
                        ui.label("no recent files");
                    } else {
                        for path in self.recent_files.files.clone() {
                            let name = path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or("unknown".to_string());
                            if ui.button(&name).clicked() {
                                self.open_file(path);
                                ui.close_menu();
                            }
                        }
                    }
                });
                ui.separator();
                if ui.button("save       \u{2318}s").clicked() {
                    self.save_document();
                    ui.close_menu();
                }
                if ui.button("save as... \u{21e7}\u{2318}s").clicked() {
                    self.show_save_as_dialog();
                    ui.close_menu();
                }
            });

            ui.menu_button("edit", |ui| {
                if ui.button("cut        \u{2318}x").clicked() {
                    // TextEdit handles clipboard natively via Cmd+X;
                    // Menu cut triggers via the UI context's events
                    ui.ctx().input_mut(|i| {
                        i.events.push(egui::Event::Cut);
                    });
                    ui.close_menu();
                }
                if ui.button("copy       \u{2318}c").clicked() {
                    ui.ctx().input_mut(|i| {
                        i.events.push(egui::Event::Copy);
                    });
                    ui.close_menu();
                }
                if ui.button("paste      \u{2318}v").clicked() {
                    // Attempt to get text from system clipboard and inject as Paste event
                    let text = arboard::Clipboard::new().ok()
                        .and_then(|mut c| c.get_text().ok())
                        .unwrap_or_default();
                    if !text.is_empty() {
                        ui.ctx().input_mut(|i| {
                            i.events.push(egui::Event::Text(text));
                        });
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("select all \u{2318}a").clicked() {
                    // Inject Ctrl+A equivalent — send key event
                    ui.ctx().input_mut(|i| {
                        i.events.push(egui::Event::Key {
                            key: Key::A,
                            physical_key: Some(Key::A),
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::COMMAND,
                        });
                    });
                    ui.close_menu();
                }
            });

            ui.menu_button("help", |ui| {
                if ui.button("keyboard shortcuts").clicked() {
                    self.show_shortcuts = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("about").clicked() {
                    self.show_about = true;
                    ui.close_menu();
                }
            });
        });
        action
    }

    /// Render the editor using egui's built-in TextEdit::multiline
    fn render_editor(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let output = egui::TextEdit::multiline(&mut self.doc.text)
                    .font(egui::FontId::proportional(16.0))
                    .desired_width(available.x)
                    .desired_rows((available.y / 20.0).max(4.0) as usize)
                    .frame(false)
                    .show(ui);

                // Detect text changes from TextEdit (typing, paste, delete, etc.)
                if output.response.changed() {
                    self.modified = true;
                }

                // Double-click-drag word selection (via slowcore)
                self.word_drag.update(ui, &output, &self.doc.text);
            });
    }

    fn render_file_browser(&mut self, ctx: &Context) {
        let title = match self.file_browser_mode {
            FileBrowserMode::Open => "open document",
            FileBrowserMode::Save => "save document",
        };
        let resp = egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("location:");
                    ui.label(self.file_browser.current_dir.to_string_lossy().to_string());
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        let entries = self.file_browser.entries.clone();
                        for (idx, entry) in entries.iter().enumerate() {
                            let selected = self.file_browser.selected_index == Some(idx);
                            let response = ui.add(
                                slowcore::widgets::FileListItem::new(&entry.name, entry.is_directory)
                                    .selected(selected),
                            );
                            if response.clicked() { self.file_browser.selected_index = Some(idx); }
                            if response.double_clicked() {
                                if entry.is_directory {
                                    self.file_browser.navigate_to(entry.path.clone());
                                } else if self.file_browser_mode == FileBrowserMode::Open {
                                    let p = entry.path.clone();
                                    self.show_file_browser = false;
                                    self.open_file(p);
                                }
                            }
                        }
                    });
                if self.file_browser_mode == FileBrowserMode::Save {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("filename:");
                        ui.text_edit_singleline(&mut self.save_filename);
                    });
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("cancel").clicked() { self.show_file_browser = false; }
                    let action_text = match self.file_browser_mode {
                        FileBrowserMode::Open => "open",
                        FileBrowserMode::Save => "save",
                    };
                    if ui.button(action_text).clicked() {
                        match self.file_browser_mode {
                            FileBrowserMode::Open => {
                                if let Some(entry) = self.file_browser.selected_entry() {
                                    if !entry.is_directory {
                                        let p = entry.path.clone();
                                        self.show_file_browser = false;
                                        self.open_file(p);
                                    }
                                }
                            }
                            FileBrowserMode::Save => {
                                if !self.save_filename.is_empty() {
                                    let path = self.file_browser.save_directory().join(&self.save_filename);
                                    self.show_file_browser = false;
                                    self.save_document_as(path);
                                }
                            }
                        }
                    }
                });
            });
        if let Some(r) = &resp { slowcore::dither::draw_window_shadow(ctx, r.response.rect); }
    }

    fn render_about(&mut self, ctx: &Context) {
        let max_height = (ctx.screen_rect().height() - 80.0).max(200.0);
        let resp = egui::Window::new("about slowWrite")
            .collapsible(false).resizable(false).default_width(300.0).max_height(max_height)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(max_height - 60.0).show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("slowWrite");
                        ui.label("version 0.2.2");
                        ui.add_space(8.0);
                        ui.label("text editor for slowOS");
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.label("supported formats:");
                    ui.label("  .txt, .md (plain text)");
                    ui.add_space(4.0);
                    ui.label("features:");
                    ui.label("  double-click-drag word selection");
                    ui.add_space(8.0);
                });
                ui.vertical_centered(|ui| {
                    if ui.button("ok").clicked() { self.show_about = false; }
                });
            });
        if let Some(r) = &resp { slowcore::dither::draw_window_shadow_large(ctx, r.response.rect); }
    }

    fn render_shortcuts(&mut self, ctx: &Context) {
        let max_height = (ctx.screen_rect().height() - 80.0).max(200.0);
        let resp = egui::Window::new("keyboard shortcuts")
            .collapsible(false).resizable(false).default_width(320.0).max_height(max_height)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(max_height - 60.0).show(ui, |ui| {
                    ui.heading("slowWrite shortcuts");
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("File").strong());
                    ui.separator();
                    shortcut_row(ui, "\u{2318}N", "New document");
                    shortcut_row(ui, "\u{2318}O", "Open file");
                    shortcut_row(ui, "\u{2318}S", "Save");
                    shortcut_row(ui, "\u{21e7}\u{2318}S", "Save as");
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Editing").strong());
                    ui.separator();
                    shortcut_row(ui, "\u{2318}X", "Cut");
                    shortcut_row(ui, "\u{2318}C", "Copy");
                    shortcut_row(ui, "\u{2318}V", "Paste");
                    shortcut_row(ui, "\u{2318}A", "Select all");
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Selection").strong());
                    ui.separator();
                    shortcut_row(ui, "Double-click", "Select word");
                    shortcut_row(ui, "Dbl-click drag", "Select words");
                    shortcut_row(ui, "\u{21e7}+Click", "Extend selection");
                    ui.add_space(8.0);
                });
                ui.vertical_centered(|ui| {
                    if ui.button("ok").clicked() { self.show_shortcuts = false; }
                });
            });
        if let Some(r) = &resp { slowcore::dither::draw_window_shadow(ctx, r.response.rect); }
    }

    fn render_close_confirm(&mut self, ctx: &Context) {
        let resp = egui::Window::new("unsaved changes")
            .collapsible(false).resizable(false).default_width(300.0)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("you have unsaved changes.");
                ui.label("do you want to save before closing?");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("don't save").clicked() {
                        self.close_confirmed = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button("cancel").clicked() { self.show_close_confirm = false; }
                    if ui.button("save").clicked() {
                        self.save_document();
                        if !self.modified {
                            self.close_confirmed = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                });
            });
        if let Some(r) = &resp { slowcore::dither::draw_window_shadow(ctx, r.response.rect); }
    }
}

fn shortcut_row(ui: &mut egui::Ui, shortcut: &str, description: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(shortcut).monospace().strong());
        ui.add_space(20.0);
        ui.label(description);
    });
}

impl eframe::App for SlowWriteApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.repaint.begin_frame(ctx);
        if slowcore::minimize::check_restore_signal("slowwrite") {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        self.handle_keyboard(ctx);

        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect()
        });
        if let Some(path) = dropped.into_iter().next() {
            let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if ext == "txt" || ext == "md" {
                self.open_file(path);
            }
        }

        let mut win_action = WindowAction::None;
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| { win_action = self.render_menu_bar(ui); });
        match win_action {
            WindowAction::Close => {
                if self.modified {
                    self.show_close_confirm = true;
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            WindowAction::Minimize => {
                let title = if self.file_title == "untitled" {
                    "slowWrite".to_string()
                } else {
                    format!("{} — slowWrite", self.file_title)
                };
                slowcore::minimize::write_minimized("slowwrite", &title);
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            WindowAction::None => {}
        }
        egui::TopBottomPanel::top("title_bar").show(ctx, |ui| {
            slowcore::theme::SlowTheme::title_bar_frame().show(ui, |ui| {
                ui.centered_and_justified(|ui| { ui.label(self.display_title()); });
            });
        });
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            let status = format!("{} lines  |  {} words, {} chars",
                self.doc.line_count(), self.doc.word_count(), self.doc.char_count());
            status_bar(ui, &status);
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(0.0)))
            .show(ctx, |ui| { self.render_editor(ui); });

        if self.show_file_browser { self.render_file_browser(ctx); }
        if self.show_close_confirm { self.render_close_confirm(ctx); }
        if self.show_about { self.render_about(ctx); }
        if self.show_shortcuts { self.render_shortcuts(ctx); }

        if ctx.input(|i| i.viewport().close_requested()) {
            if self.modified && !self.close_confirmed {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.show_close_confirm = true;
            }
        }

        self.repaint.end_frame(ctx);
    }
}
