//! SlowWrite v0.2.4 — word processor
//!
//! HTML-aware text editing with live formatting, find & replace,
//! document outline, focus mode, and writing statistics.

use crate::document::{Document, SpanKind};
use egui::{Align2, Context, Id, Key};
use slowcore::repaint::RepaintController;
use slowcore::storage::{config_dir, documents_dir, FileBrowser, RecentFiles};
use slowcore::text_edit::WordDragState;
use slowcore::theme::{consume_special_keys, menu_bar, Scale, SlowColors};
use slowcore::widgets::{
    status_bar, toolbar_separator, window_control_buttons, SlowButton, WindowAction,
};
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

/// Action deferred until user decides whether to save unsaved changes
#[derive(Clone)]
enum PendingAction {
    New,
    Open,
    OpenFile(PathBuf),
}

/// Formatting action to apply after keyboard/toolbar input
#[derive(Clone)]
enum FormatAction {
    /// Toggle an inline formatting span (bold, italic, etc.)
    ToggleSpan(SpanKind),
    /// Wrap current line in HTML block tag, e.g. tag="h1" → <h1>line</h1>
    ToggleLineTag(String),
    /// Toggle a visible line prefix, e.g. "- " for bullets
    ToggleLinePrefix(String),
}

/// Editing view mode
#[derive(Clone, Copy, PartialEq)]
enum ViewMode {
    /// Plain text: no formatting
    PlainText,
    /// Rich text: HTML-based WYSIWYG formatting
    RichText,
}

/// Application state
pub struct SlowWriteApp {
    doc: Document,
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
    /// Pending action waiting for save-before-open decision
    pending_action: Option<PendingAction>,
    show_save_before_open: bool,

    // Formatting
    pending_format: Option<FormatAction>,
    /// Saved cursor char index when pending_format was set (for toolbar clicks that steal focus)
    saved_cursor_char: Option<usize>,

    // Find & Replace
    show_find: bool,
    show_replace: bool,
    find_query: String,
    replace_text: String,
    find_matches: Vec<(usize, usize)>,
    find_current: usize,

    // View
    focus_mode: bool,
    focus_was_active: bool,
    focus_menu_sticky: bool,
    focus_popup_was_open: bool,
    show_outline: bool,
    zoom: f32,
    view_mode: ViewMode,

    // Active formatting state at cursor (for toolbar button highlighting)
    cursor_in_bold: bool,
    cursor_in_italic: bool,
    cursor_in_strikethrough: bool,
    cursor_in_underline: bool,
    /// Previous cursor byte position (to detect cursor movement)
    prev_cursor_byte: usize,
    /// Previous text length for detecting edits and adjusting spans
    prev_text: String,
    /// Autosave: frames since last save
    autosave_counter: u32,
    /// Active block tag on current line (e.g. "h1", "h2", "h3", or empty)
    cursor_line_tag: String,

    // Outline cache
    cached_headings: Vec<(usize, String, usize)>,
    /// Track text for cache invalidation
    last_text_len: usize,
    /// Pending scroll-to byte offset from outline click
    outline_scroll_byte: Option<usize>,
}

impl SlowWriteApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config_path = config_dir("slowwrite").join("recent.json");
        let recent_files =
            RecentFiles::load(&config_path).unwrap_or_else(|_| RecentFiles::new(10));

        Self {
            doc: Document::new(),
            file_path: None,
            file_title: "untitled".to_string(),
            modified: false,
            recent_files,
            show_file_browser: false,
            file_browser: FileBrowser::new(documents_dir())
                .with_filter(vec![
                    "txt".to_string(),
                    "html".to_string(),
                    "htm".to_string(),
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
            pending_action: None,
            show_save_before_open: false,
            pending_format: None,
            saved_cursor_char: None,
            show_find: false,
            show_replace: false,
            find_query: String::new(),
            replace_text: String::new(),
            find_matches: Vec::new(),
            find_current: 0,
            focus_mode: false,
            focus_was_active: false,
            focus_menu_sticky: false,
            focus_popup_was_open: false,
            show_outline: false,
            zoom: 1.0,
            view_mode: ViewMode::PlainText,
            cursor_in_bold: false,
            cursor_in_italic: false,
            cursor_in_strikethrough: false,
            cursor_in_underline: false,
            prev_cursor_byte: usize::MAX,
            prev_text: String::new(),
            autosave_counter: 0,
            cursor_line_tag: String::new(),
            cached_headings: Vec::new(),
            last_text_len: 0,
            outline_scroll_byte: None,
        }
    }

    fn request_new_document(&mut self) {
        if self.modified {
            self.pending_action = Some(PendingAction::New);
            self.show_save_before_open = true;
        } else {
            self.new_document();
        }
    }

    fn new_document(&mut self) {
        self.doc = Document::new();
        self.file_path = None;
        self.file_title = "untitled".to_string();
        self.modified = false;
        self.prev_text = String::new();
        self.word_drag = WordDragState::new();
    }

    fn request_open_dialog(&mut self) {
        if self.modified {
            self.pending_action = Some(PendingAction::Open);
            self.show_save_before_open = true;
        } else {
            self.show_open_dialog();
        }
    }

    fn request_open_file(&mut self, path: PathBuf) {
        if self.modified {
            self.pending_action = Some(PendingAction::OpenFile(path));
            self.show_save_before_open = true;
        } else {
            self.open_file(path);
        }
    }

    fn execute_pending_action(&mut self) {
        if let Some(action) = self.pending_action.take() {
            match action {
                PendingAction::New => self.new_document(),
                PendingAction::Open => self.show_open_dialog(),
                PendingAction::OpenFile(path) => self.open_file(path),
            }
        }
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
            "html" | "htm" => {
                match std::fs::read_to_string(&path) {
                    Ok(raw) => {
                        // Parse HTML into text + formatting spans
                        let doc = Document::from_html(&raw);
                        self.doc = doc;
                        self.file_title = path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or("untitled".to_string());
                        self.view_mode = ViewMode::RichText;
                        self.file_path = Some(path.clone());
                        self.modified = false;
                        self.prev_text = self.doc.text.clone();
                        self.word_drag = WordDragState::new();
                        self.recent_files.add(path);
                        self.save_recent_files();
                        return; // early return — doc already set
                    }
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

        self.doc = Document::from_text(text);
        self.file_title = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or("untitled".to_string());
        // Auto-detect view mode from file extension
        if ext == "html" || ext == "htm" {
            self.view_mode = ViewMode::RichText;
        } else {
            self.view_mode = ViewMode::PlainText;
        }
        self.file_path = Some(path.clone());
        self.modified = false;
        self.prev_text = self.doc.text.clone();
        self.word_drag = WordDragState::new();
        self.recent_files.add(path);
        self.save_recent_files();
    }

    fn save_content(&self) -> String {
        if self.view_mode == ViewMode::RichText {
            self.doc.to_html()
        } else {
            self.doc.text.clone()
        }
    }

    fn save_document(&mut self) {
        if let Some(ref path) = self.file_path.clone() {
            // If in rich text mode but file is .txt, switch to .html
            let save_path = if self.view_mode == ViewMode::RichText {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext == "txt" {
                    let new_path = path.with_extension("html");
                    self.file_path = Some(new_path.clone());
                    self.file_title = new_path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or("untitled".to_string());
                    new_path
                } else {
                    path.clone()
                }
            } else {
                // Plain text mode — if file is .html, save as .txt copy
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext == "html" || ext == "htm" {
                    let new_path = path.with_extension("txt");
                    self.file_path = Some(new_path.clone());
                    self.file_title = new_path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or("untitled".to_string());
                    new_path
                } else {
                    path.clone()
                }
            };
            let content = self.save_content();
            if let Err(e) = std::fs::write(&save_path, &content) {
                eprintln!("failed to save: {}", e);
            } else {
                self.modified = false;
            }
        } else {
            self.show_save_as_dialog();
        }
    }

    fn save_document_as(&mut self, path: PathBuf) {
        let content = self.save_content();
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
            "html".to_string(),
            "htm".to_string(),
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
            || self.save_filename.ends_with(".html")
            || self.save_filename.ends_with(".htm");
        if !has_ext {
            let ext = if self.view_mode == ViewMode::RichText { ".html" } else { ".txt" };
            self.save_filename.push_str(ext);
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
        // Zoom shortcuts — must consume BEFORE consume_special_keys eats them
        let zoom_in = ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::COMMAND, Key::Plus) ||
            i.consume_key(egui::Modifiers::COMMAND, Key::Equals)
        });
        let zoom_out = ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::Minus));
        let zoom_reset = ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::Num0));
        if zoom_in { self.zoom = (self.zoom + 0.1).min(2.0); }
        if zoom_out { self.zoom = (self.zoom - 0.1).max(0.5); }
        if zoom_reset { self.zoom = 1.0; }

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
                            Key::N if cmd => { handled = true; actions.push(Box::new(|s| s.request_new_document())); }
                            Key::O if cmd => { handled = true; actions.push(Box::new(|s| s.request_open_dialog())); }
                            Key::S if cmd && shift => { handled = true; actions.push(Box::new(|s| s.show_save_as_dialog())); }
                            Key::S if cmd => { handled = true; actions.push(Box::new(|s| s.save_document())); }
                            // Formatting
                            Key::B if cmd => { handled = true; actions.push(Box::new(|s| s.pending_format = Some(FormatAction::ToggleSpan(SpanKind::Bold)))); }
                            Key::I if cmd => { handled = true; actions.push(Box::new(|s| s.pending_format = Some(FormatAction::ToggleSpan(SpanKind::Italic)))); }
                            Key::U if cmd => { handled = true; actions.push(Box::new(|s| s.pending_format = Some(FormatAction::ToggleSpan(SpanKind::Underline)))); }
                            // View mode toggle
                            Key::R if cmd && shift => {
                                handled = true;
                                actions.push(Box::new(|s| {
                                    if s.view_mode == ViewMode::PlainText {
                                        s.view_mode = ViewMode::RichText;
                                    } else {
                                        s.view_mode = ViewMode::PlainText;
                                    }
                                }));
                            }
                            // Find & Replace
                            Key::F if cmd && shift => { handled = true; actions.push(Box::new(|s| s.focus_mode = !s.focus_mode)); }
                            Key::F if cmd => { handled = true; actions.push(Box::new(|s| { s.show_find = true; })); }
                            Key::H if cmd => { handled = true; actions.push(Box::new(|s| { s.show_find = true; s.show_replace = true; })); }
                            // Headings
                            Key::Num1 if cmd => { handled = true; actions.push(Box::new(|s| s.pending_format = Some(FormatAction::ToggleLineTag("h1".to_string())))); }
                            Key::Num2 if cmd => { handled = true; actions.push(Box::new(|s| s.pending_format = Some(FormatAction::ToggleLineTag("h2".to_string())))); }
                            Key::Num3 if cmd => { handled = true; actions.push(Box::new(|s| s.pending_format = Some(FormatAction::ToggleLineTag("h3".to_string())))); }
                            // Zoom handled before consume_special_keys
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

    /// Toggle an inline HTML tag around the current selection.
    /// `tag` is the tag name, e.g. "b" for `<b>...</b>`.
    /// Toggle a formatting span at the current cursor/selection.
    /// No text is modified — only the spans metadata changes.
    fn toggle_span(&mut self, ctx: &Context, kind: SpanKind) {
        let editor_id = Id::new("editor");
        let state = egui::TextEdit::load_state(ctx, editor_id);
        if state.is_none() { return; }
        let state = state.unwrap();

        let (primary, secondary) = if let Some(r) = state.cursor.char_range() {
            (r.primary.index, r.secondary.index)
        } else if let Some(saved) = self.saved_cursor_char {
            (saved, saved)
        } else {
            return;
        };
        let (sel_start, sel_end) = if primary <= secondary { (primary, secondary) } else { (secondary, primary) };

        let text = &self.doc.text;
        let cc = text.chars().count();
        let sel_start = sel_start.min(cc);
        let sel_end = sel_end.min(cc);

        let byte_start: usize = text.char_indices().nth(sel_start).map(|(i, _)| i).unwrap_or(text.len());
        let byte_end: usize = text.char_indices().nth(sel_end).map(|(i, _)| i).unwrap_or(text.len());

        self.doc.toggle_format(byte_start, byte_end, kind);
        self.modified = true;
    }

    /// Toggle a block-level HTML tag on the current line.
    /// For headings, replaces any existing heading tag.
    fn toggle_line_tag(&mut self, ctx: &Context, tag: &str) {
        let editor_id = Id::new("editor");
        let state = egui::TextEdit::load_state(ctx, editor_id);
        if state.is_none() { return; }
        let mut state = state.unwrap();

        let cursor_char = if let Some(range) = state.cursor.char_range() {
            range.primary.index
        } else if let Some(saved) = self.saved_cursor_char {
            saved
        } else {
            return;
        };

        let text = &self.doc.text;
        let cursor_char = cursor_char.min(text.chars().count());
        let byte_pos: usize = text.char_indices().nth(cursor_char).map(|(i, _)| i).unwrap_or(text.len());
        let line_start_byte = text[..byte_pos].rfind('\n').map(|i| i + 1).unwrap_or(0);

        let line_end_byte = text[byte_pos..].find('\n').map(|i| byte_pos + i).unwrap_or(text.len());
        let line = &text[line_start_byte..line_end_byte];

        let open_tag = format!("<{}>", tag);
        let close_tag = format!("</{}>", tag);
        let is_block = (tag.starts_with('h') && tag.len() == 2) || tag == "p";

        // Check if line already has this tag or another block tag
        let existing_tag: Option<String> = if is_block {
            ["h1", "h2", "h3", "h4", "h5", "h6", "p"].iter()
                .find(|t| line.starts_with(&format!("<{}>", t)) && line.ends_with(&format!("</{}>", t)))
                .map(|t| t.to_string())
        } else {
            if line.starts_with(&open_tag) && line.ends_with(&close_tag) {
                Some(tag.to_string())
            } else {
                None
            }
        };

        if let Some(existing) = existing_tag {
            let ex_open = format!("<{}>", existing);
            let ex_close = format!("</{}>", existing);
            let inner = &line[ex_open.len()..line.len() - ex_close.len()];

            if existing == tag {
                // Same tag — remove it
                self.doc.text = format!(
                    "{}{}{}",
                    &text[..line_start_byte],
                    inner,
                    &text[line_end_byte..]
                );
                let removed_chars = ex_open.chars().count();
                state.cursor.set_char_range(Some(egui::text::CCursorRange::one(
                    egui::text::CCursor::new(cursor_char.saturating_sub(removed_chars)),
                )));
            } else {
                // Different tag — replace
                self.doc.text = format!(
                    "{}{}{}{}{}",
                    &text[..line_start_byte],
                    open_tag,
                    inner,
                    close_tag,
                    &text[line_end_byte..]
                );
                let old_chars = ex_open.chars().count();
                let new_chars = open_tag.chars().count();
                let delta = new_chars as isize - old_chars as isize;
                state.cursor.set_char_range(Some(egui::text::CCursorRange::one(
                    egui::text::CCursor::new((cursor_char as isize + delta).max(0) as usize),
                )));
            }
        } else {
            // Wrap line in tag
            self.doc.text = format!(
                "{}{}{}{}{}",
                &text[..line_start_byte],
                open_tag,
                line,
                close_tag,
                &text[line_end_byte..]
            );
            let added_chars = open_tag.chars().count();
            state.cursor.set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(cursor_char + added_chars),
            )));
        }

        state.store(ctx, editor_id);
        self.modified = true;
    }

    /// Update cursor formatting state from spans.
    fn update_cursor_formatting(&mut self, ctx: &Context, text_changed: bool) {
        self.cursor_in_bold = false;
        self.cursor_in_italic = false;
        self.cursor_in_strikethrough = false;
        self.cursor_in_underline = false;

        let editor_id = Id::new("editor");
        let state = egui::TextEdit::load_state(ctx, editor_id);
        if state.is_none() { return; }
        let range = state.unwrap().cursor.char_range();
        if range.is_none() { return; }
        let cursor_char = range.unwrap().primary.index;

        let byte_pos: usize = self.doc.text.char_indices().nth(cursor_char).map(|(i, _)| i).unwrap_or(self.doc.text.len());

        // Only sync active_formats when cursor moves WITHOUT text changing.
        // If text changed, cursor moved because of typing — don't override active formats.
        if byte_pos != self.prev_cursor_byte && !text_changed {
            self.doc.sync_active_formats(byte_pos);
        }
        self.prev_cursor_byte = byte_pos;

        self.cursor_in_bold = self.doc.has_format_at(byte_pos, SpanKind::Bold);
        self.cursor_in_italic = self.doc.has_format_at(byte_pos, SpanKind::Italic);
        self.cursor_in_strikethrough = self.doc.has_format_at(byte_pos, SpanKind::Strikethrough);
        self.cursor_in_underline = self.doc.has_format_at(byte_pos, SpanKind::Underline);

        let text = &self.doc.text;

        // Check which block tag is on the current line
        self.cursor_line_tag.clear();
        let line_start = text[..byte_pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line_end = text[byte_pos..].find('\n').map(|i| byte_pos + i).unwrap_or(text.len());
        let line = &text[line_start..line_end];
        for tag in ["h1", "h2", "h3", "p"] {
            let open = format!("<{}>", tag);
            let close = format!("</{}>", tag);
            if line.starts_with(&open) && line.ends_with(&close) {
                self.cursor_line_tag = tag.to_string();
                break;
            }
        }
    }

    /// Toggle a visible line prefix (e.g. "- " for bullets).
    fn toggle_line_prefix(&mut self, ctx: &Context, prefix: &str) {
        let editor_id = Id::new("editor");
        let state = egui::TextEdit::load_state(ctx, editor_id);
        if state.is_none() { return; }
        let mut state = state.unwrap();
        let range = state.cursor.char_range();
        if range.is_none() { return; }
        let cursor_char = range.unwrap().primary.index;

        let text = &self.doc.text;
        let byte_pos: usize = text.char_indices().nth(cursor_char).map(|(i, _)| i).unwrap_or(text.len());
        let line_start = text[..byte_pos].rfind('\n').map(|i| i + 1).unwrap_or(0);

        if text[line_start..].starts_with(prefix) {
            // Remove prefix
            self.doc.text = format!("{}{}", &text[..line_start], &text[line_start + prefix.len()..]);
            let removed = prefix.chars().count();
            state.cursor.set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(cursor_char.saturating_sub(removed)),
            )));
        } else {
            // Add prefix
            self.doc.text = format!("{}{}{}", &text[..line_start], prefix, &text[line_start..]);
            let added = prefix.chars().count();
            state.cursor.set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(cursor_char + added),
            )));
        }
        state.store(ctx, editor_id);
        self.modified = true;
    }

    /// Apply any pending format action (called after editor has been shown so TextEdit state is valid)
    fn apply_pending_format(&mut self, ctx: &Context) {
        if let Some(action) = self.pending_format.take() {
            match action {
                FormatAction::ToggleSpan(kind) => self.toggle_span(ctx, kind),
                FormatAction::ToggleLineTag(tag) => self.toggle_line_tag(ctx, &tag),
                FormatAction::ToggleLinePrefix(prefix) => self.toggle_line_prefix(ctx, &prefix),
            }
        }
    }

    /// Detect text changes and adjust formatting spans accordingly
    fn sync_spans_with_text(&mut self, _ctx: &Context) {
        if self.doc.text == self.prev_text { return; }

        let old = &self.prev_text;
        let new = &self.doc.text;

        // Simple diff: find the first differing byte
        let common_prefix = old.bytes().zip(new.bytes()).take_while(|(a, b)| a == b).count();

        if new.len() > old.len() {
            let inserted = new.len() - old.len();
            self.doc.after_insert(common_prefix, inserted);
        } else if new.len() < old.len() {
            let deleted = old.len() - new.len();
            self.doc.after_delete(common_prefix, deleted);
        }

        self.prev_text = self.doc.text.clone();
    }

    /// Update search matches when query or text changes
    fn update_find_matches(&mut self) {
        self.find_matches.clear();
        if self.find_query.is_empty() { return; }
        let query_lower = self.find_query.to_lowercase();
        let text_lower = self.doc.text.to_lowercase();
        let mut start = 0;
        while let Some(pos) = text_lower[start..].find(&query_lower) {
            let abs_pos = start + pos;
            self.find_matches.push((abs_pos, abs_pos + self.find_query.len()));
            start = abs_pos + 1;
        }
        if self.find_current >= self.find_matches.len() {
            self.find_current = 0;
        }
    }

    /// Jump cursor to the current find match
    fn jump_to_find_match(&self, ctx: &Context) {
        if self.find_matches.is_empty() { return; }
        let (byte_start, byte_end) = self.find_matches[self.find_current];
        let editor_id = Id::new("editor");
        let state = egui::TextEdit::load_state(ctx, editor_id);
        if state.is_none() { return; }
        let mut state = state.unwrap();

        // Convert byte offsets to char indices
        let char_start = self.doc.text[..byte_start].chars().count();
        let char_end = self.doc.text[..byte_end].chars().count();

        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(char_start),
            egui::text::CCursor::new(char_end),
        )));
        state.store(ctx, editor_id);
    }

    /// Replace current match
    fn replace_current_match(&mut self) {
        if self.find_matches.is_empty() { return; }
        let (byte_start, byte_end) = self.find_matches[self.find_current];
        self.doc.text = format!(
            "{}{}{}",
            &self.doc.text[..byte_start],
            self.replace_text,
            &self.doc.text[byte_end..]
        );
        self.modified = true;
        self.update_find_matches();
        if self.find_current >= self.find_matches.len() && !self.find_matches.is_empty() {
            self.find_current = 0;
        }
    }

    /// Replace all matches
    fn replace_all_matches(&mut self) {
        if self.find_matches.is_empty() { return; }
        // Replace from end to start to preserve byte offsets
        let matches: Vec<_> = self.find_matches.iter().rev().cloned().collect();
        for (byte_start, byte_end) in matches {
            self.doc.text = format!(
                "{}{}{}",
                &self.doc.text[..byte_start],
                self.replace_text,
                &self.doc.text[byte_end..]
            );
        }
        self.modified = true;
        self.update_find_matches();
    }



    /// Draw a toolbar button that doesn't steal focus from the editor.
    /// `font_id` controls the text style (bold, italic, etc.)
    /// `text_fmt` adds underline/strikethrough to the painted text.
    fn toolbar_btn_styled(
        ui: &mut egui::Ui,
        label: &str,
        active: bool,
        font_id: egui::FontId,
        underline: bool,
        strikethrough: bool,
    ) -> bool {
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(26.0, 20.0), egui::Sense::click());
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            let fill = if active {
                egui::Color32::from_gray(160)
            } else if resp.hovered() {
                egui::Color32::from_gray(200)
            } else {
                SlowColors::WHITE
            };
            painter.rect(rect, 0.0, fill, egui::Stroke::new(1.0, SlowColors::BLACK));
            let galley = painter.layout_no_wrap(label.to_string(), font_id, SlowColors::BLACK);
            let text_pos = rect.center() - galley.size() / 2.0;
            painter.galley(text_pos, galley, SlowColors::BLACK);
            if underline {
                let y = rect.center().y + 7.0;
                painter.hline(rect.min.x + 6.0..=rect.max.x - 6.0, y, egui::Stroke::new(1.0, SlowColors::BLACK));
            }
            if strikethrough {
                let y = rect.center().y;
                painter.hline(rect.min.x + 6.0..=rect.max.x - 6.0, y, egui::Stroke::new(1.0, SlowColors::BLACK));
            }
        }
        resp.clicked()
    }

    fn toolbar_btn(ui: &mut egui::Ui, label: &str, active: bool) -> bool {
        Self::toolbar_btn_styled(ui, label, active, egui::FontId::proportional(12.0), false, false)
    }

    fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            // B/I/U/S with styled labels showing what the decoration looks like
            if Self::toolbar_btn_styled(ui, "B", self.cursor_in_bold,
                egui::FontId::new(13.0, egui::FontFamily::Name("Bold".into())), false, false) {
                self.pending_format = Some(FormatAction::ToggleSpan(SpanKind::Bold));
            }
            if Self::toolbar_btn_styled(ui, "I", self.cursor_in_italic,
                egui::FontId::new(13.0, egui::FontFamily::Name("Italic".into())), false, false) {
                self.pending_format = Some(FormatAction::ToggleSpan(SpanKind::Italic));
            }
            if Self::toolbar_btn_styled(ui, "U", self.cursor_in_underline,
                egui::FontId::proportional(12.0), true, false) {
                self.pending_format = Some(FormatAction::ToggleSpan(SpanKind::Underline));
            }
            if Self::toolbar_btn_styled(ui, "S", self.cursor_in_strikethrough,
                egui::FontId::proportional(12.0), false, true) {
                self.pending_format = Some(FormatAction::ToggleSpan(SpanKind::Strikethrough));
            }
            toolbar_separator(ui);
            ui.spacing_mut().item_spacing.x = 6.0;
            if Self::toolbar_btn(ui, "H1", self.cursor_line_tag == "h1") {
                self.pending_format = Some(FormatAction::ToggleLineTag("h1".to_string()));
            }
            if Self::toolbar_btn(ui, "H2", self.cursor_line_tag == "h2") {
                self.pending_format = Some(FormatAction::ToggleLineTag("h2".to_string()));
            }
            if Self::toolbar_btn(ui, "H3", self.cursor_line_tag == "h3") {
                self.pending_format = Some(FormatAction::ToggleLineTag("h3".to_string()));
            }
            if Self::toolbar_btn(ui, " P ", self.cursor_line_tag == "p") {
                self.pending_format = Some(FormatAction::ToggleLineTag("p".to_string()));
            }
            ui.spacing_mut().item_spacing.x = 4.0;
            toolbar_separator(ui);
            // Block formatting
            if ui.add(SlowButton::new(" \u{2022} ")).clicked() {
                self.pending_format = Some(FormatAction::ToggleLinePrefix("- ".to_string()));
            }
            if ui.add(SlowButton::new(" > ")).clicked() {
                self.pending_format = Some(FormatAction::ToggleLinePrefix("> ".to_string()));
            }
        });
    }

    fn render_menu_bar(&mut self, ui: &mut egui::Ui) -> WindowAction {
        let mut action = WindowAction::None;
        menu_bar(ui, |ui| {
            action = window_control_buttons(ui);
            ui.menu_button("file", |ui| {
                if ui.button("new        Cmd+n").clicked() {
                    self.request_new_document();
                    ui.close_menu();
                }
                if ui.button("open...    Cmd+o").clicked() {
                    self.request_open_dialog();
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
                                self.request_open_file(path);
                                ui.close_menu();
                            }
                        }
                    }
                });
                ui.separator();
                if ui.button("save       Cmd+s").clicked() {
                    self.save_document();
                    ui.close_menu();
                }
                if ui.button("save as... Shift+Cmd+s").clicked() {
                    self.show_save_as_dialog();
                    ui.close_menu();
                }
            });

            ui.menu_button("edit", |ui| {
                if ui.button("find       Cmd+f").clicked() {
                    self.show_find = true;
                    ui.close_menu();
                }
                if ui.button("replace    Cmd+h").clicked() {
                    self.show_find = true;
                    self.show_replace = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("cut        Cmd+x").clicked() {
                    ui.ctx().input_mut(|i| {
                        i.events.push(egui::Event::Cut);
                    });
                    ui.close_menu();
                }
                if ui.button("copy       Cmd+c").clicked() {
                    ui.ctx().input_mut(|i| {
                        i.events.push(egui::Event::Copy);
                    });
                    ui.close_menu();
                }
                if ui.button("paste      Cmd+v").clicked() {
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
                if ui.button("select all Cmd+a").clicked() {
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

            // Format menu only in rich text mode
            if self.view_mode == ViewMode::RichText {
                ui.menu_button("format", |ui| {
                    if ui.button("bold           Cmd+b").clicked() {
                        self.pending_format = Some(FormatAction::ToggleSpan(SpanKind::Bold));
                        ui.close_menu();
                    }
                    if ui.button("italic         Cmd+i").clicked() {
                        self.pending_format = Some(FormatAction::ToggleSpan(SpanKind::Italic));
                        ui.close_menu();
                    }
                    if ui.button("strikethrough").clicked() {
                        self.pending_format = Some(FormatAction::ToggleSpan(SpanKind::Strikethrough));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("heading 1      Cmd+1").clicked() {
                        self.pending_format = Some(FormatAction::ToggleLineTag("h1".to_string()));
                        ui.close_menu();
                    }
                    if ui.button("heading 2      Cmd+2").clicked() {
                        self.pending_format = Some(FormatAction::ToggleLineTag("h2".to_string()));
                        ui.close_menu();
                    }
                    if ui.button("heading 3      Cmd+3").clicked() {
                        self.pending_format = Some(FormatAction::ToggleLineTag("h3".to_string()));
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("bullet list").clicked() {
                        self.pending_format = Some(FormatAction::ToggleLinePrefix("- ".to_string()));
                        ui.close_menu();
                    }
                    if ui.button("blockquote").clicked() {
                        self.pending_format = Some(FormatAction::ToggleLinePrefix("> ".to_string()));
                        ui.close_menu();
                    }
                    if ui.button("code").clicked() {
                        self.pending_format = Some(FormatAction::ToggleSpan(SpanKind::Bold));
                        ui.close_menu();
                    }
                });
            }

            ui.menu_button("view", |ui| {
                let mode_label = if self.view_mode == ViewMode::PlainText {
                    "rich text view  Shift+Cmd+r"
                } else {
                    "plain text view Shift+Cmd+r"
                };
                if ui.button(mode_label).clicked() {
                    self.view_mode = if self.view_mode == ViewMode::PlainText {
                        // Switch to rich text — resize to larger window
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(780.0, 560.0)));
                        ViewMode::RichText
                    } else {
                        // Switch to plain text — resize to smaller window
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(600.0, 440.0)));
                        ViewMode::PlainText
                    };
                    ui.close_menu();
                }
                if self.view_mode == ViewMode::RichText {
                    ui.separator();
                    if ui.button("focus mode Shift+Cmd+f").clicked() {
                        self.focus_mode = !self.focus_mode;
                        ui.close_menu();
                    }
                    if ui.button(if self.show_outline { "hide outline" } else { "show outline" }).clicked() {
                        self.show_outline = !self.show_outline;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("zoom in    Cmd++").clicked() {
                        self.zoom = (self.zoom + 0.1).min(2.0);
                        ui.close_menu();
                    }
                    if ui.button("zoom out   Cmd+-").clicked() {
                        self.zoom = (self.zoom - 0.1).max(0.5);
                        ui.close_menu();
                    }
                    if ui.button("reset zoom Cmd+0").clicked() {
                        self.zoom = 1.0;
                        ui.close_menu();
                    }
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

    /// Render the editor
    fn render_editor(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let font_size = (Scale::BODY + 2.0) * self.zoom;
        let matches = if self.show_find { self.find_matches.clone() } else { vec![] };
        let current = if self.show_find { Some(self.find_current) } else { None };
        let use_rich = self.view_mode == ViewMode::RichText;
        let hide_tags = use_rich;
        let spans = self.doc.spans.clone();

        let mut layouter = |_ui: &egui::Ui, text: &str, wrap_width: f32| {
            if use_rich {
                let job = crate::html::layout_with_spans(text, wrap_width, font_size, &spans, hide_tags, &matches, current);
                _ui.fonts(|f| f.layout_job(job))
            } else {
                // Plain text: simple single-font layout, no HTML processing
                let mut job = egui::text::LayoutJob::default();
                job.wrap.max_width = wrap_width;
                job.text = text.to_string();
                job.sections.push(egui::text::LayoutSection {
                    leading_space: 0.0,
                    byte_range: 0..text.len(),
                    format: egui::TextFormat {
                        font_id: egui::FontId::proportional(font_size),
                        color: egui::Color32::BLACK,
                        ..Default::default()
                    },
                });
                _ui.fonts(|f| f.layout_job(job))
            }
        };

        let pending_scroll = self.outline_scroll_byte.take();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let output = egui::TextEdit::multiline(&mut self.doc.text)
                    .id(Id::new("editor"))
                    .layouter(&mut layouter)
                    .desired_width(available.x)
                    .desired_rows((available.y / 22.0).max(4.0) as usize)
                    .frame(false)
                    .show(ui);

                // Scroll to heading position when outline item was clicked
                if let Some(byte_off) = pending_scroll {
                    let clamped = byte_off.min(self.doc.text.len());
                    let char_idx = self.doc.text[..clamped].chars().count();
                    let cursor = output.galley.from_ccursor(egui::text::CCursor::new(char_idx));
                    if let Some(row) = output.galley.rows.get(cursor.rcursor.row) {
                        let text_min = output.galley_pos;
                        let row_rect = egui::Rect::from_min_size(
                            egui::pos2(text_min.x, text_min.y + row.min_y()),
                            egui::vec2(available.x, row.height()),
                        );
                        ui.scroll_to_rect(row_rect, Some(egui::Align::TOP));
                    }
                }

                if output.response.changed() {
                    self.modified = true;
                    // Auto-continue bullet lists: if user just pressed Enter
                    // after a line starting with "- ", insert "- " on the new line
                    if let Some(range) = output.cursor_range {
                        let cursor_char = range.primary.ccursor.index;
                        let byte_pos: usize = self.doc.text.char_indices()
                            .nth(cursor_char).map(|(i, _)| i)
                            .unwrap_or(self.doc.text.len());
                        // Check if cursor is right after a newline
                        if byte_pos > 0 && self.doc.text.as_bytes().get(byte_pos - 1) == Some(&b'\n') {
                            // Find the line before the newline
                            let before_nl = byte_pos - 1;
                            let prev_line_start = self.doc.text[..before_nl]
                                .rfind('\n').map(|i| i + 1).unwrap_or(0);
                            let prev_line = &self.doc.text[prev_line_start..before_nl];
                            let prefix = if prev_line.starts_with("- ") && prev_line.len() > 2 {
                                Some("- ")
                            } else if prev_line.starts_with("> ") && prev_line.len() > 2 {
                                Some("> ")
                            } else {
                                None
                            };
                            if let Some(pfx) = prefix {
                                self.doc.text.insert_str(byte_pos, pfx);
                                // Move cursor past the inserted prefix
                                if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), Id::new("editor")) {
                                    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(
                                        egui::text::CCursor::new(cursor_char + pfx.chars().count()),
                                    )));
                                    state.store(ui.ctx(), Id::new("editor"));
                                }
                            }
                        }
                    }
                }

                self.word_drag.update(ui, &output, &self.doc.text);
            });
    }

    fn render_find_bar(&mut self, ui: &mut egui::Ui) {
        let mut close_find = false;
        let mut do_next = false;
        let mut do_prev = false;
        let mut do_replace = false;
        let mut do_replace_all = false;

        egui::Frame::none()
            .fill(SlowColors::WHITE)
            .stroke(egui::Stroke::new(slowcore::BORDER, SlowColors::BLACK))
            .inner_margin(egui::Margin::symmetric(slowcore::theme::Grid::SM, 3.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("find:");
                    let find_resp = ui.add(
                        egui::TextEdit::singleline(&mut self.find_query)
                            .desired_width(150.0)
                            .font(egui::FontId::proportional(Scale::UI))
                    );
                    if find_resp.changed() {
                        self.update_find_matches();
                    }
                    // Enter in find field = next match
                    if find_resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        do_next = true;
                        find_resp.request_focus();
                    }
                    if ui.add(SlowButton::new("\u{2191}")).clicked() { do_prev = true; }
                    if ui.add(SlowButton::new("\u{2193}")).clicked() { do_next = true; }
                    if !self.find_matches.is_empty() {
                        ui.label(format!("{} of {}", self.find_current + 1, self.find_matches.len()));
                    } else if !self.find_query.is_empty() {
                        ui.label("0 of 0");
                    }
                    if ui.add(SlowButton::new("\u{00d7}")).clicked() { close_find = true; }
                });
                if self.show_replace {
                    ui.horizontal(|ui| {
                        ui.label("replace:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.replace_text)
                                .desired_width(150.0)
                                .font(egui::FontId::proportional(Scale::UI))
                        );
                        if ui.add(SlowButton::new("replace")).clicked() { do_replace = true; }
                        if ui.add(SlowButton::new("all")).clicked() { do_replace_all = true; }
                    });
                }
            });

        if do_next && !self.find_matches.is_empty() {
            self.find_current = (self.find_current + 1) % self.find_matches.len();
        }
        if do_prev && !self.find_matches.is_empty() {
            self.find_current = if self.find_current == 0 { self.find_matches.len() - 1 } else { self.find_current - 1 };
        }
        if do_replace { self.replace_current_match(); }
        if do_replace_all { self.replace_all_matches(); }
        if close_find {
            self.show_find = false;
            self.show_replace = false;
            self.find_matches.clear();
        }
    }

    fn render_outline(&mut self, ctx: &Context) {
        let mut jump_to: Option<usize> = None;

        egui::SidePanel::left("outline")
            .default_width(160.0)
            .resizable(false)
            .frame(egui::Frame::none()
                .fill(SlowColors::WHITE)
                .stroke(egui::Stroke::new(slowcore::BORDER, SlowColors::BLACK))
                .inner_margin(egui::Margin::symmetric(slowcore::theme::Grid::SM, slowcore::theme::Grid::SM)))
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("outline").size(Scale::SMALL).strong());
                ui.separator();
                let headings = self.cached_headings.clone();
                if headings.is_empty() {
                    ui.label(egui::RichText::new("no headings").size(Scale::SMALL));
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (level, text, byte_offset) in &headings {
                            let indent = (*level as f32 - 1.0) * 12.0;
                            ui.horizontal(|ui| {
                                ui.add_space(indent);
                                let resp = ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(text)
                                            .size(Scale::SMALL)
                                            .color(SlowColors::BLACK)
                                    ).sense(egui::Sense::click())
                                );
                                if resp.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                if resp.clicked() {
                                    jump_to = Some(*byte_offset);
                                }
                            });
                        }
                    });
                }
            });

        // Apply cursor jump outside the panel closure to avoid borrow issues
        if let Some(byte_offset) = jump_to {
            let editor_id = Id::new("editor");
            if let Some(mut state) = egui::TextEdit::load_state(ctx, editor_id) {
                let char_idx = self.doc.text[..byte_offset.min(self.doc.text.len())].chars().count();
                state.cursor.set_char_range(Some(egui::text::CCursorRange::one(
                    egui::text::CCursor::new(char_idx),
                )));
                state.store(ctx, editor_id);
                ctx.memory_mut(|mem| mem.request_focus(editor_id));
            }
            self.outline_scroll_byte = Some(byte_offset);
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
                        ui.label("version 0.2.4");
                        ui.add_space(8.0);
                        ui.label("word processor for slowOS");
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.label("supported formats:");
                    ui.label("  .txt, .html (plain text / rich text)");
                    ui.add_space(4.0);
                    ui.label("features:");
                    ui.label("  live rich text formatting");
                    ui.label("  find & replace");
                    ui.label("  document outline");
                    ui.label("  focus mode");
                    ui.label("  zoom");
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
                    shortcut_row(ui, "Cmd+N", "New document");
                    shortcut_row(ui, "Cmd+O", "Open file");
                    shortcut_row(ui, "Cmd+S", "Save");
                    shortcut_row(ui, "Shift+Cmd+S", "Save as");
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Editing").strong());
                    ui.separator();
                    shortcut_row(ui, "Cmd+X", "Cut");
                    shortcut_row(ui, "Cmd+C", "Copy");
                    shortcut_row(ui, "Cmd+V", "Paste");
                    shortcut_row(ui, "Cmd+A", "Select all");
                    shortcut_row(ui, "Cmd+F", "Find");
                    shortcut_row(ui, "Cmd+H", "Find & replace");
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Formatting").strong());
                    ui.separator();
                    shortcut_row(ui, "Cmd+B", "Bold");
                    shortcut_row(ui, "Cmd+I", "Italic");
                    shortcut_row(ui, "Cmd+1/2/3", "Heading 1/2/3");
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("View").strong());
                    ui.separator();
                    shortcut_row(ui, "Shift+Cmd+F", "Focus mode");
                    shortcut_row(ui, "Cmd++", "Zoom in");
                    shortcut_row(ui, "Cmd+-", "Zoom out");
                    shortcut_row(ui, "Cmd+0", "Reset zoom");
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Selection").strong());
                    ui.separator();
                    shortcut_row(ui, "Double-click", "Select word");
                    shortcut_row(ui, "Dbl-click drag", "Select words");
                    shortcut_row(ui, "Shift++Click", "Extend selection");
                    ui.add_space(8.0);
                });
                ui.vertical_centered(|ui| {
                    if ui.button("ok").clicked() { self.show_shortcuts = false; }
                });
            });
        if let Some(r) = &resp { slowcore::dither::draw_window_shadow(ctx, r.response.rect); }
    }

    fn render_save_before_open(&mut self, ctx: &Context) {
        let resp = egui::Window::new("unsaved changes")
            .collapsible(false).resizable(false).default_width(300.0)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("you have unsaved changes.");
                ui.label("do you want to save before continuing?");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("don't save").clicked() {
                        self.show_save_before_open = false;
                        self.execute_pending_action();
                    }
                    if ui.button("cancel").clicked() {
                        self.show_save_before_open = false;
                        self.pending_action = None;
                    }
                    if ui.button("save").clicked() {
                        self.save_document();
                        if !self.modified {
                            self.show_save_before_open = false;
                            self.execute_pending_action();
                        }
                    }
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
        // Auto-focus editor so cursor is active immediately
        ctx.memory_mut(|mem| {
            if mem.focused().is_none() {
                mem.request_focus(Id::new("editor"));
            }
        });
        if slowcore::minimize::check_restore_signal("slowwrite") {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        self.handle_keyboard(ctx);

        // Update heading cache when text changes
        if self.doc.text.len() != self.last_text_len {
            self.cached_headings = self.doc.headings();
            self.last_text_len = self.doc.text.len();
            // Also refresh find matches if searching
            if self.show_find {
                self.update_find_matches();
            }
        }

        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect()
        });
        if let Some(path) = dropped.into_iter().next() {
            let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            if ext == "txt" || ext == "html" || ext == "htm" || ext == "md" {
                self.request_open_file(path);
            }
        }

        // Edge-detect focus mode transitions for fullscreen
        if self.focus_mode && !self.focus_was_active {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        } else if !self.focus_mode && self.focus_was_active {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }
        self.focus_was_active = self.focus_mode;

        if self.focus_mode {
            // Focus mode: fullscreen editor with hover-to-show menu bar

            // Hover near top to reveal menu bar
            let near_top = ctx.input(|i| {
                i.pointer.hover_pos().map_or(false, |p| p.y < 40.0)
            });
            if near_top {
                self.focus_menu_sticky = true;
            }
            let show_menu = self.focus_menu_sticky;
            if show_menu {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            }

            let mut win_action = WindowAction::None;
            if show_menu {
                egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
                    win_action = self.render_menu_bar(ui);
                });
            }

            // Post-render dismissal
            if self.focus_menu_sticky {
                let any_popup_open = ctx.memory(|mem| mem.any_popup_open());
                let near_top2 = ctx.input(|i| {
                    i.pointer.hover_pos().map_or(false, |p| p.y < 40.0)
                });
                if any_popup_open {
                    self.focus_popup_was_open = true;
                } else if self.focus_popup_was_open {
                    self.focus_popup_was_open = false;
                } else if !near_top2 {
                    let clicked = ctx.input(|i| i.pointer.primary_clicked());
                    if clicked {
                        self.focus_menu_sticky = false;
                    }
                }
            }

            egui::CentralPanel::default()
                .frame(egui::Frame::none()
                    .fill(SlowColors::WHITE)
                    .inner_margin(egui::Margin::symmetric(60.0, 30.0)))
                .show(ctx, |ui| { self.render_editor(ui); });

            // Escape exits focus mode
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                if self.focus_menu_sticky {
                    self.focus_menu_sticky = false;
                } else {
                    self.focus_mode = false;
                    self.focus_menu_sticky = false;
                }
            }

            match win_action {
                WindowAction::Close => {
                    if self.modified {
                        self.show_close_confirm = true;
                    } else {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                WindowAction::Minimize => {
                    self.focus_mode = false;
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
        } else {
            // Normal mode
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
            // Toolbar only in rich text mode
            if self.view_mode == ViewMode::RichText {
                egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(SlowColors::WHITE)
                        .stroke(egui::Stroke::new(slowcore::BORDER, SlowColors::BLACK))
                        .inner_margin(egui::Margin::symmetric(slowcore::theme::Grid::XS, 2.0))
                        .show(ui, |ui| { self.render_toolbar(ui); });
                });
            }
            egui::TopBottomPanel::top("title_bar").show(ctx, |ui| {
                slowcore::theme::SlowTheme::title_bar_frame().show(ui, |ui| {
                    ui.centered_and_justified(|ui| { ui.label(self.display_title()); });
                });
            });
            egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
                let words = self.doc.word_count();
                let chars = self.doc.char_count();
                let lines = self.doc.line_count();
                let status = if self.view_mode == ViewMode::PlainText {
                    // Simple status for plain text
                    if self.modified {
                        format!("modified  |  {} lines  {} words  {} chars", lines, words, chars)
                    } else {
                        format!("{} lines  {} words  {} chars", lines, words, chars)
                    }
                } else {
                    let pages = self.doc.page_estimate();
                    let reading = self.doc.reading_time_minutes();
                    let zoom_pct = (self.zoom * 100.0).round() as i32;
                    if self.modified {
                        format!("modified  |  page 1 of {}  |  {} words  {} chars  ~{} min read  |  {}%",
                            pages, words, chars, reading, zoom_pct)
                    } else {
                        format!("page 1 of {}  |  {} words  {} chars  ~{} min read  |  {}%",
                            pages, words, chars, reading, zoom_pct)
                    }
                };
                status_bar(ui, &status);
            });
            if self.show_find {
                egui::TopBottomPanel::bottom("find_bar").show(ctx, |ui| {
                    self.render_find_bar(ui);
                });
            }
            // Outline only in rich text mode
            if self.show_outline && self.view_mode == ViewMode::RichText {
                self.render_outline(ctx);
            }
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(0.0)))
                .show(ctx, |ui| { self.render_editor(ui); });

            // Escape handling
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                if self.show_find { self.show_find = false; self.show_replace = false; self.find_matches.clear(); }
                else if self.show_save_before_open { self.show_save_before_open = false; self.pending_action = None; }
                else if self.show_file_browser { self.show_file_browser = false; }
                else if self.show_close_confirm { self.show_close_confirm = false; }
                else if self.show_about { self.show_about = false; }
                else if self.show_shortcuts { self.show_shortcuts = false; }
            }
            if self.show_file_browser { self.render_file_browser(ctx); }
            if self.show_save_before_open { self.render_save_before_open(ctx); }
            if self.show_close_confirm { self.render_close_confirm(ctx); }
            if self.show_about { self.render_about(ctx); }
            if self.show_shortcuts { self.render_shortcuts(ctx); }
        }

        // Apply pending format after editor is shown (so TextEdit state exists)
        self.apply_pending_format(ctx);

        // Sync formatting spans with text changes
        if self.view_mode == ViewMode::RichText {
            let text_changed = self.doc.text != self.prev_text;
            self.sync_spans_with_text(ctx);
            self.update_cursor_formatting(ctx, text_changed);
        }

        // Autosave: every ~300 frames (~5 seconds at 60fps) if file has been named
        if self.modified && self.file_path.is_some() {
            self.autosave_counter += 1;
            if self.autosave_counter >= 300 {
                self.autosave_counter = 0;
                self.save_document();
            }
        } else {
            self.autosave_counter = 0;
        }

        // Jump to find match after rendering
        if self.show_find && !self.find_matches.is_empty() {
            self.jump_to_find_match(ctx);
        }

        if ctx.input(|i| i.viewport().close_requested()) {
            if self.modified && !self.close_confirmed {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.show_close_confirm = true;
            }
        }

        self.repaint.end_frame(ctx);
    }
}
