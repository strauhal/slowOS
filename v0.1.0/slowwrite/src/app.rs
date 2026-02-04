//! SlowWrite - word processor with plain text and rich text modes
//!
//! Supports .txt, .md (plain text) and .rtf (rich text) files.
//! Drag and drop files onto the window to open them.

use egui::{Context, Key, ScrollArea};
use slowcore::storage::{FileBrowser, RecentFiles, config_dir, documents_dir};
use slowcore::theme::{SlowColors, menu_bar, consume_special_keys};
use slowcore::widgets::status_bar;
use std::path::PathBuf;

/// Document mode
#[derive(Clone, Copy, PartialEq)]
enum DocMode {
    Plain,
    Rich,
}

/// Rich text style (applies to entire document)
#[derive(Clone)]
struct RichStyle {
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    font_size: f32,
    monospace: bool,
}

impl Default for RichStyle {
    fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            font_size: 16.0,
            monospace: false,
        }
    }
}

/// Strip RTF markup to extract plain text content
fn strip_rtf(input: &str) -> String {
    let mut result = String::new();
    let mut depth: i32 = 0;
    let mut chars = input.chars().peekable();
    let mut in_header = true;

    while let Some(c) = chars.next() {
        match c {
            '{' => {
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth <= 0 {
                    break;
                }
            }
            '\\' => {
                // Read control word
                let mut word = String::new();
                while let Some(&next) = chars.peek() {
                    if next.is_ascii_alphabetic() {
                        word.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                // Skip optional numeric parameter
                let mut _num = String::new();
                while let Some(&next) = chars.peek() {
                    if next.is_ascii_digit() || next == '-' {
                        _num.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                // Consume trailing space
                if chars.peek() == Some(&' ') {
                    chars.next();
                }

                if word.is_empty() {
                    // Escaped character like \\ \{ \}
                    if let Some(esc) = chars.next() {
                        match esc {
                            '\\' => result.push('\\'),
                            '{' => result.push('{'),
                            '}' => result.push('}'),
                            '\'' => {
                                // Hex char \'xx
                                let mut hex = String::new();
                                if let Some(h1) = chars.next() { hex.push(h1); }
                                if let Some(h2) = chars.next() { hex.push(h2); }
                                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                                    result.push(byte as char);
                                }
                            }
                            _ => {}
                        }
                    }
                } else {
                    in_header = false;
                    match word.as_str() {
                        "par" | "line" => result.push('\n'),
                        "tab" => result.push('\t'),
                        _ => {}
                    }
                }
            }
            '\n' | '\r' => {
                // RTF ignores raw newlines
            }
            _ => {
                if !in_header || depth <= 1 {
                    in_header = false;
                    result.push(c);
                }
            }
        }
    }
    result.trim().to_string()
}

/// Write plain text content as basic RTF
fn to_rtf(text: &str, style: &RichStyle) -> String {
    let mut rtf = String::from("{\\rtf1\\ansi\\deff0\n");
    // Font table
    if style.monospace {
        rtf.push_str("{\\fonttbl{\\f0\\fmodern Courier;}}\n");
    } else {
        rtf.push_str("{\\fonttbl{\\f0\\fswiss Helvetica;}}\n");
    }
    let fs = (style.font_size * 2.0) as u32;
    rtf.push_str(&format!("\\f0\\fs{}", fs));
    if style.bold { rtf.push_str("\\b"); }
    if style.italic { rtf.push_str("\\i"); }
    if style.underline { rtf.push_str("\\ul"); }
    if style.strikethrough { rtf.push_str("\\strike"); }
    rtf.push(' ');

    for (i, line) in text.split('\n').enumerate() {
        if i > 0 {
            rtf.push_str("\\par\n");
        }
        // Escape special RTF characters
        for ch in line.chars() {
            match ch {
                '\\' => rtf.push_str("\\\\"),
                '{' => rtf.push_str("\\{"),
                '}' => rtf.push_str("\\}"),
                _ => rtf.push(ch),
            }
        }
    }
    rtf.push_str("\n}");
    rtf
}

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
    /// Current document mode
    doc_mode: DocMode,
    /// Rich text formatting style (applies to whole document)
    rich_style: RichStyle,
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
                .with_filter(vec!["txt".to_string(), "md".to_string(), "rtf".to_string()]),
            file_browser_mode: FileBrowserMode::Open,
            save_filename: String::new(),
            show_about: false,
            show_close_confirm: false,
            close_confirmed: false,
            doc_mode: DocMode::Plain,
            rich_style: RichStyle::default(),
        }
    }

    fn new_document(&mut self) {
        self.document = Document::new();
        self.doc_mode = DocMode::Plain;
        self.rich_style = RichStyle::default();
    }

    pub fn open_file(&mut self, path: PathBuf) {
        let is_rtf = path.extension()
            .map(|e| e.to_string_lossy().to_lowercase() == "rtf")
            .unwrap_or(false);

        if is_rtf {
            // Read raw RTF and strip markup
            match std::fs::read_to_string(&path) {
                Ok(raw) => {
                    let content = strip_rtf(&raw);
                    let title = path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "untitled".to_string());
                    self.document = Document {
                        content,
                        path: Some(path.clone()),
                        modified: false,
                        title,
                    };
                    self.doc_mode = DocMode::Rich;
                    self.recent_files.add(path);
                    self.save_recent_files();
                }
                Err(e) => eprintln!("failed to open RTF file: {}", e),
            }
        } else {
            match Document::open(path.clone()) {
                Ok(doc) => {
                    self.document = doc;
                    self.doc_mode = DocMode::Plain;
                    self.recent_files.add(path);
                    self.save_recent_files();
                }
                Err(e) => {
                    eprintln!("failed to open file: {}", e);
                }
            }
        }
    }

    fn save_document(&mut self) {
        if self.document.path.is_some() {
            if self.doc_mode == DocMode::Rich {
                self.save_as_rtf();
            } else if let Err(e) = self.document.save() {
                eprintln!("failed to save: {}", e);
            }
        } else {
            self.show_save_as_dialog();
        }
    }

    fn save_as_rtf(&mut self) {
        if let Some(ref path) = self.document.path {
            let rtf_content = to_rtf(&self.document.content, &self.rich_style);
            match std::fs::write(path, &rtf_content) {
                Ok(()) => self.document.modified = false,
                Err(e) => eprintln!("failed to save RTF: {}", e),
            }
        }
    }

    fn save_document_as(&mut self, path: PathBuf) {
        let is_rtf = path.extension()
            .map(|e| e.to_string_lossy().to_lowercase() == "rtf")
            .unwrap_or(false);

        if is_rtf || self.doc_mode == DocMode::Rich {
            let rtf_content = to_rtf(&self.document.content, &self.rich_style);
            match std::fs::write(&path, &rtf_content) {
                Ok(()) => {
                    self.document.title = path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "untitled".to_string());
                    self.document.path = Some(path.clone());
                    self.document.modified = false;
                    self.doc_mode = DocMode::Rich;
                    self.recent_files.add(path);
                    self.save_recent_files();
                }
                Err(e) => eprintln!("failed to save: {}", e),
            }
        } else {
            if let Err(e) = self.document.save_as(path.clone()) {
                eprintln!("failed to save: {}", e);
            } else {
                self.recent_files.add(path);
                self.save_recent_files();
            }
        }
    }

    fn show_open_dialog(&mut self) {
        self.file_browser = FileBrowser::new(documents_dir())
            .with_filter(vec!["txt".to_string(), "md".to_string(), "rtf".to_string()]);
        self.file_browser_mode = FileBrowserMode::Open;
        self.show_file_browser = true;
    }

    fn show_save_as_dialog(&mut self) {
        self.file_browser = FileBrowser::new(documents_dir());
        self.file_browser_mode = FileBrowserMode::Save;
        self.save_filename = self.document.title.clone();
        let ext = if self.doc_mode == DocMode::Rich { ".rtf" } else { ".txt" };
        if !self.save_filename.ends_with(".txt")
            && !self.save_filename.ends_with(".md")
            && !self.save_filename.ends_with(".rtf")
        {
            self.save_filename.push_str(ext);
        }
        self.show_file_browser = true;
    }

    fn save_recent_files(&self) {
        let config_path = config_dir("slowwrite").join("recent.json");
        let _ = self.recent_files.save(&config_path);
    }

    fn handle_keyboard(&mut self, ctx: &Context) {
        // Consume Tab key so it doesn't trigger menu hover/focus
        consume_special_keys(ctx);

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

            ui.menu_button("format", |ui| {
                let mode_label = if self.doc_mode == DocMode::Plain {
                    "mode: plain text"
                } else {
                    "mode: rich text"
                };
                ui.menu_button(mode_label, |ui| {
                    if ui.button("plain text (.txt)").clicked() {
                        self.doc_mode = DocMode::Plain;
                        ui.close_menu();
                    }
                    if ui.button("rich text (.rtf)").clicked() {
                        self.doc_mode = DocMode::Rich;
                        ui.close_menu();
                    }
                });
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
                ui.label("  .rtf (rich text format)");
                ui.add_space(4.0);
                ui.label("features:");
                ui.label("  open, save, recent files");
                ui.label("  drag and drop files to open");
                ui.label("  copy/paste (system clipboard)");
                ui.label("  rtf: bold, italic, underline,");
                ui.label("  strikethrough, font size, family");
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

        // Handle drag-and-drop
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if let Some(path) = dropped.into_iter().next() {
            let ext = path.extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if ext == "txt" || ext == "md" || ext == "rtf" {
                self.open_file(path);
            }
        }

        // Top menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            self.render_menu_bar(ui);
        });

        // Title bar with document name
        egui::TopBottomPanel::top("title_bar").show(ctx, |ui| {
            slowcore::theme::SlowTheme::title_bar_frame().show(ui, |ui| {
                let mode_label = if self.doc_mode == DocMode::Rich { " (rtf)" } else { "" };
                ui.centered_and_justified(|ui| {
                    ui.label(format!("{}{}", self.document.display_title(), mode_label));
                });
            });
        });

        // Formatting toolbar (only in Rich mode)
        if self.doc_mode == DocMode::Rich {
            egui::TopBottomPanel::top("format_toolbar")
                .exact_height(28.0)
                .frame(
                    egui::Frame::none()
                        .fill(SlowColors::WHITE)
                        .stroke(egui::Stroke::new(1.0, SlowColors::BLACK))
                        .inner_margin(egui::Margin::symmetric(8.0, 2.0)),
                )
                .show(ctx, |ui| {
                    ui.horizontal_centered(|ui| {
                        // Bold
                        let b_label = if self.rich_style.bold { "[B]" } else { " B " };
                        if ui.selectable_label(self.rich_style.bold, b_label).clicked() {
                            self.rich_style.bold = !self.rich_style.bold;
                            self.document.modified = true;
                        }

                        // Italic
                        let i_label = if self.rich_style.italic { "[I]" } else { " I " };
                        if ui.selectable_label(self.rich_style.italic, i_label).clicked() {
                            self.rich_style.italic = !self.rich_style.italic;
                            self.document.modified = true;
                        }

                        // Underline
                        let u_label = if self.rich_style.underline { "[U]" } else { " U " };
                        if ui.selectable_label(self.rich_style.underline, u_label).clicked() {
                            self.rich_style.underline = !self.rich_style.underline;
                            self.document.modified = true;
                        }

                        // Strikethrough
                        let s_label = if self.rich_style.strikethrough { "[S]" } else { " S " };
                        if ui.selectable_label(self.rich_style.strikethrough, s_label).clicked() {
                            self.rich_style.strikethrough = !self.rich_style.strikethrough;
                            self.document.modified = true;
                        }

                        ui.separator();

                        // Font size
                        ui.label("size:");
                        let mut size = self.rich_style.font_size as u32;
                        let prev_size = size;
                        ui.add(egui::DragValue::new(&mut size).clamp_range(8..=72).speed(0.5));
                        if size != prev_size {
                            self.rich_style.font_size = size as f32;
                            self.document.modified = true;
                        }

                        ui.separator();

                        // Font family
                        let family_label = if self.rich_style.monospace { "monospace" } else { "proportional" };
                        ui.menu_button(format!("font: {}", family_label), |ui| {
                            if ui.button("proportional").clicked() {
                                self.rich_style.monospace = false;
                                self.document.modified = true;
                                ui.close_menu();
                            }
                            if ui.button("monospace").clicked() {
                                self.rich_style.monospace = true;
                                self.document.modified = true;
                                ui.close_menu();
                            }
                        });
                    });
                });
        }

        // Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            let mode = if self.doc_mode == DocMode::Rich { "rtf" } else { "txt" };
            let status = format!(
                "{} lines  |  {} words, {} chars  |  {}",
                self.document.line_count(),
                self.document.word_count(),
                self.document.char_count(),
                mode,
            );
            status_bar(ui, &status);
        });

        // Main editor area
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(16.0)))
            .show(ctx, |ui| {
                let available = ui.available_size();
                let line_count = self.document.content.lines().count().max(1);
                let line_number_width = 40.0;

                // Build the font and layouter based on mode
                let base_font = if self.doc_mode == DocMode::Rich {
                    let size = self.rich_style.font_size;
                    if self.rich_style.monospace {
                        egui::FontId::monospace(size)
                    } else {
                        egui::FontId::proportional(size)
                    }
                } else {
                    egui::FontId::proportional(16.0)
                };

                let line_font_size = if self.doc_mode == DocMode::Rich {
                    self.rich_style.font_size
                } else {
                    14.0
                };

                ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        // Line numbers column
                        ui.vertical(|ui| {
                            ui.set_min_width(line_number_width);
                            for i in 1..=line_count {
                                ui.label(
                                    egui::RichText::new(format!("{:>4}", i))
                                        .font(egui::FontId::monospace(line_font_size.min(14.0)))
                                        .color(egui::Color32::GRAY)
                                );
                            }
                        });

                        // Text editor with rich text layouter
                        if self.doc_mode == DocMode::Rich {
                            let style = self.rich_style.clone();
                            let mut layouter = move |ui: &egui::Ui, text: &str, wrap_width: f32| {
                                let mut job = egui::text::LayoutJob::default();
                                job.wrap.max_width = wrap_width;

                                let size = if style.bold {
                                    style.font_size + 1.0
                                } else {
                                    style.font_size
                                };
                                let font_id = if style.monospace {
                                    egui::FontId::monospace(size)
                                } else {
                                    egui::FontId::proportional(size)
                                };

                                let underline = if style.underline {
                                    egui::Stroke::new(1.0, SlowColors::BLACK)
                                } else {
                                    egui::Stroke::NONE
                                };
                                let strikethrough = if style.strikethrough {
                                    egui::Stroke::new(1.0, SlowColors::BLACK)
                                } else {
                                    egui::Stroke::NONE
                                };

                                let format = egui::TextFormat {
                                    font_id,
                                    color: SlowColors::BLACK,
                                    underline,
                                    strikethrough,
                                    italics: style.italic,
                                    ..Default::default()
                                };

                                job.append(text, 0.0, format);
                                ui.fonts(|f| f.layout_job(job))
                            };

                            let response = ui.add_sized(
                                [available.x - line_number_width - 16.0, available.y.max(400.0)],
                                egui::TextEdit::multiline(&mut self.document.content)
                                    .font(base_font)
                                    .desired_width(available.x - line_number_width - 16.0)
                                    .frame(false)
                                    .layouter(&mut layouter)
                            );
                            if response.changed() {
                                self.document.modified = true;
                            }
                        } else {
                            let response = ui.add_sized(
                                [available.x - line_number_width - 16.0, available.y.max(400.0)],
                                egui::TextEdit::multiline(&mut self.document.content)
                                    .font(base_font)
                                    .desired_width(available.x - line_number_width - 16.0)
                                    .frame(false)
                            );
                            if response.changed() {
                                self.document.modified = true;
                            }
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
