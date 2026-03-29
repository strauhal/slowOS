//! Document model for slowWrite
//!
//! Plain text with optional HTML tags for structure.
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
        // Strip HTML tags for word counting
        strip_tags(&self.text).split_whitespace().count()
    }

    pub fn char_count(&self) -> usize {
        strip_tags(&self.text).chars().count()
    }

    pub fn line_count(&self) -> usize {
        self.text.lines().count().max(1)
    }

    pub fn page_estimate(&self) -> usize {
        (self.word_count() as f32 / 250.0).ceil().max(1.0) as usize
    }

    pub fn reading_time_minutes(&self) -> usize {
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
            let trimmed = line.trim();
            // Check for HTML heading tags
            for level in 1..=3 {
                let open = format!("<h{}>", level);
                let close = format!("</h{}>", level);
                if trimmed.starts_with(&open) && trimmed.ends_with(&close) {
                    let inner = &trimmed[open.len()..trimmed.len() - close.len()];
                    let clean = strip_tags(inner);
                    if !clean.is_empty() {
                        headings.push((level, clean, offset));
                    }
                }
            }
            offset += line.len() + 1;
        }
        headings
    }
}

/// Strip HTML tags from text, returning only the visible content
fn strip_tags(text: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for ch in text.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    result
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}
