//! Double-click-drag word selection for egui TextEdit widgets.
//!
//! Call `WordDragState::update()` after `TextEdit::show()` to get
//! double-click-hold-drag to extend selection by whole words.

use egui::Ui;
use std::time::Instant;

/// Maximum time between first click-release and second press to count as double-click.
const DOUBLE_CLICK_TIME: f64 = 0.4; // seconds
/// Maximum pixel distance between first and second click to count as double-click.
const DOUBLE_CLICK_DIST: f32 = 6.0;

/// Tracks word-level drag-selection state for a TextEdit widget.
#[derive(Debug, Clone)]
pub struct WordDragState {
    pub active: bool,
    anchor_start: usize,
    anchor_end: usize,
    /// Position and time of the last click release (for detecting double-press)
    last_click_pos: Option<egui::Pos2>,
    last_click_time: Option<Instant>,
    /// Whether we already processed the current press as a double-press
    press_handled: bool,
}

impl Default for WordDragState {
    fn default() -> Self {
        Self {
            active: false,
            anchor_start: 0,
            anchor_end: 0,
            last_click_pos: None,
            last_click_time: None,
            press_handled: false,
        }
    }
}

impl WordDragState {
    pub fn new() -> Self { Self::default() }

    /// Call this after `TextEdit::multiline(...).show(ui)`.
    /// Handles double-click (including hold-and-drag on the second press)
    /// to start word selection, then extends selection by word boundaries
    /// in any direction as the pointer moves.
    pub fn update(
        &mut self,
        ui: &Ui,
        output: &egui::text_edit::TextEditOutput,
        text: &str,
    ) {
        let text_id = output.response.id;
        let primary_down = ui.input(|i| i.pointer.primary_down());
        let primary_pressed = ui.input(|i| i.pointer.primary_pressed());
        let primary_released = ui.input(|i| i.pointer.primary_released());
        let pointer_pos = ui.input(|i| i.pointer.interact_pos());

        // On click release over the text area, record time and position
        // so we can detect a quick second press (double-click-hold).
        if primary_released && output.response.hovered() {
            if let Some(pos) = pointer_pos {
                self.last_click_pos = Some(pos);
                self.last_click_time = Some(Instant::now());
            }
            self.press_handled = false;
        }

        // On press, check if this is a "second press" close in time/space.
        // This fires immediately on mouse-down so the user can hold and drag
        // to extend word selection without releasing the second click first.
        if primary_pressed && !self.press_handled && output.response.hovered() {
            if let (Some(last_pos), Some(last_time)) = (self.last_click_pos, self.last_click_time) {
                let elapsed = last_time.elapsed().as_secs_f64();
                if let Some(pos) = pointer_pos {
                    let dist = (pos - last_pos).length();
                    if elapsed < DOUBLE_CLICK_TIME && dist < DOUBLE_CLICK_DIST {
                        // Double-press detected — activate word selection
                        let local_pos = pos - output.galley_pos;
                        let cursor = output.galley.cursor_from_pos(local_pos);
                        let char_idx = cursor.ccursor.index;
                        let (ws, we) = word_boundaries(text, char_idx);
                        self.anchor_start = ws;
                        self.anchor_end = we;
                        self.active = true;
                        self.press_handled = true;
                        self.last_click_pos = None;
                        self.last_click_time = None;

                        // Set initial word selection
                        let mut state = output.state.clone();
                        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                            egui::text::CCursor::new(ws),
                            egui::text::CCursor::new(we),
                        )));
                        state.store(ui.ctx(), text_id);
                    }
                }
            }
        }

        // Also handle the standard egui double_clicked() for cases where
        // the second click is released quickly (tap-tap rather than tap-hold)
        if output.response.double_clicked() && !self.active {
            if let Some(cr) = &output.cursor_range {
                let char_idx = cr.primary.ccursor.index;
                let (ws, we) = word_boundaries(text, char_idx);
                self.anchor_start = ws;
                self.anchor_end = we;
                self.active = true;
            }
        }

        // While active and pointer held, extend selection by word boundaries
        // as the pointer moves in any direction.
        if self.active && primary_down {
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
