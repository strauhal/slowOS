//! Text editor widget for SlowWrite
//! 
//! Handles cursor movement, selection, and text rendering.
//! Uses egui's text layout system for proper proportional font kerning.
//!
//! Selection modes:
//!   - Click: position cursor
//!   - Double-click: select word
//!   - Triple-click: select line
//!   - Click+drag: select arbitrary range
//!   - Shift+click: extend selection
//!   - Shift+arrow: extend selection

use crate::document::Document;
use egui::{FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};
use slowcore::theme::SlowColors;

/// Cursor position in the document
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cursor {
    pub pos: usize,
    pub anchor: Option<usize>,
}

impl Cursor {
    pub fn new(pos: usize) -> Self {
        Self { pos, anchor: None }
    }
    
    pub fn start_selection(&mut self) {
        if self.anchor.is_none() {
            self.anchor = Some(self.pos);
        }
    }
    
    pub fn clear_selection(&mut self) {
        self.anchor = None;
    }
    
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        self.anchor.map(|anchor| {
            if anchor < self.pos { (anchor, self.pos) } else { (self.pos, anchor) }
        })
    }
    
    pub fn has_selection(&self) -> bool {
        self.anchor.is_some() && self.anchor != Some(self.pos)
    }
}

/// Editor state and rendering
pub struct Editor {
    pub cursor: Cursor,
    pub scroll_offset: Vec2,
    pub line_height: f32,
    pub font_size: f32,
    pub left_margin: f32,
    pub cursor_visible: bool,
    cursor_blink_time: f64,
    pub find_query: String,
    pub replace_query: String,
    pub find_results: Vec<(usize, usize)>,
    pub current_find_index: Option<usize>,
    /// Mouse drag state for text selection
    is_dragging: bool,
    /// Multi-click tracking
    last_click_time: f64,
    click_count: u32,
    last_click_pos: usize,
    /// Word-select anchor for double-click+drag
    word_sel_start: Option<usize>,
    word_sel_end: Option<usize>,
}

impl Default for Editor {
    fn default() -> Self { Self::new() }
}

impl Editor {
    pub fn new() -> Self {
        Self {
            cursor: Cursor::default(),
            scroll_offset: Vec2::ZERO,
            line_height: 22.0,
            font_size: 14.0,
            left_margin: 50.0,
            cursor_visible: true,
            cursor_blink_time: 0.0,
            find_query: String::new(),
            replace_query: String::new(),
            find_results: Vec::new(),
            current_find_index: None,
            is_dragging: false,
            last_click_time: 0.0,
            click_count: 0,
            last_click_pos: usize::MAX,
            word_sel_start: None,
            word_sel_end: None,
        }
    }
    
    // ---------------------------------------------------------------
    // Text measurement helpers
    // ---------------------------------------------------------------
    
    fn measure_char_x(ctx: &egui::Context, text: &str, char_idx: usize, font: &FontId) -> f32 {
        if char_idx == 0 || text.is_empty() { return 0.0; }
        let prefix: String = text.chars().take(char_idx).collect();
        ctx.fonts(|f| f.layout_no_wrap(prefix, font.clone(), SlowColors::BLACK)).size().x
    }
    
    fn x_to_char(ctx: &egui::Context, text: &str, target_x: f32, font: &FontId) -> usize {
        if text.is_empty() || target_x <= 0.0 { return 0; }
        let chars: Vec<char> = text.chars().collect();
        let mut prev_x = 0.0f32;
        for i in 1..=chars.len() {
            let prefix: String = chars[..i].iter().collect();
            let curr_x = ctx.fonts(|f| f.layout_no_wrap(prefix, font.clone(), SlowColors::BLACK)).size().x;
            if curr_x >= target_x {
                let mid = (prev_x + curr_x) / 2.0;
                return if target_x < mid { i - 1 } else { i };
            }
            prev_x = curr_x;
        }
        chars.len()
    }
    
    /// Convert screen position to character offset in document
    fn screen_to_char_pos(&self, pos: Pos2, doc: &Document, rect: Rect, text_area: Rect, font: &FontId, ctx: &egui::Context) -> usize {
        let rel_y = pos.y - rect.min.y + self.scroll_offset.y;
        let line = ((rel_y / self.line_height) as usize).min(doc.line_count().saturating_sub(1));
        let rel_x = (pos.x - text_area.min.x).max(0.0);
        let line_text = doc.line(line).map(|l| l.trim_end_matches('\n').to_string()).unwrap_or_default();
        let col = Self::x_to_char(ctx, &line_text, rel_x, font);
        doc.line_col_to_char(line, col)
    }
    
    /// Find word boundaries around a character position.
    fn word_boundaries_at(doc: &Document, char_pos: usize) -> (usize, usize) {
        let text = doc.content.to_string();
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let pos = char_pos.min(len);
        if pos >= len { return (len, len); }
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let ch = chars[pos];
        if is_word(ch) {
            let mut start = pos;
            while start > 0 && is_word(chars[start - 1]) { start -= 1; }
            let mut end = pos;
            while end < len && is_word(chars[end]) { end += 1; }
            (start, end)
        } else if ch.is_whitespace() {
            let mut start = pos;
            while start > 0 && chars[start - 1].is_whitespace() && chars[start - 1] != '\n' { start -= 1; }
            let mut end = pos;
            while end < len && chars[end].is_whitespace() && chars[end] != '\n' { end += 1; }
            (start, end)
        } else {
            (pos, (pos + 1).min(len))
        }
    }
    
    /// Find line boundaries around a character position.
    fn line_boundaries_at(doc: &Document, char_pos: usize) -> (usize, usize) {
        let (line, _) = doc.char_to_line_col(char_pos);
        let line_start = doc.line_col_to_char(line, 0);
        let line_content = doc.line(line).unwrap_or_default();
        let line_end = line_start + line_content.chars().count();
        (line_start, line_end)
    }
    
    // ---------------------------------------------------------------
    // Cursor movement
    // ---------------------------------------------------------------
    
    pub fn move_left(&mut self, _doc: &Document, select: bool) {
        if select { self.cursor.start_selection(); }
        else if let Some((start, _)) = self.cursor.selection_range() {
            self.cursor.pos = start; self.cursor.clear_selection(); return;
        }
        if self.cursor.pos > 0 { self.cursor.pos -= 1; }
        if !select { self.cursor.clear_selection(); }
    }
    
    pub fn move_right(&mut self, doc: &Document, select: bool) {
        if select { self.cursor.start_selection(); }
        else if let Some((_, end)) = self.cursor.selection_range() {
            self.cursor.pos = end; self.cursor.clear_selection(); return;
        }
        if self.cursor.pos < doc.char_count() { self.cursor.pos += 1; }
        if !select { self.cursor.clear_selection(); }
    }
    
    pub fn move_up(&mut self, doc: &Document, select: bool) {
        if select { self.cursor.start_selection(); }
        let (line, col) = doc.char_to_line_col(self.cursor.pos);
        if line > 0 { self.cursor.pos = doc.line_col_to_char(line - 1, col); }
        if !select { self.cursor.clear_selection(); }
    }
    
    pub fn move_down(&mut self, doc: &Document, select: bool) {
        if select { self.cursor.start_selection(); }
        let (line, col) = doc.char_to_line_col(self.cursor.pos);
        if line < doc.line_count().saturating_sub(1) { self.cursor.pos = doc.line_col_to_char(line + 1, col); }
        if !select { self.cursor.clear_selection(); }
    }
    
    pub fn move_to_line_start(&mut self, doc: &Document, select: bool) {
        if select { self.cursor.start_selection(); }
        let (line, _) = doc.char_to_line_col(self.cursor.pos);
        self.cursor.pos = doc.line_col_to_char(line, 0);
        if !select { self.cursor.clear_selection(); }
    }
    
    pub fn move_to_line_end(&mut self, doc: &Document, select: bool) {
        if select { self.cursor.start_selection(); }
        let (line, _) = doc.char_to_line_col(self.cursor.pos);
        if let Some(lc) = doc.line(line) {
            let ll = lc.trim_end_matches('\n').chars().count();
            self.cursor.pos = doc.line_col_to_char(line, ll);
        }
        if !select { self.cursor.clear_selection(); }
    }
    
    pub fn move_word_left(&mut self, doc: &Document, select: bool) {
        if select { self.cursor.start_selection(); }
        let text = doc.content.to_string();
        let chars: Vec<char> = text.chars().collect();
        let mut pos = self.cursor.pos;
        while pos > 0 && chars.get(pos - 1).map(|c| c.is_whitespace()).unwrap_or(false) { pos -= 1; }
        while pos > 0 && chars.get(pos - 1).map(|c| !c.is_whitespace()).unwrap_or(false) { pos -= 1; }
        self.cursor.pos = pos;
        if !select { self.cursor.clear_selection(); }
    }
    
    pub fn move_word_right(&mut self, doc: &Document, select: bool) {
        if select { self.cursor.start_selection(); }
        let text = doc.content.to_string();
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let mut pos = self.cursor.pos;
        while pos < len && chars.get(pos).map(|c| !c.is_whitespace()).unwrap_or(false) { pos += 1; }
        while pos < len && chars.get(pos).map(|c| c.is_whitespace()).unwrap_or(false) { pos += 1; }
        self.cursor.pos = pos;
        if !select { self.cursor.clear_selection(); }
    }
    
    pub fn select_all(&mut self, doc: &Document) {
        self.cursor.anchor = Some(0);
        self.cursor.pos = doc.char_count();
    }
    
    // ---------------------------------------------------------------
    // Editing
    // ---------------------------------------------------------------
    
    pub fn insert_text(&mut self, doc: &mut Document, text: &str) {
        doc.save_undo_state(self.cursor.pos);
        if let Some((start, end)) = self.cursor.selection_range() {
            doc.delete_range(start, end);
            self.cursor.pos = start;
            self.cursor.clear_selection();
        }
        doc.insert(self.cursor.pos, text);
        self.cursor.pos += text.chars().count();
    }
    
    pub fn backspace(&mut self, doc: &mut Document) {
        doc.save_undo_state(self.cursor.pos);
        if let Some((start, end)) = self.cursor.selection_range() {
            doc.delete_range(start, end); self.cursor.pos = start; self.cursor.clear_selection();
        } else if self.cursor.pos > 0 { self.cursor.pos -= 1; doc.delete(self.cursor.pos); }
    }
    
    pub fn delete(&mut self, doc: &mut Document) {
        doc.save_undo_state(self.cursor.pos);
        if let Some((start, end)) = self.cursor.selection_range() {
            doc.delete_range(start, end); self.cursor.pos = start; self.cursor.clear_selection();
        } else { doc.delete(self.cursor.pos); }
    }
    
    pub fn kill_to_line_end(&mut self, doc: &mut Document) -> Option<String> {
        doc.save_undo_state(self.cursor.pos);
        let (line, col) = doc.char_to_line_col(self.cursor.pos);
        if let Some(lc) = doc.line(line) {
            let ll = lc.trim_end_matches('\n').chars().count();
            if col < ll {
                let end_pos = doc.line_col_to_char(line, ll);
                let killed = doc.get_range(self.cursor.pos, end_pos);
                doc.delete_range(self.cursor.pos, end_pos);
                return Some(killed);
            } else if self.cursor.pos < doc.char_count() {
                let killed = doc.get_range(self.cursor.pos, self.cursor.pos + 1);
                doc.delete(self.cursor.pos);
                return Some(killed);
            }
        }
        None
    }
    
    pub fn selected_text(&self, doc: &Document) -> Option<String> {
        self.cursor.selection_range().map(|(s, e)| doc.get_range(s, e))
    }
    
    // ---------------------------------------------------------------
    // Find / Replace
    // ---------------------------------------------------------------
    
    pub fn find(&mut self, doc: &Document) {
        self.find_results = doc.find_all(&self.find_query);
        self.current_find_index = if self.find_results.is_empty() { None } else { Some(0) };
    }
    
    pub fn find_next(&mut self) {
        if !self.find_results.is_empty() {
            self.current_find_index = Some(
                self.current_find_index.map(|i| (i + 1) % self.find_results.len()).unwrap_or(0)
            );
            if let Some(idx) = self.current_find_index {
                let (start, end) = self.find_results[idx];
                self.cursor.anchor = Some(start); self.cursor.pos = end;
            }
        }
    }
    
    pub fn replace_current(&mut self, doc: &mut Document) {
        if let Some(idx) = self.current_find_index {
            if idx < self.find_results.len() {
                let (start, end) = self.find_results[idx];
                doc.save_undo_state(self.cursor.pos);
                doc.replace(start, end, &self.replace_query);
                self.find(doc);
            }
        }
    }
    
    pub fn replace_all(&mut self, doc: &mut Document) {
        if !self.find_results.is_empty() {
            doc.save_undo_state(self.cursor.pos);
            for (start, end) in self.find_results.iter().rev() { doc.replace(*start, *end, &self.replace_query); }
            self.find_results.clear(); self.current_find_index = None;
        }
    }
    
    // ---------------------------------------------------------------
    // Cursor blink
    // ---------------------------------------------------------------
    
    pub fn update(&mut self, dt: f64) {
        self.cursor_blink_time += dt;
        if self.cursor_blink_time >= 0.5 { self.cursor_blink_time = 0.0; self.cursor_visible = !self.cursor_visible; }
    }
    
    pub fn reset_blink(&mut self) { self.cursor_visible = true; self.cursor_blink_time = 0.0; }
    
    // ---------------------------------------------------------------
    // Rendering — with full mouse interaction for selection
    // ---------------------------------------------------------------
    
    pub fn render(&mut self, ui: &mut Ui, doc: &Document, rect: Rect) -> Response {
        let ctx = ui.ctx().clone();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        
        painter.rect_filled(rect, 0.0, SlowColors::WHITE);
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));
        
        let text_area = Rect::from_min_max(
            rect.min + Vec2::new(self.left_margin, 4.0),
            rect.max - Vec2::new(4.0, 4.0),
        );
        let font = FontId::proportional(self.font_size);
        
        // ---- Mouse interaction ----
        let time = ctx.input(|i| i.time);
        let shift_held = ctx.input(|i| i.modifiers.shift);
        
        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                let char_pos = self.screen_to_char_pos(pos, doc, rect, text_area, &font, &ctx);
                
                // Detect multi-click: same approximate position within 0.4s
                let near_last = {
                    let (line_a, _) = doc.char_to_line_col(char_pos);
                    let (line_b, _) = doc.char_to_line_col(self.last_click_pos.min(doc.char_count()));
                    line_a == line_b && ((char_pos as isize) - (self.last_click_pos as isize)).unsigned_abs() < 3
                };
                let is_multi = (time - self.last_click_time) < 0.4 && near_last;
                if is_multi { self.click_count += 1; } else { self.click_count = 1; }
                self.last_click_time = time;
                self.last_click_pos = char_pos;
                
                match self.click_count {
                    1 => {
                        if shift_held {
                            self.cursor.start_selection();
                            self.cursor.pos = char_pos;
                        } else {
                            self.cursor.pos = char_pos;
                            self.cursor.anchor = Some(char_pos);
                        }
                        self.is_dragging = true;
                        self.word_sel_start = None;
                        self.word_sel_end = None;
                    }
                    2 => {
                        let (start, end) = Self::word_boundaries_at(doc, char_pos);
                        self.cursor.anchor = Some(start);
                        self.cursor.pos = end;
                        self.word_sel_start = Some(start);
                        self.word_sel_end = Some(end);
                        self.is_dragging = true; // Allow drag to extend word-by-word
                    }
                    _ => {
                        let (start, end) = Self::line_boundaries_at(doc, char_pos);
                        self.cursor.anchor = Some(start);
                        self.cursor.pos = end;
                        self.is_dragging = false;
                        self.word_sel_start = None;
                        self.word_sel_end = None;
                        self.click_count = 0;
                    }
                }
                self.reset_blink();
            }
        }
        
        if response.dragged() && self.is_dragging {
            if let Some(pos) = response.interact_pointer_pos() {
                let char_pos = self.screen_to_char_pos(pos, doc, rect, text_area, &font, &ctx);
                
                if let (Some(ws), Some(we)) = (self.word_sel_start, self.word_sel_end) {
                    // Word-by-word extension from double-click
                    let (word_start, word_end) = Self::word_boundaries_at(doc, char_pos);
                    if char_pos < ws {
                        self.cursor.anchor = Some(we);
                        self.cursor.pos = word_start;
                    } else {
                        self.cursor.anchor = Some(ws);
                        self.cursor.pos = word_end;
                    }
                } else {
                    // Normal char-by-char drag
                    self.cursor.pos = char_pos;
                }
                self.reset_blink();
            }
        }
        
        if response.drag_stopped() && self.is_dragging {
            self.is_dragging = false;
            self.word_sel_start = None;
            self.word_sel_end = None;
            if self.cursor.anchor == Some(self.cursor.pos) { self.cursor.clear_selection(); }
        }
        
        // Scroll
        let scroll_delta = ctx.input(|i| i.smooth_scroll_delta.y);
        if scroll_delta != 0.0 && response.hovered() {
            self.scroll_offset.y = (self.scroll_offset.y - scroll_delta).max(0.0);
        }
        
        // ---- Rendering ----
        let first_visible_line = (self.scroll_offset.y / self.line_height) as usize;
        let visible_lines = ((rect.height() / self.line_height) as usize) + 2;
        let last_visible_line = (first_visible_line + visible_lines).min(doc.line_count());
        
        painter.vline(rect.min.x + self.left_margin - 2.0, rect.min.y..=rect.max.y, Stroke::new(1.0, SlowColors::BLACK));
        
        for line_idx in first_visible_line..last_visible_line {
            let y = rect.min.y + (line_idx as f32 * self.line_height) - self.scroll_offset.y;
            if y < rect.min.y - self.line_height || y > rect.max.y { continue; }
            
            // Line number
            painter.text(
                Pos2::new(rect.min.x + self.left_margin - 8.0, y + self.line_height / 2.0),
                egui::Align2::RIGHT_CENTER,
                format!("{}", line_idx + 1),
                font.clone(),
                SlowColors::BLACK,
            );
            
            if let Some(line_content) = doc.line(line_idx) {
                let line_start_char = doc.content.line_to_char(line_idx);
                let line_display = line_content.trim_end_matches('\n');
                let line_char_count = line_display.chars().count();
                let line_end_char = line_start_char + line_char_count;
                
                // Check if selection intersects this line
                let sel_on_line = self.cursor.selection_range().and_then(|(sel_start, sel_end)| {
                    if sel_end > line_start_char && sel_start < line_end_char + 1 {
                        let ls = sel_start.max(line_start_char) - line_start_char;
                        let le = sel_end.min(line_end_char) - line_start_char;
                        Some((ls, le))
                    } else { None }
                });
                
                if let Some((local_start, local_end)) = sel_on_line {
                    // Draw selection background
                    let x_start = Self::measure_char_x(&ctx, line_display, local_start, &font);
                    let x_end = if local_end >= line_char_count && self.cursor.selection_range().map(|(_, se)| se > line_end_char).unwrap_or(false) {
                        Self::measure_char_x(&ctx, line_display, line_char_count, &font) + 8.0
                    } else {
                        Self::measure_char_x(&ctx, line_display, local_end, &font)
                    };
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(text_area.min.x + x_start, y),
                            Pos2::new(text_area.min.x + x_end, y + self.line_height),
                        ), 0.0, SlowColors::BLACK,
                    );
                    
                    // Draw text in segments: unselected black, selected white
                    let chars: Vec<char> = line_display.chars().collect();
                    if local_start > 0 {
                        let before: String = chars[..local_start].iter().collect();
                        painter.text(Pos2::new(text_area.min.x, y + self.line_height / 2.0), egui::Align2::LEFT_CENTER, &before, font.clone(), SlowColors::BLACK);
                    }
                    let sel_text: String = chars[local_start..local_end.min(chars.len())].iter().collect();
                    let x_sel = Self::measure_char_x(&ctx, line_display, local_start, &font);
                    painter.text(Pos2::new(text_area.min.x + x_sel, y + self.line_height / 2.0), egui::Align2::LEFT_CENTER, &sel_text, font.clone(), SlowColors::WHITE);
                    if local_end < chars.len() {
                        let after: String = chars[local_end..].iter().collect();
                        let x_after = Self::measure_char_x(&ctx, line_display, local_end, &font);
                        painter.text(Pos2::new(text_area.min.x + x_after, y + self.line_height / 2.0), egui::Align2::LEFT_CENTER, &after, font.clone(), SlowColors::BLACK);
                    }
                } else if !line_display.is_empty() {
                    painter.text(Pos2::new(text_area.min.x, y + self.line_height / 2.0), egui::Align2::LEFT_CENTER, line_display, font.clone(), SlowColors::BLACK);
                }
            }
        }
        
        // Cursor
        if self.cursor_visible {
            let (cl, cc) = doc.char_to_line_col(self.cursor.pos);
            let cy = rect.min.y + (cl as f32 * self.line_height) - self.scroll_offset.y;
            if cy >= rect.min.y && cy <= rect.max.y - self.line_height {
                let lt = doc.line(cl).map(|l| l.trim_end_matches('\n').to_string()).unwrap_or_default();
                let cx = text_area.min.x + Self::measure_char_x(&ctx, &lt, cc, &font);
                painter.vline(cx, cy..=cy + self.line_height, Stroke::new(2.0, SlowColors::BLACK));
            }
        }
        
        response
    }
    
    pub fn ensure_cursor_visible(&mut self, doc: &Document, view_height: f32) {
        let (cl, _) = doc.char_to_line_col(self.cursor.pos);
        let cy = cl as f32 * self.line_height;
        if cy < self.scroll_offset.y { self.scroll_offset.y = cy; }
        if cy + self.line_height > self.scroll_offset.y + view_height {
            self.scroll_offset.y = cy + self.line_height - view_height;
        }
    }
}
