//! Document model for slowWrite
//!
//! Plain text with optional markdown syntax for structure.
//! Headings, bold, italic, lists — stored as text, rendered with formatting.

use std::path::PathBuf;

/// A text document with metadata
#[derive(Debug, Clone)]
pub struct Document {
    pub text: String,
    pub path: Option<PathBuf>,
    pub title: String,
    pub modified: bool,
}

impl Document {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            path: None,
            title: "untitled".to_string(),
            modified: false,
        }
    }

    pub fn from_text(text: String) -> Self {
        Self {
            text,
            path: None,
            title: "untitled".to_string(),
            modified: false,
        }
    }

    /// Alias for compatibility
    pub fn from_plain_text(text: String) -> Self {
        Self::from_text(text)
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

    pub fn display_title(&self) -> String {
        if self.modified {
            format!("{}*", self.title)
        } else {
            self.title.clone()
        }
    }

    /// Strip markdown syntax from text for plain text export
    pub fn to_plain_text(&self) -> String {
        let mut result = String::new();
        for line in self.text.lines() {
            let stripped = if line.starts_with("### ") {
                &line[4..]
            } else if line.starts_with("## ") {
                &line[3..]
            } else if line.starts_with("# ") {
                &line[2..]
            } else if line.starts_with("> ") {
                &line[2..]
            } else {
                line
            };
            // Strip inline formatting markers
            let stripped = stripped
                .replace("**", "")
                .replace("~~", "");
            // Handle single * for italic (but not **)
            let stripped = strip_single_markers(&stripped, '*');
            let stripped = strip_single_markers(&stripped, '_');
            result.push_str(&stripped);
            result.push('\n');
        }
        // Remove trailing newline
        if result.ends_with('\n') {
            result.pop();
        }
        result
    }
}

/// Strip paired single-character markers (e.g., *italic*)
/// but not doubled markers (those are already stripped)
fn strip_single_markers(text: &str, marker: char) -> String {
    let mut result = String::new();
    let mut inside = false;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == marker {
            // Check it's not a double marker (already stripped)
            if i + 1 < chars.len() && chars[i + 1] == marker {
                // Double marker — pass through (shouldn't happen since we already stripped **)
                result.push(chars[i]);
                result.push(chars[i + 1]);
                i += 2;
            } else {
                // Single marker — toggle and skip
                inside = !inside;
                i += 1;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}
