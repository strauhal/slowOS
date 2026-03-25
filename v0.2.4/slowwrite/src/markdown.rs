//! Markdown-aware layouter for egui TextEdit.
//!
//! Renders markdown syntax with visual formatting while keeping the document
//! as plain text. Headings appear larger/bold, **bold** renders bold, etc.
//! Syntax markers (e.g. `#`, `**`) are rendered in gray.

use egui::text::{LayoutJob, LayoutSection};
use egui::{Color32, FontFamily, FontId, TextFormat};

/// Gray color for syntax markers (hash, asterisks, backticks, etc.)
const MARKER_GRAY: Color32 = Color32::from_gray(160);

/// Highlight color for search matches
const SEARCH_BG: Color32 = Color32::from_rgb(200, 200, 200);
/// Highlight color for the current search match
const CURRENT_SEARCH_BG: Color32 = Color32::from_rgb(120, 120, 120);

/// Layout a markdown document for egui's TextEdit layouter callback.
///
/// * `font_size` — base body font size (typically Scale::BODY + 2.0 * zoom)
/// * `search_ranges` — byte ranges of search matches to highlight
/// * `current_search` — index into search_ranges for the "current" match
pub fn layout_markdown(
    ui: &egui::Ui,
    text: &str,
    wrap_width: f32,
    font_size: f32,
    search_ranges: &[(usize, usize)],
    current_search: Option<usize>,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    job.text = text.to_string();

    // Process line by line
    let mut byte_offset = 0;
    for line in text.split('\n') {
        let line_start = byte_offset;
        let line_end = line_start + line.len();

        layout_line(&mut job, line, line_start, line_end, font_size, search_ranges, current_search);

        byte_offset = line_end + 1; // +1 for the \n
        // Add the newline character if not at end of text
        if byte_offset <= text.len() {
            job.sections.push(LayoutSection {
                leading_space: 0.0,
                byte_range: (line_end)..(line_end + 1),
                format: TextFormat {
                    font_id: FontId::proportional(font_size),
                    ..Default::default()
                },
            });
        }
    }

    // If text is empty, add a default section so the layouter doesn't panic
    if job.sections.is_empty() {
        job.sections.push(LayoutSection {
            leading_space: 0.0,
            byte_range: 0..0,
            format: TextFormat {
                font_id: FontId::proportional(font_size),
                ..Default::default()
            },
        });
    }

    // Apply search highlighting as background color overlays
    let _ = ui; // ui available for future use
    apply_search_highlights(&mut job, search_ranges, current_search);

    job
}

/// Detect the line type and lay out sections accordingly.
fn layout_line(
    job: &mut LayoutJob,
    line: &str,
    line_start: usize,
    line_end: usize,
    font_size: f32,
    _search_ranges: &[(usize, usize)],
    _current_search: Option<usize>,
) {
    let trimmed = line.trim_start();
    let leading_spaces = line.len() - trimmed.len();

    // Heading detection
    if trimmed.starts_with("### ") && trimmed.len() > 4 {
        let marker_end = line_start + leading_spaces + 4;
        // Gray marker
        push_section(job, line_start..marker_end, FontId::new(font_size * 1.1, FontFamily::Name("Bold".into())), MARKER_GRAY, None);
        // Bold heading text
        push_section(job, marker_end..line_end, FontId::new(font_size * 1.1, FontFamily::Name("Bold".into())), Color32::BLACK, None);
        return;
    }
    if trimmed.starts_with("## ") && trimmed.len() > 3 {
        let marker_end = line_start + leading_spaces + 3;
        push_section(job, line_start..marker_end, FontId::new(font_size * 1.3, FontFamily::Name("Bold".into())), MARKER_GRAY, None);
        push_section(job, marker_end..line_end, FontId::new(font_size * 1.3, FontFamily::Name("Bold".into())), Color32::BLACK, None);
        return;
    }
    if trimmed.starts_with("# ") && trimmed.len() > 2 {
        let marker_end = line_start + leading_spaces + 2;
        push_section(job, line_start..marker_end, FontId::new(font_size * 1.5, FontFamily::Name("Bold".into())), MARKER_GRAY, None);
        push_section(job, marker_end..line_end, FontId::new(font_size * 1.5, FontFamily::Name("Bold".into())), Color32::BLACK, None);
        return;
    }

    // Blockquote
    if trimmed.starts_with("> ") {
        let marker_end = line_start + leading_spaces + 2;
        push_section(job, line_start..marker_end, FontId::new(font_size, FontFamily::Name("Italic".into())), MARKER_GRAY, None);
        layout_inline(job, line, marker_end, line_end, font_size, FontFamily::Name("Italic".into()));
        return;
    }

    // Unordered list (- or *)
    if (trimmed.starts_with("- ") || trimmed.starts_with("* ")) && trimmed.len() > 2 {
        let marker_end = line_start + leading_spaces + 2;
        push_section(job, line_start..marker_end, FontId::proportional(font_size), MARKER_GRAY, None);
        layout_inline(job, line, marker_end, line_end, font_size, FontFamily::Proportional);
        return;
    }

    // Ordered list (1. 2. etc.)
    if let Some(dot_pos) = trimmed.find(". ") {
        let num_part = &trimmed[..dot_pos];
        if !num_part.is_empty() && num_part.chars().all(|c| c.is_ascii_digit()) {
            let marker_end = line_start + leading_spaces + dot_pos + 2;
            push_section(job, line_start..marker_end, FontId::proportional(font_size), MARKER_GRAY, None);
            layout_inline(job, line, marker_end, line_end, font_size, FontFamily::Proportional);
            return;
        }
    }

    // Horizontal rule (---, ***, ___)
    if (trimmed == "---" || trimmed == "***" || trimmed == "___") && trimmed.len() >= 3 {
        push_section(job, line_start..line_end, FontId::proportional(font_size), MARKER_GRAY, None);
        return;
    }

    // Regular line — process inline formatting
    layout_inline(job, line, line_start, line_end, font_size, FontFamily::Proportional);
}

/// Process inline markdown formatting: **bold**, *italic*, ***bold+italic***,
/// ~~strikethrough~~, `code`
fn layout_inline(
    job: &mut LayoutJob,
    _line: &str,
    start: usize,
    end: usize,
    font_size: f32,
    base_family: FontFamily,
) {
    // Copy bytes to avoid borrow conflict with job.sections
    let bytes_vec: Vec<u8> = job.text[start..end].as_bytes().to_vec();
    let bytes: &[u8] = &bytes_vec;
    let len = bytes.len();
    let mut pos = 0;
    let mut section_start = 0;

    while pos < len {
        // Inline code: `...`
        if bytes[pos] == b'`' {
            // Flush preceding text
            if section_start < pos {
                push_section(job, (start + section_start)..(start + pos),
                    FontId::new(font_size, base_family.clone()), Color32::BLACK, None);
            }
            // Find closing backtick
            if let Some(close) = find_byte(bytes, b'`', pos + 1) {
                // Opening marker
                push_section(job, (start + pos)..(start + pos + 1),
                    FontId::new(font_size, FontFamily::Monospace), MARKER_GRAY, None);
                // Code content
                push_section(job, (start + pos + 1)..(start + close),
                    FontId::new(font_size, FontFamily::Monospace), Color32::BLACK, None);
                // Closing marker
                push_section(job, (start + close)..(start + close + 1),
                    FontId::new(font_size, FontFamily::Monospace), MARKER_GRAY, None);
                pos = close + 1;
                section_start = pos;
                continue;
            }
        }

        // Strikethrough: ~~...~~
        if pos + 1 < len && bytes[pos] == b'~' && bytes[pos + 1] == b'~' {
            if section_start < pos {
                push_section(job, (start + section_start)..(start + pos),
                    FontId::new(font_size, base_family.clone()), Color32::BLACK, None);
            }
            if let Some(close) = find_double_byte(bytes, b'~', pos + 2) {
                // Opening ~~
                push_section(job, (start + pos)..(start + pos + 2),
                    FontId::new(font_size, base_family.clone()), MARKER_GRAY, None);
                // Struck text
                push_section_strikethrough(job, (start + pos + 2)..(start + close),
                    FontId::new(font_size, base_family.clone()), Color32::BLACK);
                // Closing ~~
                push_section(job, (start + close)..(start + close + 2),
                    FontId::new(font_size, base_family.clone()), MARKER_GRAY, None);
                pos = close + 2;
                section_start = pos;
                continue;
            }
        }

        // Bold+Italic: ***...***
        if pos + 2 < len && bytes[pos] == b'*' && bytes[pos + 1] == b'*' && bytes[pos + 2] == b'*' {
            if section_start < pos {
                push_section(job, (start + section_start)..(start + pos),
                    FontId::new(font_size, base_family.clone()), Color32::BLACK, None);
            }
            if let Some(close) = find_triple_byte(bytes, b'*', pos + 3) {
                push_section(job, (start + pos)..(start + pos + 3),
                    FontId::new(font_size, FontFamily::Name("BoldItalic".into())), MARKER_GRAY, None);
                push_section(job, (start + pos + 3)..(start + close),
                    FontId::new(font_size, FontFamily::Name("BoldItalic".into())), Color32::BLACK, None);
                push_section(job, (start + close)..(start + close + 3),
                    FontId::new(font_size, FontFamily::Name("BoldItalic".into())), MARKER_GRAY, None);
                pos = close + 3;
                section_start = pos;
                continue;
            }
        }

        // Bold: **...**
        if pos + 1 < len && bytes[pos] == b'*' && bytes[pos + 1] == b'*' {
            // Make sure it's not ***
            if pos + 2 < len && bytes[pos + 2] == b'*' {
                pos += 1;
                continue;
            }
            if section_start < pos {
                push_section(job, (start + section_start)..(start + pos),
                    FontId::new(font_size, base_family.clone()), Color32::BLACK, None);
            }
            if let Some(close) = find_double_byte(bytes, b'*', pos + 2) {
                push_section(job, (start + pos)..(start + pos + 2),
                    FontId::new(font_size, FontFamily::Name("Bold".into())), MARKER_GRAY, None);
                push_section(job, (start + pos + 2)..(start + close),
                    FontId::new(font_size, FontFamily::Name("Bold".into())), Color32::BLACK, None);
                push_section(job, (start + close)..(start + close + 2),
                    FontId::new(font_size, FontFamily::Name("Bold".into())), MARKER_GRAY, None);
                pos = close + 2;
                section_start = pos;
                continue;
            }
        }

        // Italic: *...*
        if bytes[pos] == b'*' {
            // Make sure it's not ** or ***
            if pos + 1 < len && bytes[pos + 1] == b'*' {
                pos += 1;
                continue;
            }
            if section_start < pos {
                push_section(job, (start + section_start)..(start + pos),
                    FontId::new(font_size, base_family.clone()), Color32::BLACK, None);
            }
            if let Some(close) = find_single_asterisk(bytes, pos + 1) {
                push_section(job, (start + pos)..(start + pos + 1),
                    FontId::new(font_size, FontFamily::Name("Italic".into())), MARKER_GRAY, None);
                push_section(job, (start + pos + 1)..(start + close),
                    FontId::new(font_size, FontFamily::Name("Italic".into())), Color32::BLACK, None);
                push_section(job, (start + close)..(start + close + 1),
                    FontId::new(font_size, FontFamily::Name("Italic".into())), MARKER_GRAY, None);
                pos = close + 1;
                section_start = pos;
                continue;
            }
        }

        pos += 1;
    }

    // Remaining text
    if section_start < len {
        push_section(job, (start + section_start)..(start + len),
            FontId::new(font_size, base_family), Color32::BLACK, None);
    }
}

fn push_section(
    job: &mut LayoutJob,
    byte_range: std::ops::Range<usize>,
    font_id: FontId,
    color: Color32,
    background: Option<Color32>,
) {
    if byte_range.is_empty() {
        return;
    }
    let mut format = TextFormat {
        font_id,
        color,
        ..Default::default()
    };
    if let Some(bg) = background {
        format.background = bg;
    }
    job.sections.push(LayoutSection {
        leading_space: 0.0,
        byte_range,
        format,
    });
}

fn push_section_strikethrough(
    job: &mut LayoutJob,
    byte_range: std::ops::Range<usize>,
    font_id: FontId,
    color: Color32,
) {
    if byte_range.is_empty() {
        return;
    }
    job.sections.push(LayoutSection {
        leading_space: 0.0,
        byte_range,
        format: TextFormat {
            font_id,
            color,
            strikethrough: egui::Stroke::new(1.0, color),
            ..Default::default()
        },
    });
}

/// Find a single byte starting from `from`.
fn find_byte(bytes: &[u8], needle: u8, from: usize) -> Option<usize> {
    for i in from..bytes.len() {
        if bytes[i] == needle {
            return Some(i);
        }
    }
    None
}

/// Find a double byte (e.g. **) starting from `from`.
fn find_double_byte(bytes: &[u8], needle: u8, from: usize) -> Option<usize> {
    for i in from..bytes.len().saturating_sub(1) {
        if bytes[i] == needle && bytes[i + 1] == needle {
            // Make sure it's not a triple
            if i + 2 < bytes.len() && bytes[i + 2] == needle {
                continue;
            }
            // Also check it's not preceded by the same byte (would be triple from other side)
            if i > 0 && bytes[i - 1] == needle {
                continue;
            }
            return Some(i);
        }
    }
    None
}

/// Find a triple byte (e.g. ***) starting from `from`.
fn find_triple_byte(bytes: &[u8], needle: u8, from: usize) -> Option<usize> {
    for i in from..bytes.len().saturating_sub(2) {
        if bytes[i] == needle && bytes[i + 1] == needle && bytes[i + 2] == needle {
            return Some(i);
        }
    }
    None
}

/// Find a single asterisk that is NOT part of ** or ***.
fn find_single_asterisk(bytes: &[u8], from: usize) -> Option<usize> {
    for i in from..bytes.len() {
        if bytes[i] == b'*' {
            // Check it's not followed by another *
            if i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                continue;
            }
            // Check it's not preceded by another *
            if i > 0 && bytes[i - 1] == b'*' {
                continue;
            }
            return Some(i);
        }
    }
    None
}

/// Apply search highlight backgrounds to existing sections by splitting them
/// at match boundaries.
fn apply_search_highlights(
    job: &mut LayoutJob,
    search_ranges: &[(usize, usize)],
    current_search: Option<usize>,
) {
    if search_ranges.is_empty() {
        return;
    }

    let mut new_sections = Vec::new();

    for section in &job.sections {
        let s_start = section.byte_range.start;
        let s_end = section.byte_range.end;

        // Collect overlapping search ranges for this section
        let mut splits: Vec<(usize, usize, bool)> = Vec::new(); // (start, end, is_current)
        for (idx, &(ms, me)) in search_ranges.iter().enumerate() {
            let overlap_start = ms.max(s_start);
            let overlap_end = me.min(s_end);
            if overlap_start < overlap_end {
                let is_current = current_search == Some(idx);
                splits.push((overlap_start, overlap_end, is_current));
            }
        }

        if splits.is_empty() {
            new_sections.push(section.clone());
            continue;
        }

        splits.sort_by_key(|s| s.0);

        let mut cursor = s_start;
        for (ms, me, is_current) in splits {
            // Before the match
            if cursor < ms {
                let mut fmt = section.format.clone();
                fmt.background = Color32::TRANSPARENT;
                new_sections.push(LayoutSection {
                    leading_space: if cursor == s_start { section.leading_space } else { 0.0 },
                    byte_range: cursor..ms,
                    format: fmt,
                });
            }
            // The match itself
            let mut fmt = section.format.clone();
            fmt.background = if is_current { CURRENT_SEARCH_BG } else { SEARCH_BG };
            new_sections.push(LayoutSection {
                leading_space: if cursor == s_start && cursor == ms { section.leading_space } else { 0.0 },
                byte_range: ms..me,
                format: fmt,
            });
            cursor = me;
        }
        // After last match
        if cursor < s_end {
            let mut fmt = section.format.clone();
            fmt.background = Color32::TRANSPARENT;
            new_sections.push(LayoutSection {
                leading_space: 0.0,
                byte_range: cursor..s_end,
                format: fmt,
            });
        }
    }

    job.sections = new_sections;
}
