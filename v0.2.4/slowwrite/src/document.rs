//! Document model for slowWrite
//!
//! Plain text stored without formatting tags. Formatting is tracked as
//! separate span metadata. On save, spans are baked into HTML tags.

/// Inline formatting kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKind {
    Bold,
    Italic,
    Strikethrough,
    Underline,
}

/// A formatting span (byte range + kind)
#[derive(Debug, Clone)]
pub struct FormatSpan {
    pub start: usize, // byte offset
    pub end: usize,   // byte offset
    pub kind: SpanKind,
}

/// A text document with formatting spans
#[derive(Debug, Clone)]
pub struct Document {
    pub text: String,
    pub spans: Vec<FormatSpan>,
}

impl Document {
    pub fn new() -> Self {
        Self { text: String::new(), spans: Vec::new() }
    }

    pub fn from_text(text: String) -> Self {
        Self { text, spans: Vec::new() }
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
        (self.word_count() as f32 / 250.0).ceil().max(1.0) as usize
    }

    pub fn reading_time_minutes(&self) -> usize {
        let wc = self.word_count();
        if wc == 0 { return 0; }
        ((wc as f32) / 200.0).ceil() as usize
    }

    /// Extract headings for the outline panel.
    /// Headings are lines wrapped in <h1>...<h3> tags (block-level, kept in text).
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

    /// Check if a byte position is inside a span of the given kind
    pub fn has_format_at(&self, byte_pos: usize, kind: SpanKind) -> bool {
        self.spans.iter().any(|s| s.kind == kind && s.start <= byte_pos && byte_pos <= s.end)
    }

    /// Toggle a format span. If cursor has no selection and is inside a span
    /// of this kind, end that span at cursor. Otherwise start a new span.
    pub fn toggle_format(&mut self, byte_start: usize, byte_end: usize, kind: SpanKind) {
        if byte_start == byte_end {
            // No selection — toggle the active formatting flag
            // If inside an existing span, split/end it
            if let Some(idx) = self.spans.iter().position(|s| s.kind == kind && s.start <= byte_start && byte_start <= s.end) {
                let span = self.spans[idx].clone();
                if span.start == span.end {
                    // Empty span — just remove it
                    self.spans.remove(idx);
                } else if byte_start == span.end {
                    // At the end of span — do nothing (cursor exits naturally)
                    // Actually, trim the span to not extend further
                } else {
                    // Split: keep the part before cursor, discard rest
                    self.spans[idx].end = byte_start;
                    if byte_start < span.end {
                        // Create a second span for the part after cursor
                        // (user might want to keep formatting after a gap)
                    }
                }
            } else {
                // Not in a span — start a new zero-width span at cursor
                // It will grow as the user types (see adjust_spans_for_insert)
                self.spans.push(FormatSpan { start: byte_start, end: byte_start, kind });
            }
        } else {
            // Has selection — toggle formatting on the range
            // Check if fully covered
            let fully_covered = self.spans.iter().any(|s| s.kind == kind && s.start <= byte_start && s.end >= byte_end);
            if fully_covered {
                // Remove formatting from this range
                self.remove_format_range(byte_start, byte_end, kind);
            } else {
                // Add formatting to this range
                self.add_format_range(byte_start, byte_end, kind);
            }
        }
        self.merge_spans();
    }

    /// Add a format span, merging with existing overlapping spans
    fn add_format_range(&mut self, start: usize, end: usize, kind: SpanKind) {
        self.spans.push(FormatSpan { start, end, kind });
        self.merge_spans();
    }

    /// Remove formatting from a byte range
    fn remove_format_range(&mut self, start: usize, end: usize, kind: SpanKind) {
        let mut new_spans = Vec::new();
        for span in &self.spans {
            if span.kind != kind {
                new_spans.push(span.clone());
                continue;
            }
            // Before the removed range
            if span.start < start {
                new_spans.push(FormatSpan { start: span.start, end: start.min(span.end), kind });
            }
            // After the removed range
            if span.end > end {
                new_spans.push(FormatSpan { start: end.max(span.start), end: span.end, kind });
            }
        }
        self.spans = new_spans;
    }

    /// Merge overlapping/adjacent spans of the same kind
    fn merge_spans(&mut self) {
        if self.spans.len() < 2 { return; }
        self.spans.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.start.cmp(&b.start)));
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

    /// Adjust spans after text insertion at `byte_pos` of `len` bytes.
    /// Zero-width spans at the insertion point are expanded (so typing
    /// inside an activated formatting grows the span).
    pub fn adjust_spans_for_insert(&mut self, byte_pos: usize, len: usize) {
        for span in &mut self.spans {
            if span.start == span.end && span.start == byte_pos {
                // Zero-width span at insertion point: expand it
                span.end += len;
            } else if span.start == byte_pos && span.end > span.start {
                // Span starts exactly at insertion: shift end
                span.end += len;
            } else {
                if span.start > byte_pos {
                    span.start += len;
                }
                if span.end >= byte_pos {
                    span.end += len;
                }
            }
        }
    }

    /// Adjust spans after text deletion from `byte_pos` of `len` bytes.
    pub fn adjust_spans_for_delete(&mut self, byte_pos: usize, len: usize) {
        let del_end = byte_pos + len;
        self.spans.retain_mut(|span| {
            if span.end <= byte_pos {
                // Before deletion — no change
                return true;
            }
            if span.start >= del_end {
                // After deletion — shift back
                span.start -= len;
                span.end -= len;
                return true;
            }
            // Overlapping — adjust
            if span.start < byte_pos {
                span.end = if span.end <= del_end { byte_pos } else { span.end - len };
            } else {
                span.start = byte_pos;
                span.end = if span.end <= del_end { byte_pos } else { byte_pos + (span.end - del_end) };
            }
            span.start < span.end // remove empty spans
        });
    }

    /// Bake formatting spans into HTML for saving.
    pub fn to_html(&self) -> String {
        if self.spans.is_empty() {
            // No formatting — just convert newlines to <br>
            let body = self.text.lines().collect::<Vec<_>>().join("<br>\n");
            return format!("<!DOCTYPE html>\n<html>\n<body>\n{}\n</body>\n</html>\n", body);
        }

        // Build a list of formatting events (open/close) sorted by position
        let mut events: Vec<(usize, bool, SpanKind)> = Vec::new(); // (byte_pos, is_open, kind)
        for span in &self.spans {
            if span.start < span.end {
                events.push((span.start, true, span.kind));
                events.push((span.end, false, span.kind));
            }
        }
        // Sort: closes before opens at same position, then by position
        events.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        let mut result = String::new();
        let mut pos = 0;
        for (byte_pos, is_open, kind) in &events {
            if *byte_pos > pos {
                // Emit text between events, converting newlines to <br>
                let segment = &self.text[pos..*byte_pos];
                result.push_str(&segment.replace('\n', "<br>\n"));
            }
            let tag = match kind {
                SpanKind::Bold => "b",
                SpanKind::Italic => "i",
                SpanKind::Strikethrough => "s",
                SpanKind::Underline => "u",
            };
            if *is_open {
                result.push_str(&format!("<{}>", tag));
            } else {
                result.push_str(&format!("</{}>", tag));
            }
            pos = *byte_pos;
        }
        // Remaining text
        if pos < self.text.len() {
            result.push_str(&self.text[pos..].replace('\n', "<br>\n"));
        }

        format!("<!DOCTYPE html>\n<html>\n<body>\n{}\n</body>\n</html>\n", result)
    }

    /// Parse HTML and extract plain text + formatting spans.
    pub fn from_html(html: &str) -> Self {
        let mut content = html.to_string();
        // Strip wrapper
        if let Some(body_start) = content.find("<body") {
            if let Some(body_end) = content[body_start..].find('>') {
                content = content[body_start + body_end + 1..].to_string();
            }
        }
        if let Some(pos) = content.find("</body>") {
            content = content[..pos].to_string();
        }
        // Convert <br> to newlines
        content = content.replace("<br>\n", "\n");
        content = content.replace("<br>", "\n");
        content = content.replace("<br/>", "\n");
        content = content.replace("<br />", "\n");
        let content = content.trim().to_string();

        let mut text = String::new();
        let mut spans = Vec::new();
        let mut stack: Vec<(SpanKind, usize)> = Vec::new(); // (kind, byte_start_in_output)

        let bytes = content.as_bytes();
        let len = bytes.len();
        let mut pos = 0;

        while pos < len {
            if bytes[pos] == b'<' {
                // Find closing >
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
                            if let Some(stack_pos) = stack.iter().rposition(|(sk, _)| *sk == k) {
                                let (_, start) = stack.remove(stack_pos);
                                spans.push(FormatSpan { start, end: text.len(), kind: k });
                            }
                        } else {
                            stack.push((k, text.len()));
                        }
                        pos += end + 1;
                        continue;
                    }
                    // Non-formatting tag: keep as text (e.g. <h1>, <p>)
                    text.push_str(tag_str);
                    pos += end + 1;
                    continue;
                }
            }
            text.push(bytes[pos] as char);
            pos += 1;
        }

        let mut doc = Self { text, spans };
        doc.merge_spans();
        doc
    }
}

impl SpanKind {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}
