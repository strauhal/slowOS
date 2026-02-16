//! Rich text model for slowWrite — per-character styling
//!
//! Every character can have its own font family, size, weight, and decorations.
//! The model stores a parallel Vec<CharStyle> alongside the text content.

use serde::{Deserialize, Serialize};

/// Style properties for a single character
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub font_size: f32,
    pub font_family: FontFamily,
}

impl Default for CharStyle {
    fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            font_size: 16.0,
            font_family: FontFamily::Proportional,
        }
    }
}

impl CharStyle {
    pub fn with_bold(mut self, b: bool) -> Self { self.bold = b; self }
    pub fn with_italic(mut self, i: bool) -> Self { self.italic = i; self }
    pub fn with_underline(mut self, u: bool) -> Self { self.underline = u; self }
    pub fn with_strikethrough(mut self, s: bool) -> Self { self.strikethrough = s; self }
    pub fn with_font_size(mut self, size: f32) -> Self { self.font_size = size; self }
    pub fn with_font_family(mut self, family: FontFamily) -> Self { self.font_family = family; self }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FontFamily {
    Proportional,
    Monospace,
}

/// A rich text document: plain text + per-character styles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RichDocument {
    /// The actual text content
    pub text: String,
    /// One style per character (same length as text.chars().count())
    pub styles: Vec<CharStyle>,
    /// The "current" style used when typing new characters
    #[serde(skip)]
    pub cursor_style: CharStyle,
}

impl Default for RichDocument {
    fn default() -> Self {
        Self {
            text: String::new(),
            styles: Vec::new(),
            cursor_style: CharStyle::default(),
        }
    }
}

impl RichDocument {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create from plain text (all default style)
    pub fn from_plain_text(text: String) -> Self {
        let char_count = text.chars().count();
        Self {
            text,
            styles: vec![CharStyle::default(); char_count],
            cursor_style: CharStyle::default(),
        }
    }

    /// Ensure styles vector matches text length
    pub fn sync_styles(&mut self) {
        let char_count = self.text.chars().count();
        // If styles are shorter, extend with cursor_style
        while self.styles.len() < char_count {
            self.styles.push(self.cursor_style.clone());
        }
        // If styles are longer, truncate
        self.styles.truncate(char_count);
    }

    /// Apply a style modification to a character range (char indices, not byte)
    pub fn apply_style_range(&mut self, start: usize, end: usize, modify: impl Fn(&mut CharStyle)) {
        let start = start.min(self.styles.len());
        let end = end.min(self.styles.len());
        for i in start..end {
            modify(&mut self.styles[i]);
        }
    }

    /// Toggle bold on a range
    pub fn toggle_bold(&mut self, start: usize, end: usize) {
        let all_bold = (start..end.min(self.styles.len()))
            .all(|i| self.styles[i].bold);
        self.apply_style_range(start, end, |s| s.bold = !all_bold);
    }

    /// Toggle italic on a range
    pub fn toggle_italic(&mut self, start: usize, end: usize) {
        let all_italic = (start..end.min(self.styles.len()))
            .all(|i| self.styles[i].italic);
        self.apply_style_range(start, end, |s| s.italic = !all_italic);
    }

    /// Toggle underline on a range
    pub fn toggle_underline(&mut self, start: usize, end: usize) {
        let all_underline = (start..end.min(self.styles.len()))
            .all(|i| self.styles[i].underline);
        self.apply_style_range(start, end, |s| s.underline = !all_underline);
    }

    /// Toggle strikethrough on a range
    pub fn toggle_strikethrough(&mut self, start: usize, end: usize) {
        let all_strike = (start..end.min(self.styles.len()))
            .all(|i| self.styles[i].strikethrough);
        self.apply_style_range(start, end, |s| s.strikethrough = !all_strike);
    }

    /// Set font size on a range
    pub fn set_font_size(&mut self, start: usize, end: usize, size: f32) {
        self.apply_style_range(start, end, |s| s.font_size = size);
    }

    /// Set font family on a range
    pub fn set_font_family(&mut self, start: usize, end: usize, family: FontFamily) {
        self.apply_style_range(start, end, |s| s.font_family = family);
    }

    /// Increase font size on a range
    pub fn increase_font_size(&mut self, start: usize, end: usize) {
        self.apply_style_range(start, end, |s| {
            s.font_size = (s.font_size + 2.0).min(72.0);
        });
    }

    /// Decrease font size on a range
    pub fn decrease_font_size(&mut self, start: usize, end: usize) {
        self.apply_style_range(start, end, |s| {
            s.font_size = (s.font_size - 2.0).max(8.0);
        });
    }

    pub fn word_count(&self) -> usize {
        self.text.split_whitespace().count()
    }

    pub fn char_count(&self) -> usize {
        self.text.chars().count()
    }

    pub fn line_count(&self) -> usize {
        self.text.lines().count().max(1)
    }

    /// Convert char index to byte index
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        self.text.char_indices()
            .nth(char_idx)
            .map(|(i, _)| i)
            .unwrap_or(self.text.len())
    }

    /// Convert byte index to char index
    pub fn byte_to_char(&self, byte_idx: usize) -> usize {
        self.text[..byte_idx.min(self.text.len())].chars().count()
    }
}

/// Serialize a RichDocument to our simple JSON format
pub fn save_rich_document(doc: &RichDocument) -> String {
    serde_json::to_string_pretty(doc).unwrap_or_default()
}

/// Load a RichDocument from JSON
pub fn load_rich_document(json: &str) -> Option<RichDocument> {
    serde_json::from_str(json).ok()
}
