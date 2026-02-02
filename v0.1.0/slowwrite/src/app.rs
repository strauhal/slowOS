//! SlowWrite application
//! 
//! Main application state and UI.
//! Supports both macOS ⌘ shortcuts and emacs/vim Ctrl keybindings.

use crate::document::Document;
use crate::editor::Editor;
use egui::{Context, Key};
use slowcore::storage::{FileBrowser, RecentFiles, config_dir, documents_dir};
use slowcore::theme::{SlowColors, menu_bar};
use slowcore::widgets::status_bar;
use std::path::PathBuf;

/// Application state
pub struct SlowWriteApp {
    /// Current document
    document: Document,
    /// Editor state
    editor: Editor,
    /// Recent files list
    recent_files: RecentFiles,
    /// Whether to show the file browser
    show_file_browser: bool,
    /// File browser state
    file_browser: FileBrowser,
    /// Whether browsing for open or save
    file_browser_mode: FileBrowserMode,
    /// Save filename input
    save_filename: String,
    /// Show find/replace dialog
    show_find_replace: bool,
    /// Show about dialog
    show_about: bool,
    /// System clipboard (may fail on some platforms)
    #[allow(dead_code)]
    clipboard: Option<arboard::Clipboard>,
    /// Internal clipboard fallback — always works
    internal_clipboard: String,
}

#[derive(Clone, Copy, PartialEq)]
enum FileBrowserMode {
    Open,
    Save,
}

impl SlowWriteApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config_path = config_dir("slowwrite").join("recent.json");
        let recent_files = RecentFiles::load(&config_path).unwrap_or_else(|_| RecentFiles::new(10));
        
        Self {
            document: Document::new(),
            editor: Editor::new(),
            recent_files,
            show_file_browser: false,
            file_browser: FileBrowser::new(documents_dir())
                .with_filter(vec!["txt".to_string(), "md".to_string()]),
            file_browser_mode: FileBrowserMode::Open,
            save_filename: String::new(),
            show_find_replace: false,
            show_about: false,
            clipboard: arboard::Clipboard::new().ok(),
            internal_clipboard: String::new(),
        }
    }
    
    fn new_document(&mut self) {
        self.document = Document::new();
        self.editor = Editor::new();
    }
    
    fn open_file(&mut self, path: PathBuf) {
        match Document::open(path.clone()) {
            Ok(doc) => {
                self.document = doc;
                self.editor = Editor::new();
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
        self.save_filename = self.document.meta.title.clone();
        if self.save_filename.is_empty() {
            self.save_filename = "untitled.txt".to_string();
        } else if !self.save_filename.ends_with(".txt") && !self.save_filename.ends_with(".md") {
            self.save_filename.push_str(".txt");
        }
        self.show_file_browser = true;
    }
    
    fn save_recent_files(&self) {
        let config_path = config_dir("slowwrite").join("recent.json");
        let _ = self.recent_files.save(&config_path);
    }
    
    // ---------------------------------------------------------------
    // Clipboard operations — hardened with retry + internal fallback
    // ---------------------------------------------------------------
    
    fn copy(&mut self) {
        if let Some(text) = self.editor.selected_text(&self.document) {
            if text.is_empty() { return; }
            // Always store in internal clipboard
            self.internal_clipboard = text.clone();
            
            // Try system clipboard — create fresh handle each time
            if let Ok(mut cb) = arboard::Clipboard::new() {
                let _ = cb.set_text(&text);
            }
        }
    }
    
    fn cut(&mut self) {
        self.copy();
        if self.editor.cursor.has_selection() {
            self.editor.delete(&mut self.document);
        }
    }
    
    fn paste(&mut self) {
        // Try system clipboard first with a fresh handle
        let text = arboard::Clipboard::new().ok()
            .and_then(|mut cb| cb.get_text().ok())
            .filter(|t| !t.is_empty())
            .or_else(|| {
                if !self.internal_clipboard.is_empty() {
                    Some(self.internal_clipboard.clone())
                } else {
                    None
                }
            });
        
        if let Some(t) = text {
            self.editor.insert_text(&mut self.document, &t);
        }
    }
    
    // ---------------------------------------------------------------
    // Keyboard handling
    // ---------------------------------------------------------------
    
    fn handle_keyboard(&mut self, ctx: &Context) {
        // Check for Tab key press and handle it directly, before egui can use it for navigation
        let tab_pressed = ctx.input(|i| i.key_pressed(Key::Tab));
        if tab_pressed {
            self.editor.insert_text(&mut self.document, "    ");
            self.editor.reset_blink();
        }

        // Read key states BEFORE consuming events
        let (modifiers, key_n, key_o, key_s, key_f, key_z, key_c, key_x, key_v, key_a) = ctx.input(|i| {
            (
                i.modifiers,
                i.key_pressed(Key::N),
                i.key_pressed(Key::O),
                i.key_pressed(Key::S),
                i.key_pressed(Key::F),
                i.key_pressed(Key::Z),
                i.key_pressed(Key::C),
                i.key_pressed(Key::X),
                i.key_pressed(Key::V),
                i.key_pressed(Key::A),
            )
        });

        // Consume Tab and command key events so egui doesn't use them for widget navigation
        ctx.input_mut(|i| {
            // Consume Tab events
            i.events.retain(|e| !matches!(e, egui::Event::Key { key: Key::Tab, .. }));
            // Consume command key events to prevent egui widget navigation
            if i.modifiers.command {
                i.events.retain(|e| !matches!(e,
                    egui::Event::Key { key: Key::C | Key::V | Key::X | Key::A | Key::Z | Key::N | Key::O | Key::S | Key::F, .. }
                ));
            }
        });

        // =============================================================
        // ⌘ (Command) shortcuts — standard macOS / desktop shortcuts
        // On Mac: ⌘ key. On Linux: Ctrl key (via egui's mapping).
        // =============================================================
        if modifiers.command {
            if key_n {
                self.new_document();
            }
            if key_o {
                self.show_open_dialog();
            }
            if key_s {
                if modifiers.shift {
                    self.show_save_as_dialog();
                } else {
                    self.save_document();
                }
            }
            if key_f {
                self.show_find_replace = !self.show_find_replace;
            }

            // Edit
            if key_z {
                if modifiers.shift {
                    if let Some(pos) = self.document.redo() {
                        self.editor.cursor.pos = pos;
                        self.editor.cursor.clear_selection();
                    }
                } else if let Some(pos) = self.document.undo() {
                    self.editor.cursor.pos = pos;
                    self.editor.cursor.clear_selection();
                }
            }
            if key_c {
                self.copy();
            }
            if key_x {
                self.cut();
            }
            if key_v {
                self.paste();
            }
            if key_a {
                self.editor.select_all(&self.document);
            }
        }

        ctx.input(|i| {
            
            // =============================================================
            // Ctrl keybindings — emacs / vim home-row navigation
            // On Mac: physical Ctrl key (separate from ⌘).
            // On Linux: Ctrl = Command, so this block is skipped
            //           (the command block above handles Ctrl on Linux).
            //
            // Ctrl+F  → forward (right)     Ctrl+B  → backward (left)
            // Ctrl+P  → previous (up)       Ctrl+N  → next (down)
            // Ctrl+A  → beginning of line   Ctrl+E  → end of line
            // Ctrl+D  → delete forward      Ctrl+H  → delete backward
            // Ctrl+K  → kill to end of line
            // =============================================================
            if modifiers.ctrl && !modifiers.command {
                if i.key_pressed(Key::F) {
                    self.editor.move_right(&self.document, modifiers.shift);
                    self.editor.reset_blink();
                }
                if i.key_pressed(Key::B) {
                    self.editor.move_left(&self.document, modifiers.shift);
                    self.editor.reset_blink();
                }
                if i.key_pressed(Key::P) {
                    self.editor.move_up(&self.document, modifiers.shift);
                    self.editor.reset_blink();
                }
                if i.key_pressed(Key::N) {
                    self.editor.move_down(&self.document, modifiers.shift);
                    self.editor.reset_blink();
                }
                if i.key_pressed(Key::A) {
                    self.editor.move_to_line_start(&self.document, modifiers.shift);
                    self.editor.reset_blink();
                }
                if i.key_pressed(Key::E) {
                    self.editor.move_to_line_end(&self.document, modifiers.shift);
                    self.editor.reset_blink();
                }
                if i.key_pressed(Key::D) {
                    self.editor.delete(&mut self.document);
                    self.editor.reset_blink();
                }
                if i.key_pressed(Key::H) {
                    self.editor.backspace(&mut self.document);
                    self.editor.reset_blink();
                }
                if i.key_pressed(Key::K) {
                    // Kill to end of line — store killed text in internal clipboard
                    if let Some(killed) = self.editor.kill_to_line_end(&mut self.document) {
                        self.internal_clipboard = killed.clone();
                        if let Some(ref mut cb) = self.clipboard {
                            let _ = cb.set_text(&killed);
                        }
                    }
                    self.editor.reset_blink();
                }
            }
            
            // =============================================================
            // Arrow key navigation
            // =============================================================
            if i.key_pressed(Key::ArrowLeft) {
                if modifiers.alt || (modifiers.ctrl && modifiers.command) {
                    self.editor.move_word_left(&self.document, modifiers.shift);
                } else if !modifiers.ctrl || modifiers.command {
                    // Skip if Ctrl is being used for emacs bindings
                    self.editor.move_left(&self.document, modifiers.shift);
                }
                self.editor.reset_blink();
            }
            if i.key_pressed(Key::ArrowRight) {
                if modifiers.alt || (modifiers.ctrl && modifiers.command) {
                    self.editor.move_word_right(&self.document, modifiers.shift);
                } else if !modifiers.ctrl || modifiers.command {
                    self.editor.move_right(&self.document, modifiers.shift);
                }
                self.editor.reset_blink();
            }
            if i.key_pressed(Key::ArrowUp) {
                self.editor.move_up(&self.document, modifiers.shift);
                self.editor.reset_blink();
            }
            if i.key_pressed(Key::ArrowDown) {
                self.editor.move_down(&self.document, modifiers.shift);
                self.editor.reset_blink();
            }
            if i.key_pressed(Key::Home) {
                self.editor.move_to_line_start(&self.document, modifiers.shift);
                self.editor.reset_blink();
            }
            if i.key_pressed(Key::End) {
                self.editor.move_to_line_end(&self.document, modifiers.shift);
                self.editor.reset_blink();
            }
            
            // =============================================================
            // Editing keys
            // =============================================================
            if i.key_pressed(Key::Backspace) {
                self.editor.backspace(&mut self.document);
                self.editor.reset_blink();
            }
            if i.key_pressed(Key::Delete) {
                self.editor.delete(&mut self.document);
                self.editor.reset_blink();
            }
            if i.key_pressed(Key::Enter) {
                self.editor.insert_text(&mut self.document, "\n");
                self.editor.reset_blink();
            }
            // Tab is handled at the top of handle_keyboard()
            
            // =============================================================
            // Text input — only when no modifier keys are held
            // =============================================================
            for event in &i.events {
                if let egui::Event::Text(text) = event {
                    if !modifiers.command && !modifiers.ctrl {
                        self.editor.insert_text(&mut self.document, text);
                        self.editor.reset_blink();
                    }
                }
            }
        });
    }
    
    // ---------------------------------------------------------------
    // UI rendering
    // ---------------------------------------------------------------
    
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
            
            ui.menu_button("edit", |ui| {
                if ui.button("undo       ⌘z").clicked() {
                    if let Some(pos) = self.document.undo() {
                        self.editor.cursor.pos = pos;
                        self.editor.cursor.clear_selection();
                    }
                    ui.close_menu();
                }
                if ui.button("redo       ⇧⌘z").clicked() {
                    if let Some(pos) = self.document.redo() {
                        self.editor.cursor.pos = pos;
                        self.editor.cursor.clear_selection();
                    }
                    ui.close_menu();
                }
                
                ui.separator();
                
                if ui.button("cut        ⌘x").clicked() {
                    self.cut();
                    ui.close_menu();
                }
                if ui.button("copy       ⌘c").clicked() {
                    self.copy();
                    ui.close_menu();
                }
                if ui.button("paste      ⌘v").clicked() {
                    self.paste();
                    ui.close_menu();
                }
                
                ui.separator();
                
                if ui.button("select all ⌘a").clicked() {
                    self.editor.select_all(&self.document);
                    ui.close_menu();
                }
                
                ui.separator();
                
                if ui.button("find...    ⌘f").clicked() {
                    self.show_find_replace = true;
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
            .default_width(400.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("location:");
                    ui.label(self.file_browser.current_dir.to_string_lossy().to_string());
                });
                
                ui.separator();
                
                egui::ScrollArea::vertical()
                    .max_height(300.0)
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
                                    let path = self.file_browser.current_dir.join(&self.save_filename);
                                    self.save_document_as(path);
                                    self.show_file_browser = false;
                                }
                            }
                        }
                    }
                });
            });
    }
    
    fn render_find_replace(&mut self, ctx: &Context) {
        egui::Window::new("find & replace")
            .collapsible(false)
            .resizable(false)
            .default_width(350.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("find:");
                    if ui.text_edit_singleline(&mut self.editor.find_query).changed() {
                        self.editor.find(&self.document);
                    }
                });
                
                ui.horizontal(|ui| {
                    ui.label("replace:");
                    ui.text_edit_singleline(&mut self.editor.replace_query);
                });
                
                ui.horizontal(|ui| {
                    let count = self.editor.find_results.len();
                    if count > 0 {
                        let current = self.editor.current_find_index.map(|i| i + 1).unwrap_or(0);
                        ui.label(format!("{} of {} matches", current, count));
                    } else if !self.editor.find_query.is_empty() {
                        ui.label("no matches");
                    }
                });
                
                ui.horizontal(|ui| {
                    if ui.button("find next").clicked() {
                        self.editor.find_next();
                    }
                    if ui.button("replace").clicked() {
                        self.editor.replace_current(&mut self.document);
                    }
                    if ui.button("replace all").clicked() {
                        self.editor.replace_all(&mut self.document);
                    }
                    if ui.button("close").clicked() {
                        self.show_find_replace = false;
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
                ui.label("  undo/redo, find/replace");
                ui.label("  emacs/vim keybindings (Ctrl+)");
                ui.add_space(4.0);
                ui.label("frameworks:");
                ui.label("  egui/eframe (MIT), ropey (MIT)");
                ui.label("  arboard (MIT), unicode-segmentation");
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
        // Update cursor blink
        ctx.input(|i| {
            self.editor.update(i.stable_dt as f64);
        });
        
        // Handle keyboard input
        self.handle_keyboard(ctx);
        
        // Request repaint for cursor blink
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
        
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
            let (line, col) = self.document.char_to_line_col(self.editor.cursor.pos);
            let status = format!(
                "line {}, col {}  |  {} words, {} chars",
                line + 1,
                col + 1,
                self.document.meta.word_count,
                self.document.meta.char_count
            );
            status_bar(ui, &status);
        });
        
        // Main editor area
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE))
            .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();
                self.editor.render(ui, &self.document, rect);
                self.editor.ensure_cursor_visible(&self.document, rect.height());
            });
        
        // Dialogs
        if self.show_file_browser {
            self.render_file_browser(ctx);
        }
        
        if self.show_find_replace {
            self.render_find_replace(ctx);
        }
        
        if self.show_about {
            self.render_about(ctx);
        }
    }
}
