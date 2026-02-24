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

/// Serialize a RichDocument to our simple JSON format
pub fn save_rich_document(doc: &RichDocument) -> String {
    serde_json::to_string_pretty(doc).unwrap_or_default()
}

/// Load a RichDocument from JSON
pub fn load_rich_document(json: &str) -> Option<RichDocument> {
    serde_json::from_str(json).ok()
}

/// Export a RichDocument as RTF
pub fn save_as_rtf(doc: &RichDocument) -> String {
    let mut rtf = String::from("{\\rtf1\\ansi\\deff0\n");
    rtf.push_str("{\\fonttbl{\\f0 IBM Plex Sans;}{\\f1 JetBrains Mono;}}\n");
    rtf.push('\n');

    let default = CharStyle::default();
    let mut prev_style = &default;

    for (i, c) in doc.text.chars().enumerate() {
        let style = doc.styles.get(i).unwrap_or(&default);

        // Emit style changes
        if i == 0 || style != prev_style {
            // Close previous group if not first char
            if i > 0 { rtf.push('}'); }

            rtf.push('{');
            // Font family
            match style.font_family {
                FontFamily::Proportional => rtf.push_str("\\f0"),
                FontFamily::Monospace => rtf.push_str("\\f1"),
            }
            // Font size (RTF uses half-points)
            rtf.push_str(&format!("\\fs{}", (style.font_size * 2.0) as u32));
            if style.bold { rtf.push_str("\\b"); }
            if style.italic { rtf.push_str("\\i"); }
            if style.underline { rtf.push_str("\\ul"); }
            if style.strikethrough { rtf.push_str("\\strike"); }
            rtf.push(' ');
        }

        // Emit character
        match c {
            '\n' => rtf.push_str("\\par\n"),
            '\\' => rtf.push_str("\\\\"),
            '{' => rtf.push_str("\\{"),
            '}' => rtf.push_str("\\}"),
            _ if (c as u32) > 127 => rtf.push_str(&format!("\\u{}?", c as i32)),
            _ => rtf.push(c),
        }

        prev_style = style;
    }

    if !doc.text.is_empty() { rtf.push('}'); }
    rtf.push_str("\n}");
    rtf
}

/// Load an RTF file, extracting styled text.
/// Supports basic RTF: \b, \i, \ul, \strike, \fsN, \f0/\f1, \par
pub fn load_rtf(rtf: &str) -> Option<RichDocument> {
    let mut text = String::new();
    let mut styles: Vec<CharStyle> = Vec::new();
    let mut current_style = CharStyle::default();
    let mut style_stack: Vec<CharStyle> = Vec::new();
    let mut chars = rtf.chars().peekable();

    // Skip header - find first content after fonttbl
    let rtf_trimmed = rtf.trim();
    if !rtf_trimmed.starts_with("{\\rtf") {
        return None;
    }

    // Simple RTF parser: skip groups we don't understand, parse basic commands
    let mut depth = 0i32;
    let mut in_fonttbl = false;
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                depth += 1;
                // Push current style so it can be restored when group closes
                style_stack.push(current_style.clone());
                if depth == 2 {
                    // Check if this is fonttbl
                    let rest: String = chars.clone().take(8).collect();
                    if rest.starts_with("\\fonttbl") {
                        in_fonttbl = true;
                    }
                }
            }
            '}' => {
                if in_fonttbl && depth == 2 { in_fonttbl = false; }
                // Pop style to restore parent group's formatting
                if let Some(prev) = style_stack.pop() {
                    current_style = prev;
                }
                depth -= 1;
                if depth <= 0 { break; }
            }
            '\\' if !in_fonttbl => {
                // Parse command
                let mut cmd = String::new();
                while let Some(&nc) = chars.peek() {
                    if nc.is_ascii_alphabetic() {
                        cmd.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                // Parse optional numeric parameter
                let mut num_str = String::new();
                let mut has_neg = false;
                if let Some(&nc) = chars.peek() {
                    if nc == '-' { has_neg = true; chars.next(); }
                }
                while let Some(&nc) = chars.peek() {
                    if nc.is_ascii_digit() {
                        num_str.push(nc);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if has_neg && !num_str.is_empty() { num_str.insert(0, '-'); }
                let num: Option<i32> = num_str.parse().ok();

                // Consume trailing space
                if let Some(&' ') = chars.peek() { chars.next(); }

                match cmd.as_str() {
                    "par" => {
                        text.push('\n');
                        styles.push(current_style.clone());
                    }
                    "b" => current_style.bold = num.unwrap_or(1) != 0,
                    "i" => current_style.italic = num.unwrap_or(1) != 0,
                    "ul" => current_style.underline = true,
                    "ulnone" => current_style.underline = false,
                    "strike" => current_style.strikethrough = num.unwrap_or(1) != 0,
                    "fs" => if let Some(n) = num { current_style.font_size = n as f32 / 2.0; },
                    "f0" => current_style.font_family = FontFamily::Proportional,
                    "f1" => current_style.font_family = FontFamily::Monospace,
                    "u" => {
                        // Unicode: \uN? — N is the char code, ? is fallback
                        if let Some(n) = num {
                            if let Some(ch) = char::from_u32(n as u32) {
                                text.push(ch);
                                styles.push(current_style.clone());
                            }
                        }
                        // Skip fallback character
                        if let Some(&nc) = chars.peek() {
                            if nc != '\\' && nc != '{' && nc != '}' { chars.next(); }
                        }
                    }
                    "" => {
                        // Escaped character: next char after backslash
                        // Already consumed, but we need the char
                    }
                    _ => {} // Unknown command, skip
                }

                // Handle escaped chars: \\ \{ \}
                if cmd.is_empty() {
                    if let Some(&nc) = chars.peek() {
                        match nc {
                            '\\' | '{' | '}' => {
                                text.push(nc);
                                styles.push(current_style.clone());
                                chars.next();
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ if !in_fonttbl && depth >= 1 => {
                if c != '\r' && c != '\n' {
                    text.push(c);
                    styles.push(current_style.clone());
                }
            }
            _ => {}
        }
    }

    Some(RichDocument { text, styles, cursor_style: CharStyle::default() })
}
