//! Document model for slowWrite
//!
//! Plain text stored without formatting tags. Formatting is tracked as
//! separate span metadata. On save, spans are baked into HTML tags.

/// Inline formatting kind
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpanKind {
    Bold,
    Italic,
    Strikethrough,
    Underline,
}

/// A formatting span (byte range + kind)
#[derive(Debug, Clone)]
pub struct FormatSpan {
    pub start: usize,
    pub end: usize,
    pub kind: SpanKind,
}

/// A text document with formatting spans
#[derive(Debug, Clone)]
pub struct Document {
    pub text: String,
    pub spans: Vec<FormatSpan>,
    /// Which formatting kinds are currently "active" (typing will be formatted)
    pub active_formats: std::collections::HashSet<SpanKind>,
}

impl Document {
    pub fn new() -> Self {
        Self { text: String::new(), spans: Vec::new(), active_formats: Default::default() }
    }

    pub fn from_text(text: String) -> Self {
        Self { text, spans: Vec::new(), active_formats: Default::default() }
    }

    pub fn word_count(&self) -> usize { self.text.split_whitespace().count() }
    pub fn char_count(&self) -> usize { self.text.chars().count() }
    pub fn line_count(&self) -> usize { self.text.lines().count().max(1) }

    pub fn page_estimate(&self) -> usize {
        (self.word_count() as f32 / 250.0).ceil().max(1.0) as usize
    }

    pub fn reading_time_minutes(&self) -> usize {
        let wc = self.word_count();
        if wc == 0 { return 0; }
        ((wc as f32) / 200.0).ceil() as usize
    }

    pub fn headings(&self) -> Vec<(usize, String, usize)> {
        let mut headings = Vec::new();
        let mut offset = 0;
        for line in self.text.split('\n') {
            let trimmed = line.trim();
            for level in 1..=3 {
                let open = format!("<h{}>", level);
                let close = format!("</h{}>", level);
                if trimmed.starts_with(&open) && trimmed.ends_with(&close) {
                    let inner = &trimmed[open.len()..trimmed.len() - close.len()];
                    if !inner.is_empty() {
                        headings.push((level, inner.to_string(), offset));
                    }
                }
            }
            offset += line.len() + 1;
        }
        headings
    }

    /// Check if a formatting kind is active at a byte position
    pub fn has_format_at(&self, byte_pos: usize, kind: SpanKind) -> bool {
        // Check explicit active flags first (for zero-width preemptive state)
        if self.active_formats.contains(&kind) { return true; }
        // Check spans — cursor must be strictly inside (start <= pos < end)
        self.spans.iter().any(|s| s.kind == kind && s.start <= byte_pos && byte_pos < s.end)
    }

    /// Toggle a formatting kind on/off at the cursor.
    /// With no selection: toggles the active flag (affects future typing).
    /// With selection: applies/removes formatting on the range.
    pub fn toggle_format(&mut self, byte_start: usize, byte_end: usize, kind: SpanKind) {
        if byte_start == byte_end {
            // No selection — toggle the active flag
            if self.active_formats.contains(&kind) {
                self.active_formats.remove(&kind);
            } else if self.spans.iter().any(|s| s.kind == kind && s.start <= byte_start && byte_start < s.end) {
                // Inside an existing span — end it at cursor
                if let Some(idx) = self.spans.iter().position(|s| s.kind == kind && s.start <= byte_start && byte_start < s.end) {
                    self.spans[idx].end = byte_start;
                }
                self.active_formats.remove(&kind);
            } else {
                self.active_formats.insert(kind);
            }
        } else {
            // Has selection
            let fully_covered = self.spans.iter().any(|s| s.kind == kind && s.start <= byte_start && s.end >= byte_end);
            if fully_covered {
                self.remove_format_range(byte_start, byte_end, kind);
            } else {
                self.spans.push(FormatSpan { start: byte_start, end: byte_end, kind });
            }
        }
        self.cleanup_spans();
    }

    /// Called after text is inserted. Extends active formatting spans.
    pub fn after_insert(&mut self, byte_pos: usize, len: usize) {
        // Shift existing spans
        for span in &mut self.spans {
            if span.start > byte_pos {
                span.start += len;
                span.end += len;
            } else if span.end > byte_pos {
                // Insertion inside span: grow it
                span.end += len;
            } else if span.end == byte_pos {
                // At end of span: only grow if this kind is "active"
                if self.active_formats.contains(&span.kind) {
                    span.end += len;
                }
            }
        }
        // For active formats that don't have a span ending here, create one
        for kind in self.active_formats.clone() {
            let covered = self.spans.iter().any(|s| s.kind == kind && s.start <= byte_pos && s.end >= byte_pos + len);
            if !covered {
                self.spans.push(FormatSpan { start: byte_pos, end: byte_pos + len, kind });
            }
        }
        self.cleanup_spans();
    }

    /// Called after text is deleted.
    pub fn after_delete(&mut self, byte_pos: usize, len: usize) {
        let del_end = byte_pos + len;
        self.spans.retain_mut(|span| {
            if span.end <= byte_pos { return true; }
            if span.start >= del_end {
                span.start -= len;
                span.end -= len;
                return true;
            }
            if span.start < byte_pos {
                span.end = if span.end <= del_end { byte_pos } else { span.end - len };
            } else {
                span.start = byte_pos;
                span.end = if span.end <= del_end { byte_pos } else { byte_pos + (span.end - del_end) };
            }
            span.start < span.end
        });
    }

    /// Remove formatting from a byte range
    fn remove_format_range(&mut self, start: usize, end: usize, kind: SpanKind) {
        let mut new_spans = Vec::new();
        for span in &self.spans {
            if span.kind != kind { new_spans.push(span.clone()); continue; }
            if span.start < start {
                new_spans.push(FormatSpan { start: span.start, end: start.min(span.end), kind });
            }
            if span.end > end {
                new_spans.push(FormatSpan { start: end.max(span.start), end: span.end, kind });
            }
        }
        self.spans = new_spans;
    }

    /// Merge overlapping spans and remove empty ones
    fn cleanup_spans(&mut self) {
        self.spans.retain(|s| s.start < s.end);
        if self.spans.len() < 2 { return; }
        self.spans.sort_by(|a, b| (a.kind as u8).cmp(&(b.kind as u8)).then(a.start.cmp(&b.start)));
        let mut merged = Vec::new();
        for span in &self.spans {
            if let Some(last) = merged.last_mut() {
                let last: &mut FormatSpan = last;
                if last.kind == span.kind && span.start <= last.end {
                    last.end = last.end.max(span.end);
                    continue;
                }
            }
            merged.push(span.clone());
        }
        self.spans = merged;
    }

    /// Update active_formats based on cursor position (called when cursor moves)
    pub fn sync_active_formats(&mut self, byte_pos: usize) {
        self.active_formats.clear();
        for span in &self.spans {
            if span.start <= byte_pos && byte_pos < span.end {
                self.active_formats.insert(span.kind);
            }
        }
    }

    /// Bake formatting spans into HTML for saving.
    pub fn to_html(&self) -> String {
        let mut events: Vec<(usize, bool, SpanKind)> = Vec::new();
        for span in &self.spans {
            if span.start < span.end {
                events.push((span.start, true, span.kind));
                events.push((span.end, false, span.kind));
            }
        }
        events.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        let mut result = String::new();
        let mut pos = 0;
        for (byte_pos, is_open, kind) in &events {
            if *byte_pos > pos {
                let segment = &self.text[pos..*byte_pos];
                result.push_str(&segment.replace('\n', "<br>\n"));
            }
            let tag = match kind {
                SpanKind::Bold => "b", SpanKind::Italic => "i",
                SpanKind::Strikethrough => "s", SpanKind::Underline => "u",
            };
            if *is_open { result.push_str(&format!("<{}>", tag)); }
            else { result.push_str(&format!("</{}>", tag)); }
            pos = *byte_pos;
        }
        if pos < self.text.len() {
            result.push_str(&self.text[pos..].replace('\n', "<br>\n"));
        }
        if result.is_empty() && self.text.is_empty() {
            return String::from("<!DOCTYPE html>\n<html>\n<body>\n\n</body>\n</html>\n");
        }
        format!("<!DOCTYPE html>\n<html>\n<body>\n{}\n</body>\n</html>\n", result)
    }

    /// Parse HTML and extract plain text + formatting spans.
    pub fn from_html(html: &str) -> Self {
        let mut content = html.to_string();
        if let Some(body_start) = content.find("<body") {
            if let Some(body_end) = content[body_start..].find('>') {
                content = content[body_start + body_end + 1..].to_string();
            }
        }
        if let Some(pos) = content.find("</body>") {
            content = content[..pos].to_string();
        }
        content = content.replace("<br>\n", "\n").replace("<br>", "\n")
            .replace("<br/>", "\n").replace("<br />", "\n");
        let content = content.trim().to_string();

        let mut text = String::new();
        let mut spans = Vec::new();
        let mut stack: Vec<(SpanKind, usize)> = Vec::new();
        let bytes = content.as_bytes();
        let len = bytes.len();
        let mut pos = 0;

        while pos < len {
            if bytes[pos] == b'<' {
                if let Some(end) = content[pos..].find('>') {
                    let tag_str = &content[pos..pos + end + 1];
                    let is_closing = tag_str.starts_with("</");
                    let tag_name: String = tag_str.trim_start_matches('<').trim_start_matches('/')
                        .chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
                    let tag_name = tag_name.to_lowercase();

                    let kind = match tag_name.as_str() {
                        "b" | "strong" => Some(SpanKind::Bold),
                        "i" | "em" => Some(SpanKind::Italic),
                        "s" | "del" | "strike" => Some(SpanKind::Strikethrough),
                        "u" => Some(SpanKind::Underline),
                        _ => None,
                    };

                    if let Some(k) = kind {
                        if is_closing {
                            if let Some(sp) = stack.iter().rposition(|(sk, _)| *sk == k) {
                                let (_, start) = stack.remove(sp);
                                spans.push(FormatSpan { start, end: text.len(), kind: k });
                            }
                        } else {
                            stack.push((k, text.len()));
                        }
                        pos += end + 1;
                        continue;
                    }
                    text.push_str(tag_str);
                    pos += end + 1;
                    continue;
                }
            }
            text.push(bytes[pos] as char);
            pos += 1;
        }

        let mut doc = Self { text, spans, active_formats: Default::default() };
        doc.cleanup_spans();
        doc
    }
}

impl Default for Document {
    fn default() -> Self { Self::new() }
}
