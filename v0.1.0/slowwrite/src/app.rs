//! SlowWrite - simple word processor using egui's built-in TextEdit
//!
//! Rebuilt from slowNotes approach for reliable copy/paste.

use egui::{Context, Key, ScrollArea};
use slowcore::storage::{FileBrowser, RecentFiles, config_dir, documents_dir};
use slowcore::theme::{SlowColors, menu_bar, consume_tab_key};
use slowcore::widgets::status_bar;
use std::path::PathBuf;

/// Document state
struct Document {
    content: String,
    path: Option<PathBuf>,
    modified: bool,
    title: String,
}

impl Document {
    fn new() -> Self {
        Self {
            content: String::new(),
            path: None,
            modified: false,
            title: "untitled".to_string(),
        }
    }

    fn open(path: PathBuf) -> std::io::Result<Self> {
        let content = std::fs::read_to_string(&path)?;
        let title = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string());
        Ok(Self {
            content,
            path: Some(path),
            modified: false,
            title,
        })
    }

    fn save(&mut self) -> std::io::Result<()> {
        if let Some(ref path) = self.path {
            std::fs::write(path, &self.content)?;
            self.modified = false;
        }
        Ok(())
    }

    fn save_as(&mut self, path: PathBuf) -> std::io::Result<()> {
        std::fs::write(&path, &self.content)?;
        self.title = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string());
        self.path = Some(path);
        self.modified = false;
        Ok(())
    }

    fn display_title(&self) -> String {
        if self.modified {
            format!("{}*", self.title)
        } else {
            self.title.clone()
        }
    }

    fn word_count(&self) -> usize {
        self.content.split_whitespace().count()
    }

    fn char_count(&self) -> usize {
        self.content.chars().count()
    }

    fn line_count(&self) -> usize {
        self.content.lines().count().max(1)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum FileBrowserMode {
    Open,
    Save,
}

/// Application state
pub struct SlowWriteApp {
    document: Document,
    recent_files: RecentFiles,
    show_file_browser: bool,
    file_browser: FileBrowser,
    file_browser_mode: FileBrowserMode,
    save_filename: String,
    show_about: bool,
    show_close_confirm: bool,
    close_confirmed: bool,
}

impl SlowWriteApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config_path = config_dir("slowwrite").join("recent.json");
        let recent_files = RecentFiles::load(&config_path).unwrap_or_else(|_| RecentFiles::new(10));

        Self {
            document: Document::new(),
            recent_files,
            show_file_browser: false,
            file_browser: FileBrowser::new(documents_dir())
                .with_filter(vec!["txt".to_string(), "md".to_string()]),
            file_browser_mode: FileBrowserMode::Open,
            save_filename: String::new(),
            show_about: false,
            show_close_confirm: false,
            close_confirmed: false,
        }
    }

    fn new_document(&mut self) {
        self.document = Document::new();
    }

    fn open_file(&mut self, path: PathBuf) {
        match Document::open(path.clone()) {
            Ok(doc) => {
                self.document = doc;
                self.recent_files.add(path);
                self.save_recent_files();
            }
            Err(e) => {
                eprintln!("failed to open file: {}", e);
            }
        }
    }

    fn save_document(&mut self) {
        if self.document.path.is_some() {
            if let Err(e) = self.document.save() {
                eprintln!("failed to save: {}", e);
            }
        } else {
            self.show_save_as_dialog();
        }
    }

    fn save_document_as(&mut self, path: PathBuf) {
        if let Err(e) = self.document.save_as(path.clone()) {
            eprintln!("failed to save: {}", e);
        } else {
            self.recent_files.add(path);
            self.save_recent_files();
        }
    }

    fn show_open_dialog(&mut self) {
        self.file_browser = FileBrowser::new(documents_dir())
            .with_filter(vec!["txt".to_string(), "md".to_string()]);
        self.file_browser_mode = FileBrowserMode::Open;
        self.show_file_browser = true;
    }

    fn show_save_as_dialog(&mut self) {
        self.file_browser = FileBrowser::new(documents_dir());
        self.file_browser_mode = FileBrowserMode::Save;
        self.save_filename = self.document.title.clone();
        if !self.save_filename.ends_with(".txt") && !self.save_filename.ends_with(".md") {
            self.save_filename.push_str(".txt");
        }
        self.show_file_browser = true;
    }

    fn save_recent_files(&self) {
        let config_path = config_dir("slowwrite").join("recent.json");
        let _ = self.recent_files.save(&config_path);
    }

    fn handle_keyboard(&mut self, ctx: &Context) {
        // Consume Tab key so it doesn't trigger menu hover/focus
        consume_tab_key(ctx);

        ctx.input(|i| {
            let cmd = i.modifiers.command;
            let shift = i.modifiers.shift;

            if cmd && i.key_pressed(Key::N) {
                self.new_document();
            }
            if cmd && i.key_pressed(Key::O) {
                self.show_open_dialog();
            }
            if cmd && i.key_pressed(Key::S) {
                if shift {
                    self.show_save_as_dialog();
                } else {
                    self.save_document();
                }
            }
        });
    }

    fn render_menu_bar(&mut self, ui: &mut egui::Ui) {
        menu_bar(ui, |ui| {
            ui.menu_button("file", |ui| {
                if ui.button("new        ⌘n").clicked() {
                    self.new_document();
                    ui.close_menu();
                }
                if ui.button("open...    ⌘o").clicked() {
                    self.show_open_dialog();
                    ui.close_menu();
                }

                ui.menu_button("open recent", |ui| {
                    if self.recent_files.files.is_empty() {
                        ui.label("no recent files");
                    } else {
                        for path in self.recent_files.files.clone() {
                            let name = path.file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "unknown".to_string());
                            if ui.button(&name).clicked() {
                                self.open_file(path);
                                ui.close_menu();
                            }
                        }
                    }
                });

                ui.separator();

                if ui.button("save       ⌘s").clicked() {
                    self.save_document();
                    ui.close_menu();
                }
                if ui.button("save as... ⇧⌘s").clicked() {
                    self.show_save_as_dialog();
                    ui.close_menu();
                }
            });

            ui.menu_button("help", |ui| {
                if ui.button("about slowWrite").clicked() {
                    self.show_about = true;
                    ui.close_menu();
                }
            });
        });
    }

    fn render_file_browser(&mut self, ctx: &Context) {
        let title = match self.file_browser_mode {
            FileBrowserMode::Open => "open document",
            FileBrowserMode::Save => "save document",
        };

        egui::Window::new(title)
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
                                    .selected(selected)
                            );

                            if response.clicked() {
                                self.file_browser.selected_index = Some(idx);
                            }

                            if response.double_clicked() {
                                if entry.is_directory {
                                    self.file_browser.navigate_to(entry.path.clone());
                                } else if self.file_browser_mode == FileBrowserMode::Open {
                                    self.open_file(entry.path.clone());
                                    self.show_file_browser = false;
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
                    if ui.button("cancel").clicked() {
                        self.show_file_browser = false;
                    }

                    let action_text = match self.file_browser_mode {
                        FileBrowserMode::Open => "open",
                        FileBrowserMode::Save => "save",
                    };

                    if ui.button(action_text).clicked() {
                        match self.file_browser_mode {
                            FileBrowserMode::Open => {
                                if let Some(entry) = self.file_browser.selected_entry() {
                                    if !entry.is_directory {
                                        self.open_file(entry.path.clone());
                                        self.show_file_browser = false;
                                    }
                                }
                            }
                            FileBrowserMode::Save => {
                                if !self.save_filename.is_empty() {
                                    let path = self.file_browser.save_directory().join(&self.save_filename);
                                    self.save_document_as(path);
                                    self.show_file_browser = false;
                                }
                            }
                        }
                    }
                });
            });
    }

    fn render_close_confirm(&mut self, ctx: &Context) {
        egui::Window::new("unsaved changes")
            .collapsible(false)
            .resizable(false)
            .default_width(300.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("you have unsaved changes.");
                ui.label("do you want to save before closing?");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("don't save").clicked() {
                        self.close_confirmed = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button("cancel").clicked() {
                        self.show_close_confirm = false;
                    }
                    if ui.button("save").clicked() {
                        self.save_document();
                        if !self.document.modified {
                            self.close_confirmed = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                });
            });
    }

    fn render_about(&mut self, ctx: &Context) {
        egui::Window::new("about slowWrite")
            .collapsible(false)
            .resizable(false)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("slowWrite");
                    ui.label("version 0.1.0");
                    ui.add_space(8.0);
                    ui.label("word processor for slowOS");
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.label("supported formats:");
                ui.label("  .txt, .md (plain text, markdown)");
                ui.add_space(4.0);
                ui.label("features:");
                ui.label("  open, save, recent files");
                ui.label("  copy/paste (system clipboard)");
                ui.add_space(4.0);
                ui.label("frameworks:");
                ui.label("  egui/eframe (MIT)");
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    if ui.button("ok").clicked() {
                        self.show_about = false;
                    }
                });
            });
    }
}

impl eframe::App for SlowWriteApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.handle_keyboard(ctx);

        // Top menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            self.render_menu_bar(ui);
        });

        // Title bar with document name
        egui::TopBottomPanel::top("title_bar").show(ctx, |ui| {
            slowcore::theme::SlowTheme::title_bar_frame().show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(self.document.display_title());
                });
            });
        });

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            let status = format!(
                "{} lines  |  {} words, {} chars",
                self.document.line_count(),
                self.document.word_count(),
                self.document.char_count()
            );
            status_bar(ui, &status);
        });

        // Main editor area - using egui's built-in TextEdit for reliable copy/paste
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(16.0)))
            .show(ctx, |ui| {
                let available = ui.available_size();
                let line_count = self.document.content.lines().count().max(1);
                let line_number_width = 40.0;

                ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        // Line numbers column
                        ui.vertical(|ui| {
                            ui.set_min_width(line_number_width);
                            for i in 1..=line_count {
                                ui.label(
                                    egui::RichText::new(format!("{:>4}", i))
                                        .font(egui::FontId::monospace(14.0))
                                        .color(egui::Color32::GRAY)
                                );
                            }
                        });

                        // Text editor
                        let response = ui.add_sized(
                            [available.x - line_number_width - 16.0, available.y.max(400.0)],
                            egui::TextEdit::multiline(&mut self.document.content)
                                .font(egui::FontId::proportional(16.0))
                                .desired_width(available.x - line_number_width - 16.0)
                                .frame(false)
                        );
                        if response.changed() {
                            self.document.modified = true;
                        }
                    });
                });
            });

        // Dialogs
        if self.show_file_browser {
            self.render_file_browser(ctx);
        }

        if self.show_close_confirm {
            self.render_close_confirm(ctx);
        }

        if self.show_about {
            self.render_about(ctx);
        }

        // Handle close request
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.document.modified && !self.close_confirmed {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.show_close_confirm = true;
            }
        }
    }
}
