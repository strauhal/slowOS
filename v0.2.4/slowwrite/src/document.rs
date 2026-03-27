//! Document model for slowWrite
//!
//! Plain text with optional markdown syntax for structure.
//! Headings, bold, italic, lists — stored as text, rendered with formatting.

/// A text document
#[derive(Debug, Clone)]
pub struct Document {
    pub text: String,
}

impl Document {
    pub fn new() -> Self {
        Self {
            text: String::new(),
        }
    }

    pub fn from_text(text: String) -> Self {
        Self { text }
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

    pub fn page_estimate(&self) -> usize {
        // ~250 words per page
        (self.word_count() as f32 / 250.0).ceil().max(1.0) as usize
    }

    pub fn reading_time_minutes(&self) -> usize {
        // ~200 words per minute
        let wc = self.word_count();
        if wc == 0 { return 0; }
        ((wc as f32) / 200.0).ceil() as usize
    }

    /// Extract headings for the outline panel.
    /// Returns (level, heading_text, byte_offset_of_line_start).
    pub fn headings(&self) -> Vec<(usize, String, usize)> {
        let mut headings = Vec::new();
        let mut offset = 0;
        for line in self.text.split('\n') {
            let trimmed = line.trim_start();
            if trimmed.starts_with("### ") && trimmed.len() > 4 {
                headings.push((3, trimmed[4..].to_string(), offset));
            } else if trimmed.starts_with("## ") && trimmed.len() > 3 {
                headings.push((2, trimmed[3..].to_string(), offset));
            } else if trimmed.starts_with("# ") && trimmed.len() > 2 {
                headings.push((1, trimmed[2..].to_string(), offset));
            }
            offset += line.len() + 1; // +1 for the \n
        }
        headings
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}
