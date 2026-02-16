//! Custom text editing widget with improved selection behavior.
//!
//! Supports double-click-drag to select whole words at a time,
//! continuing to extend selection by word boundaries while holding down.

use egui::{FontId, Pos2, Ui};

/// Word-level selection state for double-click-drag
#[derive(Debug, Clone, Default)]
pub struct WordSelectState {
    /// Whether we're in word-selection mode (double-click held down)
    pub active: bool,
    /// The anchor word boundaries (start..end byte indices) from the initial double-click
    pub anchor_word_start: usize,
    pub anchor_word_end: usize,
    /// Current selection range (byte indices)
    pub sel_start: usize,
    pub sel_end: usize,
}

/// Find word boundaries around a byte position in text.
/// Returns (word_start, word_end) as byte indices.
pub fn word_boundaries(text: &str, byte_pos: usize) -> (usize, usize) {
    let byte_pos = byte_pos.min(text.len());
    if text.is_empty() {
        return (0, 0);
    }

    // Find start: scan backwards to find word boundary
    let mut start = byte_pos;
    while start > 0 {
        let prev = prev_char_boundary(text, start);
        let c = text[prev..start].chars().next().unwrap_or(' ');
        if is_word_char(c) {
            start = prev;
        } else {
            break;
        }
    }

    // Find end: scan forward to find word boundary
    let mut end = byte_pos;
    while end < text.len() {
        let next = next_char_boundary(text, end);
        let c = text[end..next].chars().next().unwrap_or(' ');
        if is_word_char(c) {
            end = next;
        } else {
            break;
        }
    }

    // If we clicked on whitespace/punctuation, select just that character
    if start == end && byte_pos < text.len() {
        let next = next_char_boundary(text, byte_pos);
        return (byte_pos, next);
    }

    (start, end)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '\''
}

fn prev_char_boundary(text: &str, pos: usize) -> usize {
    let mut p = pos.saturating_sub(1);
    while p > 0 && !text.is_char_boundary(p) {
        p -= 1;
    }
    p
}

fn next_char_boundary(text: &str, pos: usize) -> usize {
    let mut p = pos + 1;
    while p < text.len() && !text.is_char_boundary(p) {
        p += 1;
    }
    p.min(text.len())
}

/// Convert a screen position to a character/byte index in text,
/// given font metrics and layout info.
pub fn pos_to_byte_index(
    text: &str,
    pos: Pos2,
    text_origin: Pos2,
    font: &FontId,
    row_height: f32,
    ui: &Ui,
) -> usize {
    if text.is_empty() {
        return 0;
    }

    let rel_y = pos.y - text_origin.y;
    let target_row = (rel_y / row_height).floor().max(0.0) as usize;

    // Find the byte index of the target row
    let mut current_row = 0;
    let mut row_start = 0;
    for (i, c) in text.char_indices() {
        if current_row == target_row {
            row_start = i;
            break;
        }
        if c == '\n' {
            current_row += 1;
            row_start = i + 1;
        }
    }
    if current_row < target_row {
        return text.len();
    }

    // Find end of this row
    let row_end = text[row_start..].find('\n')
        .map(|p| row_start + p)
        .unwrap_or(text.len());

    let row_text = &text[row_start..row_end];
    let rel_x = pos.x - text_origin.x;

    // Walk characters to find closest position
    let mut x_accum = 0.0;

    for (i, c) in row_text.char_indices() {
        let char_width = ui.fonts(|f| f.glyph_width(font, c));
        if rel_x < x_accum + char_width * 0.5 {
            return row_start + i;
        }
        x_accum += char_width;
    }

    row_end
}
