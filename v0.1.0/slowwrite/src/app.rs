//! SlowWrite - word processor with plain text and rich text modes
//!
//! Supports .txt, .md (plain text) and .rtf (rich text) files.
//! Drag and drop files onto the window to open them.

use egui::{Context, Key, ScrollArea};
use slowcore::storage::{FileBrowser, RecentFiles, config_dir, documents_dir};
use slowcore::theme::{SlowColors, menu_bar, consume_special_keys_with_tab};
use slowcore::widgets::status_bar;
use std::path::PathBuf;

/// Text style for plain text editing
#[derive(Clone)]
struct TextStyle {
    font_size: f32,
    monospace: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_size: 16.0,
            monospace: true,
        }
    }
}

/// Strip RTF markup to extract plain text content
fn strip_rtf(input: &str) -> String {
    let mut result = String::new();
    let mut depth: i32 = 0;
    let mut chars = input.chars().peekable();
    let mut skip_depth: i32 = 0; // Depth at which we started skipping (for groups to ignore)
    let mut in_fonttbl = false;
    let mut in_colortbl = false;
    let mut in_stylesheet = false;
    let mut in_info = false;
    let mut skip_to_space = false; // Skip until space (for HYPERLINK URLs, etc.)

    while let Some(c) = chars.next() {
        // If we're skipping to space, consume until we hit space or end of argument
        if skip_to_space {
            if c == ' ' || c == '\\' || c == '{' || c == '}' {
                skip_to_space = false;
                // Put back the control char if needed
                if c == '\\' || c == '{' || c == '}' {
                    // We need to process this char, so fall through
                } else {
                    continue;
                }
            } else {
                continue;
            }
        }

        match c {
            '{' => {
                depth += 1;
            }
            '}' => {
                // Check if we're exiting a skipped group
                if skip_depth > 0 && depth == skip_depth {
                    skip_depth = 0;
                    in_fonttbl = false;
                    in_colortbl = false;
                    in_stylesheet = false;
                    in_info = false;
                }
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
                // Consume trailing space (but not if it's meaningful)
                if chars.peek() == Some(&' ') {
                    chars.next();
                }

                if word.is_empty() {
                    // Escaped character like \\ \{ \}
                    if let Some(esc) = chars.next() {
                        match esc {
                            '\\' => {
                                if skip_depth == 0 { result.push('\\'); }
                            }
                            '{' => {
                                if skip_depth == 0 { result.push('{'); }
                            }
                            '}' => {
                                if skip_depth == 0 { result.push('}'); }
                            }
                            '\'' => {
                                // Hex char \'xx
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
                    // Check for groups we should skip entirely
                    match word.as_str() {
                        "fonttbl" => {
                            in_fonttbl = true;
                            skip_depth = depth;
                        }
                        "colortbl" => {
                            in_colortbl = true;
                            skip_depth = depth;
                        }
                        "stylesheet" => {
                            in_stylesheet = true;
                            skip_depth = depth;
                        }
                        "info" => {
                            in_info = true;
                            skip_depth = depth;
                        }
                        "pict" | "object" | "field" => {
                            // Skip picture, object, and field data groups
                            skip_depth = depth;
                        }
                        "par" | "line" => {
                            if skip_depth == 0 { result.push('\n'); }
                        }
                        "tab" => {
                            if skip_depth == 0 { result.push('\t'); }
                        }
                        // Skip hyperlink URL - the text that follows is just the URL
                        "HYPERLINK" => {
                            // Skip until we hit a space, then the actual text follows
                            // The format is: {\field{\*\fldinst{HYPERLINK "url"}}{\fldrslt text}}
                            // We want to skip "url" and keep "text"
                            // For now, skip the quoted URL
                            skip_to_space = true;
                        }
                        // Control words to ignore (formatting)
                        "b" | "i" | "ul" | "strike" | "f" | "fs" | "cf" | "cb" |
                        "pard" | "plain" | "ql" | "qc" | "qr" | "qj" | "fi" | "li" | "ri" |
                        "sl" | "slmult" | "sb" | "sa" | "lang" | "deflang" | "deff" |
                        "fnil" | "fswiss" | "froman" | "fmodern" | "fscript" | "fdecor" |
                        "ftech" | "fbidi" | "fcharset" | "cpg" | "ansicpg" | "ansi" |
                        "rtf" | "uc" | "viewkind" | "viewscale" | "paperw" | "paperh" |
                        "margl" | "margr" | "margt" | "margb" | "sectd" | "linex" |
                        "headery" | "footery" | "ftnbj" | "aenddoc" | "noxlattoyen" |
                        "expshrtn" | "noultrlspc" | "dntblnsbdb" | "nospaceforul" |
                        "formshade" | "horzdoc" | "dgmargin" | "dghspace" | "dgvspace" |
                        "dghorigin" | "dgvorigin" | "dghshow" | "dgvshow" | "jcompress" |
                        "red" | "green" | "blue" | "fldinst" | "fldrslt" | "cs" | "s" |
                        "highlight" | "expndtw" | "kerning" | "outl" | "shad" | "caps" |
                        "scaps" | "ltrch" | "rtlch" | "loch" | "hich" | "dbch" => {}
                        _ => {
                            // Unknown control word - ignore
                        }
                    }
                }
            }
            '\n' | '\r' => {
                // RTF ignores raw newlines
            }
            ';' => {
                // Semicolons are delimiters in font tables, color tables, etc.
                // Only output if we're not in a skipped group
                if skip_depth == 0 && !in_fonttbl && !in_colortbl && !in_stylesheet && !in_info {
                    result.push(c);
                }
            }
            '"' => {
                // Quotes might be part of HYPERLINK URLs - skip if we're looking for space
                if skip_depth == 0 && !skip_to_space {
                    result.push(c);
                }
            }
            _ => {
                // Only output text if we're not in a skipped group
                if skip_depth == 0 {
                    result.push(c);
                }
            }
        }
    }

    // Clean up: remove leading/trailing whitespace and collapse multiple spaces
    let cleaned: String = result.trim().to_string();
    // Collapse multiple consecutive spaces into one
    let mut final_result = String::new();
    let mut prev_space = false;
    for c in cleaned.chars() {
        if c == ' ' {
            if !prev_space {
                final_result.push(c);
                prev_space = true;
            }
        } else {
            final_result.push(c);
            prev_space = false;
        }
    }
    final_result
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
    /// Text style (font size and family)
    text_style: TextStyle,
    /// Show keyboard shortcuts window
    show_shortcuts: bool,
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
            text_style: TextStyle::default(),
            show_shortcuts: false,
        }
    }

    fn new_document(&mut self) {
        self.document = Document::new();
        self.text_style = TextStyle::default();
    }

    pub fn open_file(&mut self, path: PathBuf) {
        let is_rtf = path.extension()
            .map(|e| e.to_string_lossy().to_lowercase() == "rtf")
            .unwrap_or(false);

        if is_rtf {
            // Read RTF and strip markup to view as plain text
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
                    self.recent_files.add(path);
                    self.save_recent_files();
                }
                Err(e) => eprintln!("failed to open RTF file: {}", e),
            }
        } else {
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
            .with_filter(vec!["txt".to_string(), "md".to_string(), "rtf".to_string()]);
        self.file_browser_mode = FileBrowserMode::Open;
        self.show_file_browser = true;
    }

    fn show_save_as_dialog(&mut self) {
        self.file_browser = FileBrowser::new(documents_dir());
        self.file_browser_mode = FileBrowserMode::Save;
        self.save_filename = self.document.title.clone();
        if !self.save_filename.ends_with(".txt")
            && !self.save_filename.ends_with(".md")
        {
            self.save_filename.push_str(".txt");
        }
        self.show_file_browser = true;
    }

    fn save_recent_files(&self) {
        let config_path = config_dir("slowwrite").join("recent.json");
        let _ = self.recent_files.save(&config_path);
    }

    fn handle_keyboard(&mut self, ctx: &Context) {
        // Consume Tab key and replace with 4 spaces in text editor
        consume_special_keys_with_tab(ctx, 4);

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
        // Calculate max height based on available screen space
        let screen_rect = ctx.screen_rect();
        let max_height = (screen_rect.height() - 80.0).max(200.0);

        egui::Window::new("about slowWrite")
            .collapsible(false)
            .resizable(false)
            .default_width(300.0)
            .max_height(max_height)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(max_height - 60.0).show(ui, |ui| {
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
                });
                ui.vertical_centered(|ui| {
                    if ui.button("ok").clicked() {
                        self.show_about = false;
                    }
                });
            });
    }

    fn render_shortcuts(&mut self, ctx: &Context) {
        // Calculate max height based on available screen space
        let screen_rect = ctx.screen_rect();
        let max_height = (screen_rect.height() - 80.0).max(200.0);

        egui::Window::new("keyboard shortcuts")
            .collapsible(false)
            .resizable(false)
            .default_width(320.0)
            .max_height(max_height)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(max_height - 60.0).show(ui, |ui| {
                    ui.heading("slowWrite shortcuts");
                    ui.add_space(8.0);

                    ui.label(egui::RichText::new("File Operations").strong());
                    ui.separator();
                    shortcut_row(ui, "⌘N", "New document");
                    shortcut_row(ui, "⌘O", "Open file");
                    shortcut_row(ui, "⌘S", "Save");
                    shortcut_row(ui, "⇧⌘S", "Save as");
                    shortcut_row(ui, "⌘W", "Close");
                    ui.add_space(8.0);

                    ui.label(egui::RichText::new("Editing").strong());
                    ui.separator();
                    shortcut_row(ui, "⌘Z", "Undo");
                    shortcut_row(ui, "⇧⌘Z", "Redo");
                    shortcut_row(ui, "⌘X", "Cut");
                    shortcut_row(ui, "⌘C", "Copy");
                    shortcut_row(ui, "⌘V", "Paste");
                    shortcut_row(ui, "⌘A", "Select all");
                    ui.add_space(8.0);

                    ui.label(egui::RichText::new("Navigation").strong());
                    ui.separator();
                    shortcut_row(ui, "⌥←", "Move word left");
                    shortcut_row(ui, "⌥→", "Move word right");
                    shortcut_row(ui, "⇧⌥→", "Select word right");
                    shortcut_row(ui, "Ctrl+B", "Move back one char");
                    shortcut_row(ui, "Ctrl+F", "Move forward one char");
                    shortcut_row(ui, "Ctrl+P", "Move up one line");
                    shortcut_row(ui, "Ctrl+N", "Move down one line");
                    ui.add_space(8.0);

                    ui.label(egui::RichText::new("Text Formatting").strong());
                    ui.separator();
                    shortcut_row(ui, "⌘B", "Bold");
                    shortcut_row(ui, "⌘I", "Italic");
                    shortcut_row(ui, "⌘U", "Underline");
                    ui.add_space(8.0);
                });
                ui.vertical_centered(|ui| {
                    if ui.button("ok").clicked() {
                        self.show_shortcuts = false;
                    }
                });
            });
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
                self.document.char_count(),
            );
            status_bar(ui, &status);
        });

        // Main editor area
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(0.0)))
            .show(ctx, |ui| {
                let available = ui.available_size();
                let line_number_width = 48.0;

                let base_font = if self.text_style.monospace {
                    egui::FontId::monospace(self.text_style.font_size)
                } else {
                    egui::FontId::proportional(self.text_style.font_size)
                };

                // Use egui's actual row height for this font so gutter matches TextEdit exactly
                let row_height = ui.fonts(|f| f.row_height(&base_font));

                ScrollArea::vertical().show(ui, |ui: &mut egui::Ui| {
                    ui.horizontal_top(|ui: &mut egui::Ui| {
                        // Line number gutter - paint directly, no individual widgets
                        let line_count = self.document.content.split('\n').count().max(1);
                        let gutter_height = line_count as f32 * row_height;
                        let (gutter_rect, _) = ui.allocate_exact_size(
                            egui::Vec2::new(line_number_width, gutter_height.max(available.y)),
                            egui::Sense::hover(),
                        );
                        let painter = ui.painter_at(gutter_rect);
                        painter.rect_filled(gutter_rect, 0.0, SlowColors::WHITE);

                        // Only paint visible line numbers
                        let clip = painter.clip_rect();
                        let first_visible = ((clip.min.y - gutter_rect.min.y) / row_height).floor().max(0.0) as usize;
                        let last_visible = ((clip.max.y - gutter_rect.min.y) / row_height).ceil().max(0.0) as usize;
                        let last_visible = last_visible.min(line_count);

                        // Width of widest number for right-alignment
                        let num_width = format!("{}", line_count).len();
                        for i in first_visible..last_visible {
                            let y = gutter_rect.min.y + i as f32 * row_height;
                            painter.text(
                                egui::Pos2::new(gutter_rect.max.x - 8.0, y),
                                egui::Align2::RIGHT_TOP,
                                format!("{:>width$}", i + 1, width = num_width),
                                egui::FontId::monospace(self.text_style.font_size * 0.85),
                                egui::Color32::GRAY,
                            );
                        }

                        // Separator line
                        painter.vline(
                            gutter_rect.max.x - 1.0,
                            gutter_rect.min.y..=gutter_rect.max.y,
                            egui::Stroke::new(1.0, egui::Color32::from_gray(200)),
                        );

                        // Text editor
                        let editor_width = (available.x - line_number_width - 8.0).max(100.0);
                        let response = ui.add_sized(
                            [editor_width, gutter_height.max(available.y)],
                            egui::TextEdit::multiline(&mut self.document.content)
                                .font(base_font)
                                .desired_width(editor_width)
                                .frame(false)
                                .margin(egui::Margin::symmetric(8.0, 0.0))
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

        if self.show_shortcuts {
            self.render_shortcuts(ctx);
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
