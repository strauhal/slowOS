//! SlowWrite v0.2.0 — word processor with rich text editing
//!
//! Per-character styling: each character can have its own font, size,
//! weight, and decorations. Users can double-click-drag to select by
//! word, and Tab inserts spaces.

use crate::rich_text::{CharStyle, FontFamily, RichDocument, load_rich_document, save_rich_document, save_as_rtf, load_rtf};
use egui::{
    Align2, Color32, Context, FontId, Key, Painter, Pos2, Rect, Response, Sense, Stroke, Vec2,
};
use slowcore::dither;
use slowcore::storage::{config_dir, documents_dir, FileBrowser, RecentFiles};
use slowcore::text_edit::word_boundaries;
use slowcore::theme::{consume_special_keys_with_tab, menu_bar, SlowColors};
use slowcore::widgets::status_bar;
use std::path::PathBuf;
use std::time::Instant;

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
                let mut _num = String::new();
                while let Some(&next) = chars.peek() {
                    if next.is_ascii_digit() || next == '-' { _num.push(chars.next().unwrap()); }
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

/// Double-click timing
const DOUBLE_CLICK_MS: u128 = 400;

#[derive(Clone, Copy, PartialEq)]
enum FileBrowserMode {
    Open,
    Save,
}

/// Editor cursor and selection state
struct EditorState {
    /// Cursor position as char index
    cursor: usize,
    /// Selection anchor (char index), None if no selection
    sel_anchor: Option<usize>,
    /// Whether we're in word-selection drag mode (double-click held)
    word_select_active: bool,
    /// Anchor word boundaries (char indices) for word-select drag
    word_anchor_start: usize,
    word_anchor_end: usize,
    /// Last click time for double-click detection
    last_click_time: Instant,
    /// Last click char position
    last_click_char: usize,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            cursor: 0,
            sel_anchor: None,
            word_select_active: false,
            word_anchor_start: 0,
            word_anchor_end: 0,
            last_click_time: Instant::now(),
            last_click_char: 0,
        }
    }
}

impl EditorState {
    fn has_selection(&self) -> bool {
        self.sel_anchor.is_some() && self.sel_anchor != Some(self.cursor)
    }

    fn selection_range(&self) -> Option<(usize, usize)> {
        self.sel_anchor.map(|anchor| {
            let start = anchor.min(self.cursor);
            let end = anchor.max(self.cursor);
            (start, end)
        })
    }

    fn clear_selection(&mut self) {
        self.sel_anchor = None;
        self.word_select_active = false;
    }
}

/// Editor mode: plain text (default) or rich text
#[derive(Clone, Copy, PartialEq)]
pub enum EditorMode {
    PlainText,
    RichText,
}

/// Application state
pub struct SlowWriteApp {
    doc: RichDocument,
    file_path: Option<PathBuf>,
    file_title: String,
    modified: bool,
    editor: EditorState,
    recent_files: RecentFiles,
    show_file_browser: bool,
    file_browser: FileBrowser,
    file_browser_mode: FileBrowserMode,
    save_filename: String,
    show_about: bool,
    show_close_confirm: bool,
    close_confirmed: bool,
    show_shortcuts: bool,
    /// Show the formatting toolbar (only in rich text mode)
    show_toolbar: bool,
    /// Font size options for the toolbar dropdown
    font_sizes: Vec<f32>,
    /// Current editor mode
    mode: EditorMode,
    /// Internal clipboard (fallback when system clipboard is unavailable)
    internal_clipboard: String,
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
            editor: EditorState::default(),
            recent_files,
            show_file_browser: false,
            file_browser: FileBrowser::new(documents_dir())
                .with_filter(vec![
                    "txt".to_string(),
                    "md".to_string(),
                    "rtf".to_string(),
                    "swd".to_string(),
                ]),
            file_browser_mode: FileBrowserMode::Open,
            save_filename: String::new(),
            show_about: false,
            show_close_confirm: false,
            close_confirmed: false,
            show_shortcuts: false,
            show_toolbar: true,
            font_sizes: vec![8.0, 10.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 32.0, 36.0, 48.0, 64.0, 72.0],
            mode: EditorMode::PlainText,
            internal_clipboard: String::new(),
        }
    }

    fn new_document(&mut self) {
        self.doc = RichDocument::new();
        self.file_path = None;
        self.file_title = "untitled".to_string();
        self.modified = false;
        self.editor = EditorState::default();
    }

    pub fn open_file(&mut self, path: PathBuf) {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        match ext.as_str() {
            "swd" => {
                match std::fs::read_to_string(&path) {
                    Ok(json) => {
                        if let Some(doc) = load_rich_document(&json) {
                            self.doc = doc;
                        } else {
                            self.doc = RichDocument::from_plain_text(json);
                        }
                        self.mode = EditorMode::RichText;
                    }
                    Err(e) => {
                        eprintln!("failed to open: {}", e);
                        return;
                    }
                }
            }
            "rtf" => {
                match std::fs::read_to_string(&path) {
                    Ok(raw) => {
                        if let Some(doc) = load_rtf(&raw) {
                            self.doc = doc;
                            self.mode = EditorMode::RichText;
                        } else {
                            // Fallback: strip RTF and load as plain
                            let plain = strip_rtf(&raw);
                            self.doc = RichDocument::from_plain_text(plain);
                        }
                    }
                    Err(e) => {
                        eprintln!("failed to open RTF: {}", e);
                        return;
                    }
                }
            }
            _ => {
                match std::fs::read_to_string(&path) {
                    Ok(text) => {
                        self.doc = RichDocument::from_plain_text(text);
                    }
                    Err(e) => {
                        eprintln!("failed to open: {}", e);
                        return;
                    }
                }
            }
        }

        self.file_title = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or("untitled".to_string());
        self.file_path = Some(path.clone());
        self.modified = false;
        self.editor = EditorState::default();
        self.recent_files.add(path);
        self.save_recent_files();
    }

    fn save_content_for_path(&self, path: &std::path::Path) -> String {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "swd" => save_rich_document(&self.doc),
            "rtf" => save_as_rtf(&self.doc),
            _ => self.doc.text.clone(), // .txt, .md, etc.
        }
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
            "rtf".to_string(),
            "swd".to_string(),
        ]);
        self.file_browser_mode = FileBrowserMode::Open;
        self.show_file_browser = true;
    }

    fn show_save_as_dialog(&mut self) {
        self.file_browser = FileBrowser::new(documents_dir());
        self.file_browser_mode = FileBrowserMode::Save;
        self.save_filename = self.file_title.clone();
        let has_ext = self.save_filename.ends_with(".txt")
            || self.save_filename.ends_with(".md")
            || self.save_filename.ends_with(".swd")
            || self.save_filename.ends_with(".rtf");
        if !has_ext {
            match self.mode {
                EditorMode::RichText => self.save_filename.push_str(".rtf"),
                EditorMode::PlainText => self.save_filename.push_str(".txt"),
            }
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

    /// Delete the currently selected text
    fn delete_selection(&mut self) {
        if let Some((start, end)) = self.editor.selection_range() {
            let byte_start = self.doc.char_to_byte(start);
            let byte_end = self.doc.char_to_byte(end);
            self.doc.text.replace_range(byte_start..byte_end, "");
            let drain_end = end.min(self.doc.styles.len());
            let drain_start = start.min(drain_end);
            self.doc.styles.drain(drain_start..drain_end);
            self.editor.cursor = start;
            self.editor.clear_selection();
            self.modified = true;
        }
    }

    /// Insert text at cursor with current style
    fn insert_text(&mut self, text: &str) {
        if self.editor.has_selection() {
            self.delete_selection();
        }
        let byte_pos = self.doc.char_to_byte(self.editor.cursor);
        self.doc.text.insert_str(byte_pos, text);
        let new_chars: Vec<CharStyle> = text
            .chars()
            .map(|_| self.doc.cursor_style.clone())
            .collect();
        let insert_count = new_chars.len();
        for (i, style) in new_chars.into_iter().enumerate() {
            let pos = self.editor.cursor + i;
            if pos <= self.doc.styles.len() {
                self.doc.styles.insert(pos, style);
            } else {
                self.doc.styles.push(style);
            }
        }
        self.editor.cursor += insert_count;
        self.modified = true;
    }

    fn handle_keyboard(&mut self, ctx: &Context) {
        consume_special_keys_with_tab(ctx, 4);

        let mut actions: Vec<Box<dyn FnOnce(&mut Self)>> = Vec::new();

        ctx.input(|i| {
            let cmd = i.modifiers.command;
            let shift = i.modifiers.shift;

            if cmd && i.key_pressed(Key::N) {
                actions.push(Box::new(|s| s.new_document()));
            }
            if cmd && i.key_pressed(Key::O) {
                actions.push(Box::new(|s| s.show_open_dialog()));
            }
            if cmd && i.key_pressed(Key::S) {
                if shift {
                    actions.push(Box::new(|s| s.show_save_as_dialog()));
                } else {
                    actions.push(Box::new(|s| s.save_document()));
                }
            }
            if cmd && i.key_pressed(Key::B) {
                actions.push(Box::new(|s| {
                    if let Some((start, end)) = s.editor.selection_range() {
                        s.doc.toggle_bold(start, end);
                        s.modified = true;
                    }
                    s.doc.cursor_style.bold = !s.doc.cursor_style.bold;
                }));
            }
            if cmd && i.key_pressed(Key::I) {
                actions.push(Box::new(|s| {
                    if let Some((start, end)) = s.editor.selection_range() {
                        s.doc.toggle_italic(start, end);
                        s.modified = true;
                    }
                    s.doc.cursor_style.italic = !s.doc.cursor_style.italic;
                }));
            }
            if cmd && i.key_pressed(Key::U) {
                actions.push(Box::new(|s| {
                    if let Some((start, end)) = s.editor.selection_range() {
                        s.doc.toggle_underline(start, end);
                        s.modified = true;
                    }
                    s.doc.cursor_style.underline = !s.doc.cursor_style.underline;
                }));
            }
            if cmd && i.key_pressed(Key::A) {
                actions.push(Box::new(|s| {
                    s.editor.sel_anchor = Some(0);
                    s.editor.cursor = s.doc.char_count();
                }));
            }
            if i.key_pressed(Key::Backspace) {
                actions.push(Box::new(|s| {
                    if s.editor.has_selection() {
                        s.delete_selection();
                    } else if s.editor.cursor > 0 {
                        let byte_start = s.doc.char_to_byte(s.editor.cursor - 1);
                        let byte_end = s.doc.char_to_byte(s.editor.cursor);
                        s.doc.text.replace_range(byte_start..byte_end, "");
                        if s.editor.cursor - 1 < s.doc.styles.len() {
                            s.doc.styles.remove(s.editor.cursor - 1);
                        }
                        s.editor.cursor -= 1;
                        s.modified = true;
                    }
                }));
            }
            if i.key_pressed(Key::Delete) {
                actions.push(Box::new(|s| {
                    if s.editor.has_selection() {
                        s.delete_selection();
                    } else if s.editor.cursor < s.doc.char_count() {
                        let byte_start = s.doc.char_to_byte(s.editor.cursor);
                        let byte_end = s.doc.char_to_byte(s.editor.cursor + 1);
                        s.doc.text.replace_range(byte_start..byte_end, "");
                        if s.editor.cursor < s.doc.styles.len() {
                            s.doc.styles.remove(s.editor.cursor);
                        }
                        s.modified = true;
                    }
                }));
            }
            if i.key_pressed(Key::ArrowLeft) {
                let word_mode = i.modifiers.alt;
                let extend = shift;
                actions.push(Box::new(move |s| {
                    if !extend && s.editor.has_selection() {
                        let (start, _) = s.editor.selection_range().unwrap();
                        s.editor.cursor = start;
                        s.editor.clear_selection();
                    } else if s.editor.cursor > 0 {
                        if !extend { s.editor.clear_selection(); }
                        else if s.editor.sel_anchor.is_none() {
                            s.editor.sel_anchor = Some(s.editor.cursor);
                        }
                        if word_mode {
                            let byte_pos = s.doc.char_to_byte(s.editor.cursor);
                            let (ws, _) = word_boundaries(&s.doc.text, byte_pos.saturating_sub(1));
                            s.editor.cursor = s.doc.byte_to_char(ws);
                        } else {
                            s.editor.cursor -= 1;
                        }
                    }
                }));
            }
            if i.key_pressed(Key::ArrowRight) {
                let word_mode = i.modifiers.alt;
                let extend = shift;
                actions.push(Box::new(move |s| {
                    if !extend && s.editor.has_selection() {
                        let (_, end) = s.editor.selection_range().unwrap();
                        s.editor.cursor = end;
                        s.editor.clear_selection();
                    } else if s.editor.cursor < s.doc.char_count() {
                        if !extend { s.editor.clear_selection(); }
                        else if s.editor.sel_anchor.is_none() {
                            s.editor.sel_anchor = Some(s.editor.cursor);
                        }
                        if word_mode {
                            let byte_pos = s.doc.char_to_byte(s.editor.cursor);
                            let (_, we) = word_boundaries(&s.doc.text, byte_pos);
                            s.editor.cursor = s.doc.byte_to_char(we);
                        } else {
                            s.editor.cursor += 1;
                        }
                    }
                }));
            }
            if i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::ArrowDown) {
                let going_up = i.key_pressed(Key::ArrowUp);
                let extend = shift;
                actions.push(Box::new(move |s| {
                    if !extend { s.editor.clear_selection(); }
                    else if s.editor.sel_anchor.is_none() {
                        s.editor.sel_anchor = Some(s.editor.cursor);
                    }
                    let byte_pos = s.doc.char_to_byte(s.editor.cursor);
                    let before = &s.doc.text[..byte_pos];
                    let line_start = before.rfind('\n').map(|p| p + 1).unwrap_or(0);
                    let col = byte_pos - line_start;
                    if going_up {
                        if line_start > 0 {
                            let prev_line_end = line_start - 1;
                            let prev_line_start = s.doc.text[..prev_line_end].rfind('\n').map(|p| p + 1).unwrap_or(0);
                            let prev_line_len = prev_line_end - prev_line_start;
                            let target_byte = prev_line_start + col.min(prev_line_len);
                            s.editor.cursor = s.doc.byte_to_char(target_byte);
                        }
                    } else {
                        let line_end = s.doc.text[byte_pos..].find('\n').map(|p| byte_pos + p);
                        if let Some(le) = line_end {
                            let next_line_start = le + 1;
                            let next_line_end = s.doc.text[next_line_start..].find('\n')
                                .map(|p| next_line_start + p)
                                .unwrap_or(s.doc.text.len());
                            let next_line_len = next_line_end - next_line_start;
                            let target_byte = next_line_start + col.min(next_line_len);
                            s.editor.cursor = s.doc.byte_to_char(target_byte);
                        }
                    }
                }));
            }
            if i.key_pressed(Key::Enter) {
                actions.push(Box::new(|s| s.insert_text("\n")));
            }
            // Text input events (skip when command modifier is held for shortcuts)
            if !cmd {
                for event in &i.events {
                    if let egui::Event::Text(text) = event {
                        let text = text.clone();
                        actions.push(Box::new(move |s| s.insert_text(&text)));
                    }
                }
            }
            // Copy
            if cmd && i.key_pressed(Key::C) {
                actions.push(Box::new(|s| {
                    if let Some((start, end)) = s.editor.selection_range() {
                        let byte_start = s.doc.char_to_byte(start);
                        let byte_end = s.doc.char_to_byte(end);
                        if byte_end <= s.doc.text.len() {
                            let selected = s.doc.text[byte_start..byte_end].to_string();
                            s.internal_clipboard = selected;
                        }
                    }
                }));
            }
            // Cut
            if cmd && i.key_pressed(Key::X) {
                actions.push(Box::new(|s| {
                    if let Some((start, end)) = s.editor.selection_range() {
                        let byte_start = s.doc.char_to_byte(start);
                        let byte_end = s.doc.char_to_byte(end);
                        if byte_end <= s.doc.text.len() {
                            let selected = s.doc.text[byte_start..byte_end].to_string();
                            s.internal_clipboard = selected;
                            s.delete_selection();
                        }
                    }
                }));
            }
            // Paste
            if cmd && i.key_pressed(Key::V) {
                actions.push(Box::new(|s| {
                    // Try system clipboard first, fall back to internal
                    let text = arboard::Clipboard::new().ok()
                        .and_then(|mut c| c.get_text().ok())
                        .unwrap_or_else(|| s.internal_clipboard.clone());
                    if !text.is_empty() {
                        s.insert_text(&text);
                    }
                }));
            }
        });

        let clipboard_before = self.internal_clipboard.clone();
        for action in actions {
            action(self);
        }
        // If clipboard changed this frame, propagate to system
        if self.internal_clipboard != clipboard_before && !self.internal_clipboard.is_empty() {
            let clip = self.internal_clipboard.clone();
            ctx.output_mut(|o| o.copied_text = clip.clone());
            if let Ok(mut cb) = arboard::Clipboard::new() {
                let _ = cb.set_text(&clip);
            }
        }
    }

    fn render_menu_bar(&mut self, ui: &mut egui::Ui) {
        menu_bar(ui, |ui| {
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
                    if let Some((start, end)) = self.editor.selection_range() {
                        let byte_start = self.doc.char_to_byte(start);
                        let byte_end = self.doc.char_to_byte(end);
                        if byte_end <= self.doc.text.len() {
                            let selected = self.doc.text[byte_start..byte_end].to_string();
                            self.internal_clipboard = selected;
                            self.delete_selection();
                        }
                    }
                    ui.close_menu();
                }
                if ui.button("copy       \u{2318}c").clicked() {
                    if let Some((start, end)) = self.editor.selection_range() {
                        let byte_start = self.doc.char_to_byte(start);
                        let byte_end = self.doc.char_to_byte(end);
                        if byte_end <= self.doc.text.len() {
                            let selected = self.doc.text[byte_start..byte_end].to_string();
                            self.internal_clipboard = selected.clone();
                            ui.ctx().output_mut(|o| o.copied_text = selected.clone());
                            if let Ok(mut cb) = arboard::Clipboard::new() {
                                let _ = cb.set_text(&selected);
                            }
                        }
                    }
                    ui.close_menu();
                }
                if ui.button("paste      \u{2318}v").clicked() {
                    let text = arboard::Clipboard::new().ok()
                        .and_then(|mut c| c.get_text().ok())
                        .unwrap_or_else(|| self.internal_clipboard.clone());
                    if !text.is_empty() {
                        self.insert_text(&text);
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("select all \u{2318}a").clicked() {
                    self.editor.sel_anchor = Some(0);
                    self.editor.cursor = self.doc.char_count();
                    ui.close_menu();
                }
            });

            ui.menu_button("view", |ui| {
                let plain_label = if self.mode == EditorMode::PlainText { "> plain text" } else { "  plain text" };
                let rich_label = if self.mode == EditorMode::RichText { "> rich text" } else { "  rich text" };
                if ui.button(plain_label).clicked() {
                    self.mode = EditorMode::PlainText;
                    ui.close_menu();
                }
                if ui.button(rich_label).clicked() {
                    self.mode = EditorMode::RichText;
                    self.show_toolbar = true;
                    ui.close_menu();
                }
            });

            if self.mode == EditorMode::RichText {
            ui.menu_button("format", |ui| {
                if ui.button("bold          \u{2318}b").clicked() {
                    if let Some((start, end)) = self.editor.selection_range() {
                        self.doc.toggle_bold(start, end);
                        self.modified = true;
                    }
                    self.doc.cursor_style.bold = !self.doc.cursor_style.bold;
                    ui.close_menu();
                }
                if ui.button("italic        \u{2318}i").clicked() {
                    if let Some((start, end)) = self.editor.selection_range() {
                        self.doc.toggle_italic(start, end);
                        self.modified = true;
                    }
                    self.doc.cursor_style.italic = !self.doc.cursor_style.italic;
                    ui.close_menu();
                }
                if ui.button("underline     \u{2318}u").clicked() {
                    if let Some((start, end)) = self.editor.selection_range() {
                        self.doc.toggle_underline(start, end);
                        self.modified = true;
                    }
                    self.doc.cursor_style.underline = !self.doc.cursor_style.underline;
                    ui.close_menu();
                }
                if ui.button("strikethrough").clicked() {
                    if let Some((start, end)) = self.editor.selection_range() {
                        self.doc.toggle_strikethrough(start, end);
                        self.modified = true;
                    }
                    self.doc.cursor_style.strikethrough = !self.doc.cursor_style.strikethrough;
                    ui.close_menu();
                }
                ui.separator();
                ui.menu_button("font size", |ui| {
                    for &size in &self.font_sizes.clone() {
                        let label = format!("{}pt", size as u32);
                        if ui.button(&label).clicked() {
                            if let Some((start, end)) = self.editor.selection_range() {
                                self.doc.set_font_size(start, end, size);
                                self.modified = true;
                            }
                            self.doc.cursor_style.font_size = size;
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button("font family", |ui| {
                    if ui.button("proportional").clicked() {
                        if let Some((start, end)) = self.editor.selection_range() {
                            self.doc.set_font_family(start, end, FontFamily::Proportional);
                            self.modified = true;
                        }
                        self.doc.cursor_style.font_family = FontFamily::Proportional;
                        ui.close_menu();
                    }
                    if ui.button("monospace").clicked() {
                        if let Some((start, end)) = self.editor.selection_range() {
                            self.doc.set_font_family(start, end, FontFamily::Monospace);
                            self.modified = true;
                        }
                        self.doc.cursor_style.font_family = FontFamily::Monospace;
                        ui.close_menu();
                    }
                });
                ui.separator();
                let toolbar_label = if self.show_toolbar { "hide toolbar" } else { "show toolbar" };
                if ui.button(toolbar_label).clicked() {
                    self.show_toolbar = !self.show_toolbar;
                    ui.close_menu();
                }
            });
            } // end if RichText

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

    /// Draw the formatting toolbar
    fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        if !self.show_toolbar || self.mode == EditorMode::PlainText {
            return;
        }
        egui::Frame::none()
            .fill(SlowColors::WHITE)
            .stroke(Stroke::new(1.0, SlowColors::BLACK))
            .inner_margin(egui::Margin::symmetric(6.0, 3.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let bold_sel = self.doc.cursor_style.bold;
                    if ui.selectable_label(bold_sel, egui::RichText::new("B").strong()).clicked() {
                        if let Some((s, e)) = self.editor.selection_range() {
                            self.doc.toggle_bold(s, e);
                            self.modified = true;
                        }
                        self.doc.cursor_style.bold = !self.doc.cursor_style.bold;
                    }
                    let italic_sel = self.doc.cursor_style.italic;
                    if ui.selectable_label(italic_sel, egui::RichText::new("I").italics()).clicked() {
                        if let Some((s, e)) = self.editor.selection_range() {
                            self.doc.toggle_italic(s, e);
                            self.modified = true;
                        }
                        self.doc.cursor_style.italic = !self.doc.cursor_style.italic;
                    }
                    let underline_sel = self.doc.cursor_style.underline;
                    if ui.selectable_label(underline_sel, egui::RichText::new("U").underline()).clicked() {
                        if let Some((s, e)) = self.editor.selection_range() {
                            self.doc.toggle_underline(s, e);
                            self.modified = true;
                        }
                        self.doc.cursor_style.underline = !self.doc.cursor_style.underline;
                    }
                    let strike_sel = self.doc.cursor_style.strikethrough;
                    if ui.selectable_label(strike_sel, egui::RichText::new("S").strikethrough()).clicked() {
                        if let Some((s, e)) = self.editor.selection_range() {
                            self.doc.toggle_strikethrough(s, e);
                            self.modified = true;
                        }
                        self.doc.cursor_style.strikethrough = !self.doc.cursor_style.strikethrough;
                    }
                    ui.separator();
                    ui.label(format!("{}pt", self.doc.cursor_style.font_size as u32));
                    if ui.small_button("+").clicked() {
                        if let Some((s, e)) = self.editor.selection_range() {
                            self.doc.increase_font_size(s, e);
                            self.modified = true;
                        }
                        self.doc.cursor_style.font_size = (self.doc.cursor_style.font_size + 2.0).min(72.0);
                    }
                    if ui.small_button("\u{2212}").clicked() {
                        if let Some((s, e)) = self.editor.selection_range() {
                            self.doc.decrease_font_size(s, e);
                            self.modified = true;
                        }
                        self.doc.cursor_style.font_size = (self.doc.cursor_style.font_size - 2.0).max(8.0);
                    }
                    ui.separator();
                    let is_mono = self.doc.cursor_style.font_family == FontFamily::Monospace;
                    if ui.selectable_label(!is_mono, "Aa").clicked() {
                        if let Some((s, e)) = self.editor.selection_range() {
                            self.doc.set_font_family(s, e, FontFamily::Proportional);
                            self.modified = true;
                        }
                        self.doc.cursor_style.font_family = FontFamily::Proportional;
                    }
                    if ui.selectable_label(is_mono, "Mm").clicked() {
                        if let Some((s, e)) = self.editor.selection_range() {
                            self.doc.set_font_family(s, e, FontFamily::Monospace);
                            self.modified = true;
                        }
                        self.doc.cursor_style.font_family = FontFamily::Monospace;
                    }
                });
            });
    }

    /// Render the custom rich text editor area
    fn render_editor(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let text_area_width = available.x;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let default_font = FontId::proportional(16.0);
                let default_row_height = ui.fonts(|f| f.row_height(&default_font));
                let line_count = self.doc.text.lines().count().max(1);
                let estimated_height = (line_count as f32 * default_row_height * 1.2).max(available.y);

                let (response, painter) = ui.allocate_painter(
                    Vec2::new(text_area_width, estimated_height),
                    Sense::click_and_drag(),
                );
                let rect = response.rect;
                painter.rect_filled(rect, 0.0, SlowColors::WHITE);
                self.layout_and_draw_text(&painter, rect, &response, ui);
            });
    }

    /// Main text layout and drawing
    fn layout_and_draw_text(
        &mut self,
        painter: &Painter,
        rect: Rect,
        response: &Response,
        ui: &egui::Ui,
    ) {
        let text = self.doc.text.clone();
        let styles = self.doc.styles.clone();

        if text.is_empty() && !response.has_focus() {
            painter.text(
                Pos2::new(rect.min.x + 8.0, rect.min.y + 4.0),
                Align2::LEFT_TOP,
                "start typing...",
                FontId::proportional(16.0),
                Color32::from_gray(160),
            );
        }

        if response.clicked() || response.drag_started() {
            response.request_focus();
        }

        // Layout: walk characters, compute positions
        let mut char_positions: Vec<(Pos2, f32, f32)> = Vec::with_capacity(text.chars().count());
        let mut x = rect.min.x + 8.0;
        let mut y = rect.min.y + 4.0;
        let wrap_width = rect.width() - 16.0;
        let default_style = CharStyle::default();

        for (char_idx, c) in text.chars().enumerate() {
            let style = styles.get(char_idx).unwrap_or(&default_style);
            let font = char_style_to_font(style);
            let row_height = ui.fonts(|f| f.row_height(&font));

            if c == '\n' {
                char_positions.push((Pos2::new(x, y), 0.0, row_height));
                x = rect.min.x + 8.0;
                y += row_height;
                continue;
            }

            let char_width = ui.fonts(|f| f.glyph_width(&font, c));
            if x + char_width > rect.min.x + wrap_width && x > rect.min.x + 8.0 {
                x = rect.min.x + 8.0;
                y += row_height;
            }

            char_positions.push((Pos2::new(x, y), char_width, row_height));
            x += char_width;
        }

        // Mouse interaction
        self.handle_mouse_interaction(response, &char_positions, rect, ui);

        // Selection overlay
        if let Some((sel_start, sel_end)) = self.editor.selection_range() {
            for i in sel_start..sel_end.min(char_positions.len()) {
                let (pos, width, height) = char_positions[i];
                let sel_rect = Rect::from_min_size(pos, Vec2::new(width.max(4.0), height));
                dither::draw_dither_selection(painter, sel_rect);
            }
        }

        // Draw characters
        for (char_idx, c) in text.chars().enumerate() {
            if c == '\n' { continue; }
            if char_idx >= char_positions.len() { break; }

            let (pos, _width, row_height) = char_positions[char_idx];
            let style = styles.get(char_idx).unwrap_or(&default_style);
            let font = char_style_to_font(style);

            let in_selection = self.editor.selection_range()
                .map(|(s, e)| char_idx >= s && char_idx < e)
                .unwrap_or(false);
            let color = if in_selection { SlowColors::WHITE } else { SlowColors::BLACK };

            painter.text(pos, Align2::LEFT_TOP, c.to_string(), font, color);

            // Decorations
            if style.underline {
                let uy = pos.y + row_height - 2.0;
                let cw = char_positions[char_idx].1;
                painter.line_segment(
                    [Pos2::new(pos.x, uy), Pos2::new(pos.x + cw, uy)],
                    Stroke::new(1.0, color),
                );
            }
            if style.strikethrough {
                let sy = pos.y + row_height * 0.45;
                let cw = char_positions[char_idx].1;
                painter.line_segment(
                    [Pos2::new(pos.x, sy), Pos2::new(pos.x + cw, sy)],
                    Stroke::new(1.0, color),
                );
            }
        }

        // Cursor
        if response.has_focus() {
            let blink = (ui.input(|i| i.time) * 2.0) as u64 % 2 == 0;
            if blink {
                let cursor_pos = if self.editor.cursor == 0 {
                    Pos2::new(rect.min.x + 8.0, rect.min.y + 4.0)
                } else if self.editor.cursor <= char_positions.len() {
                    let idx = self.editor.cursor - 1;
                    let (pos, width, rh) = char_positions[idx];
                    let prev_char = text.chars().nth(idx);
                    if prev_char == Some('\n') {
                        Pos2::new(rect.min.x + 8.0, pos.y + rh)
                    } else {
                        Pos2::new(pos.x + width, pos.y)
                    }
                } else {
                    Pos2::new(rect.min.x + 8.0, rect.min.y + 4.0)
                };

                let cursor_height = if self.editor.cursor > 0 && self.editor.cursor <= char_positions.len() {
                    char_positions[self.editor.cursor - 1].2
                } else {
                    ui.fonts(|f| f.row_height(&FontId::proportional(self.doc.cursor_style.font_size)))
                };

                painter.vline(
                    cursor_pos.x,
                    cursor_pos.y..=cursor_pos.y + cursor_height,
                    Stroke::new(1.0, SlowColors::BLACK),
                );
            }
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
        }
    }

    /// Handle mouse clicks, double-clicks, and drags for text selection
    fn handle_mouse_interaction(
        &mut self,
        response: &Response,
        char_positions: &[(Pos2, f32, f32)],
        rect: Rect,
        ui: &egui::Ui,
    ) {
        let pointer_pos = ui.input(|i| i.pointer.interact_pos());
        let primary_down = ui.input(|i| i.pointer.primary_down());
        let primary_pressed = ui.input(|i| i.pointer.primary_pressed());

        if let Some(pos) = pointer_pos {
            if !rect.contains(pos) { return; }

            let clicked_char = pos_to_char_index(pos, char_positions, rect);

            if primary_pressed {
                let now = Instant::now();
                let elapsed = now.duration_since(self.editor.last_click_time).as_millis();
                let same_pos = (clicked_char as i64 - self.editor.last_click_char as i64).unsigned_abs() <= 1;

                if elapsed < DOUBLE_CLICK_MS && same_pos {
                    // Double-click: select word, enter word-select drag mode
                    let byte_pos = self.doc.char_to_byte(clicked_char);
                    let (ws, we) = word_boundaries(&self.doc.text, byte_pos);
                    let ws_char = self.doc.byte_to_char(ws);
                    let we_char = self.doc.byte_to_char(we);
                    self.editor.word_select_active = true;
                    self.editor.word_anchor_start = ws_char;
                    self.editor.word_anchor_end = we_char;
                    self.editor.sel_anchor = Some(ws_char);
                    self.editor.cursor = we_char;
                } else {
                    self.editor.word_select_active = false;
                    let shift = ui.input(|i| i.modifiers.shift);
                    if shift {
                        if self.editor.sel_anchor.is_none() {
                            self.editor.sel_anchor = Some(self.editor.cursor);
                        }
                    } else {
                        self.editor.sel_anchor = None;
                    }
                    self.editor.cursor = clicked_char;
                    if clicked_char > 0 && clicked_char <= self.doc.styles.len() {
                        self.doc.cursor_style = self.doc.styles[clicked_char - 1].clone();
                    }
                }
                self.editor.last_click_time = now;
                self.editor.last_click_char = clicked_char;
            } else if primary_down && response.dragged() {
                if self.editor.word_select_active {
                    // Word-select drag
                    let byte_pos = self.doc.char_to_byte(clicked_char);
                    let (ws, we) = word_boundaries(&self.doc.text, byte_pos);
                    let ws_char = self.doc.byte_to_char(ws);
                    let we_char = self.doc.byte_to_char(we);
                    let sel_start = ws_char.min(self.editor.word_anchor_start);
                    let sel_end = we_char.max(self.editor.word_anchor_end);
                    self.editor.sel_anchor = Some(sel_start);
                    self.editor.cursor = sel_end;
                } else {
                    if self.editor.sel_anchor.is_none() {
                        self.editor.sel_anchor = Some(self.editor.cursor);
                    }
                    self.editor.cursor = clicked_char;
                }
            }
        }
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
                        ui.label("version 0.2.0");
                        ui.add_space(8.0);
                        ui.label("rich text editor for slowOS");
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.label("supported formats:");
                    ui.label("  .txt, .md (plain text)");
                    ui.label("  .rtf (import only)");
                    ui.label("  .swd (slowWrite rich document)");
                    ui.add_space(4.0);
                    ui.label("features:");
                    ui.label("  per-character styling");
                    ui.label("  bold, italic, underline, strikethrough");
                    ui.label("  variable font sizes (8-72pt)");
                    ui.label("  proportional & monospace fonts");
                    ui.label("  double-click-drag word selection");
                    ui.add_space(8.0);
                });
                ui.vertical_centered(|ui| {
                    if ui.button("ok").clicked() { self.show_about = false; }
                });
            });
        if let Some(r) = &resp { slowcore::dither::draw_window_shadow(ctx, r.response.rect); }
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
                    ui.label(egui::RichText::new("Formatting").strong());
                    ui.separator();
                    shortcut_row(ui, "\u{2318}B", "Bold");
                    shortcut_row(ui, "\u{2318}I", "Italic");
                    shortcut_row(ui, "\u{2318}U", "Underline");
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

/// Convert a CharStyle to an egui FontId, using the correct bold/italic font variant
fn char_style_to_font(style: &CharStyle) -> FontId {
    let family = match style.font_family {
        FontFamily::Proportional => match (style.bold, style.italic) {
            (true, true) => egui::FontFamily::Name("BoldItalic".into()),
            (true, false) => egui::FontFamily::Name("Bold".into()),
            (false, true) => egui::FontFamily::Name("Italic".into()),
            (false, false) => egui::FontFamily::Proportional,
        },
        FontFamily::Monospace => egui::FontFamily::Monospace,
    };
    FontId::new(style.font_size, family)
}

/// Convert screen position to char index
fn pos_to_char_index(pos: Pos2, char_positions: &[(Pos2, f32, f32)], _rect: Rect) -> usize {
    if char_positions.is_empty() { return 0; }

    let target_y = pos.y;
    let mut closest_row_y = char_positions[0].0.y;
    for &(cpos, _, _) in char_positions {
        if (cpos.y - target_y).abs() < (closest_row_y - target_y).abs() {
            closest_row_y = cpos.y;
        }
    }

    let mut best_idx = 0;
    let mut best_dist = f32::MAX;
    for (i, &(cpos, width, row_h)) in char_positions.iter().enumerate() {
        if (cpos.y - closest_row_y).abs() > row_h * 0.5 { continue; }
        let char_center_x = cpos.x + width * 0.5;
        let dist = (pos.x - char_center_x).abs();
        if dist < best_dist {
            best_dist = dist;
            best_idx = if pos.x > char_center_x { i + 1 } else { i };
        }
    }
    best_idx.min(char_positions.len())
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

        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect()
        });
        if let Some(path) = dropped.into_iter().next() {
            let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if ext == "txt" || ext == "md" || ext == "rtf" || ext == "swd" {
                self.open_file(path);
            }
        }

        self.doc.sync_styles();

        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| { self.render_menu_bar(ui); });
        egui::TopBottomPanel::top("title_bar").show(ctx, |ui| {
            slowcore::theme::SlowTheme::title_bar_frame().show(ui, |ui| {
                ui.centered_and_justified(|ui| { ui.label(self.display_title()); });
            });
        });
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| { self.render_toolbar(ui); });
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
    }
}
