//! HTML-aware layouter for egui TextEdit.
//!
//! Renders plain text with formatting spans overlaid. Block-level tags
//! (h1-h3, p) are still in the text buffer and get hidden in WYSIWYG mode.

use crate::document::{FormatSpan, SpanKind};
use egui::text::{LayoutJob, LayoutSection};
use egui::{Color32, FontFamily, FontId, TextFormat};

/// Layout text with formatting spans for egui's TextEdit.
pub fn layout_with_spans(
    text: &str,
    wrap_width: f32,
    font_size: f32,
    spans: &[FormatSpan],
    hide_block_tags: bool,
    search_ranges: &[(usize, usize)],
    current_search: Option<usize>,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    job.text = text.to_string();

    let len = text.len();
    if len == 0 {
        job.sections.push(LayoutSection {
            leading_space: 0.0,
            byte_range: 0..0,
            format: TextFormat { font_id: FontId::proportional(font_size), ..Default::default() },
        });
        return job;
    }

    // Build a set of "formatting change" events at byte boundaries
    // For each byte position, determine: bold, italic, strikethrough, underline, heading_level
    let bytes = text.as_bytes();
    let mut pos = 0;

    while pos < len {
        // Check for block-level tags in the text
        let mut block_tag_end = None;
        let mut heading_size = None;
        if bytes[pos] == b'<' && hide_block_tags {
            if let Some(tag_end_offset) = text[pos..].find('>') {
                let tag_str = &text[pos..pos + tag_end_offset + 1];
                let is_closing = tag_str.starts_with("</");
                let tag_name: String = tag_str.trim_start_matches('<').trim_start_matches('/')
                    .chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
                match tag_name.as_str() {
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" => {
                        // Hide the tag
                        let tag_byte_end = pos + tag_end_offset + 1;
                        let tag_font_size = 0.1;
                        job.sections.push(LayoutSection {
                            leading_space: 0.0,
                            byte_range: pos..tag_byte_end,
                            format: TextFormat {
                                font_id: FontId::new(tag_font_size, FontFamily::Proportional),
                                color: Color32::TRANSPARENT,
                                ..Default::default()
                            },
                        });
                        if !is_closing {
                            heading_size = match tag_name.as_str() {
                                "h1" => Some(font_size * 1.5),
                                "h2" => Some(font_size * 1.3),
                                "h3" => Some(font_size * 1.1),
                                _ => None,
                            };
                        }
                        block_tag_end = Some(tag_byte_end);
                    }
                    _ => {}
                }
            }
        }

        let section_start = if let Some(bte) = block_tag_end { bte } else { pos };
        if section_start >= len { break; }

        // Find the next block tag or end of text
        let section_end = if hide_block_tags {
            text[section_start..].find('<')
                .map(|i| section_start + i)
                .unwrap_or(len)
        } else {
            len
        };

        if section_start >= section_end {
            if block_tag_end.is_some() {
                pos = section_start;
                continue;
            }
            break;
        }

        // Now emit sections for section_start..section_end, splitting at span boundaries
        emit_formatted_sections(&mut job, section_start, section_end, font_size, heading_size, spans);

        pos = section_end;
    }

    // Handle case where no sections were emitted
    if job.sections.is_empty() {
        job.sections.push(LayoutSection {
            leading_space: 0.0,
            byte_range: 0..0,
            format: TextFormat { font_id: FontId::proportional(font_size), ..Default::default() },
        });
    }

    apply_search_highlights(&mut job, search_ranges, current_search);
    job
}

/// Emit formatted sections for a text range, splitting at span boundaries
fn emit_formatted_sections(
    job: &mut LayoutJob,
    start: usize,
    end: usize,
    font_size: f32,
    heading_size: Option<f32>,
    spans: &[FormatSpan],
) {
    // Collect all span boundary points within this range
    let mut boundaries: Vec<usize> = vec![start, end];
    for span in spans {
        if span.start > start && span.start < end { boundaries.push(span.start); }
        if span.end > start && span.end < end { boundaries.push(span.end); }
    }
    boundaries.sort();
    boundaries.dedup();

    for i in 0..boundaries.len() - 1 {
        let s = boundaries[i];
        let e = boundaries[i + 1];
        if s >= e { continue; }

        // Determine formatting at this sub-range
        let mut bold = heading_size.is_some();
        let mut italic = false;
        let mut strikethrough = false;
        let mut underline = false;

        for span in spans {
            if span.start <= s && span.end >= e {
                match span.kind {
                    SpanKind::Bold => bold = true,
                    SpanKind::Italic => italic = true,
                    SpanKind::Strikethrough => strikethrough = true,
                    SpanKind::Underline => underline = true,
                }
            }
        }

        let fs = heading_size.unwrap_or(font_size);
        let family = match (bold, italic) {
            (true, true) => FontFamily::Name("BoldItalic".into()),
            (true, false) => FontFamily::Name("Bold".into()),
            (false, true) => FontFamily::Name("Italic".into()),
            (false, false) => FontFamily::Proportional,
        };

        let mut format = TextFormat {
            font_id: FontId::new(fs, family),
            color: Color32::BLACK,
            ..Default::default()
        };
        if strikethrough {
            format.strikethrough = egui::Stroke::new(1.0, Color32::BLACK);
        }
        if underline {
            format.underline = egui::Stroke::new(1.0, Color32::BLACK);
        }

        job.sections.push(LayoutSection {
            leading_space: 0.0,
            byte_range: s..e,
            format,
        });
    }
}

/// Apply search highlight backgrounds
fn apply_search_highlights(
    job: &mut LayoutJob,
    search_ranges: &[(usize, usize)],
    current_search: Option<usize>,
) {
    if search_ranges.is_empty() { return; }

    let search_bg = Color32::from_rgb(200, 200, 200);
    let current_bg = Color32::from_rgb(120, 120, 120);

    let mut new_sections = Vec::new();
    for section in &job.sections {
        let s_start = section.byte_range.start;
        let s_end = section.byte_range.end;

        let mut splits: Vec<(usize, usize, bool)> = Vec::new();
        for (idx, &(ms, me)) in search_ranges.iter().enumerate() {
            let os = ms.max(s_start);
            let oe = me.min(s_end);
            if os < oe {
                splits.push((os, oe, current_search == Some(idx)));
            }
        }

        if splits.is_empty() {
            new_sections.push(section.clone());
            continue;
        }

        splits.sort_by_key(|s| s.0);
        let mut cursor = s_start;
        for (ms, me, is_current) in splits {
            if cursor < ms {
                let mut fmt = section.format.clone();
                fmt.background = Color32::TRANSPARENT;
                new_sections.push(LayoutSection {
                    leading_space: if cursor == s_start { section.leading_space } else { 0.0 },
                    byte_range: cursor..ms, format: fmt,
                });
            }
            let mut fmt = section.format.clone();
            fmt.background = if is_current { current_bg } else { search_bg };
            new_sections.push(LayoutSection {
                leading_space: 0.0, byte_range: ms..me, format: fmt,
            });
            cursor = me;
        }
        if cursor < s_end {
            let mut fmt = section.format.clone();
            fmt.background = Color32::TRANSPARENT;
            new_sections.push(LayoutSection {
                leading_space: 0.0, byte_range: cursor..s_end, format: fmt,
            });
        }
    }
    job.sections = new_sections;
}
