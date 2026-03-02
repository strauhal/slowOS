//! Document model for slowWrite — plain text

/// A text document
#[derive(Debug, Clone, Default)]
pub struct RichDocument {
    /// The actual text content
    pub text: String,
}

impl RichDocument {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create from plain text
    pub fn from_plain_text(text: String) -> Self {
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
}
