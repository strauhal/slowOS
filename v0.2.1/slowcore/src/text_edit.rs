//! Double-click-drag word selection for egui TextEdit widgets.
//!
//! Call `WordDragState::update()` after `TextEdit::show()` to get
//! double-click-hold-drag to extend selection by whole words.

use egui::Ui;

/// Tracks word-level drag-selection state for a TextEdit widget.
#[derive(Debug, Clone, Default)]
pub struct WordDragState {
    pub active: bool,
    anchor_start: usize,
    anchor_end: usize,
}

impl WordDragState {
    pub fn new() -> Self { Self::default() }

    /// Call this after `TextEdit::multiline(...).show(ui)`.
    /// Handles double-click to start word selection, then dragging
    /// extends selection by word boundaries.
    pub fn update(
        &mut self,
        ui: &Ui,
        output: &egui::text_edit::TextEditOutput,
        text: &str,
    ) {
        let text_id = output.response.id;
        let primary_down = ui.input(|i| i.pointer.primary_down());

        if output.response.double_clicked() {
            if let Some(cr) = &output.cursor_range {
                let char_idx = cr.primary.ccursor.index;
                let (ws, we) = word_boundaries(text, char_idx);
                self.anchor_start = ws;
                self.anchor_end = we;
                self.active = true;
            }
        }

        if self.active && primary_down && output.response.dragged() {
            if let Some(pointer_pos) = ui.input(|i| i.pointer.interact_pos()) {
                let local_pos = pointer_pos - output.galley_pos;
                let cursor = output.galley.cursor_from_pos(local_pos);
                let drag_char = cursor.ccursor.index;
                let (dws, dwe) = word_boundaries(text, drag_char);

                let sel_start = dws.min(self.anchor_start);
                let sel_end = dwe.max(self.anchor_end);

                let primary_idx = if drag_char < self.anchor_start { sel_start } else { sel_end };
                let secondary_idx = if drag_char < self.anchor_start { sel_end } else { sel_start };

                let mut state = output.state.clone();
                state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(secondary_idx),
                    egui::text::CCursor::new(primary_idx),
                )));
                state.store(ui.ctx(), text_id);
            }
        }

        if !primary_down {
            self.active = false;
        }
    }
}

/// Find word boundaries around a character index.
/// Returns (start, end) as character indices.
pub fn word_boundaries(text: &str, char_idx: usize) -> (usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    let pos = char_idx.min(chars.len());
    if chars.is_empty() { return (0, 0); }

    let mut start = pos;
    while start > 0 && is_word_char(chars[start - 1]) {
        start -= 1;
    }

    let mut end = pos;
    while end < chars.len() && is_word_char(chars[end]) {
        end += 1;
    }

    // If on whitespace/punctuation, select just that character
    if start == end && pos < chars.len() {
        return (pos, pos + 1);
    }

    (start, end)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '\''
}
