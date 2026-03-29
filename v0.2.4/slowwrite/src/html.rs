//! HTML-aware layouter for egui TextEdit.
//!
//! Two rendering modes:
//! - **Raw mode**: Shows HTML tags in gray (for plain text view)
//! - **Clean mode**: Hides tags entirely — headings, bold, italic etc. just
//!   appear formatted. Users never see `<b>`, `<h1>`, etc.

use egui::text::{LayoutJob, LayoutSection};
use egui::{Color32, FontFamily, FontId, TextFormat};

/// Gray color for HTML tags
const TAG_GRAY: Color32 = Color32::from_gray(160);

/// Highlight color for search matches
const SEARCH_BG: Color32 = Color32::from_rgb(200, 200, 200);
/// Highlight color for the current search match
const CURRENT_SEARCH_BG: Color32 = Color32::from_rgb(120, 120, 120);

/// Layout an HTML document for egui's TextEdit layouter callback.
pub fn layout_html(
    _ui: &egui::Ui,
    text: &str,
    wrap_width: f32,
    font_size: f32,
    hide_tags: bool,
    search_ranges: &[(usize, usize)],
    current_search: Option<usize>,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    job.text = text.to_string();

    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    // Tag display settings — when hiding, use transparent + tiny font
    let tag_color = if hide_tags { Color32::TRANSPARENT } else { TAG_GRAY };
    let tag_size = if hide_tags { 0.1 } else { font_size };

    // Stack of active formatting (font_family, font_size_multiplier)
    let mut bold = false;
    let mut italic = false;
    let mut strikethrough = false;
    let mut heading_size: Option<f32> = None;
    let mut in_code = false;

    while pos < len {
        // Check for HTML tag
        if bytes[pos] == b'<' {
            // Find closing >
            if let Some(tag_end) = find_tag_end(bytes, pos) {
                let tag_str = &text[pos..=tag_end];
                let tag_name = parse_tag_name(tag_str);
                let is_closing = tag_str.starts_with("</");

                // Determine what this tag does
                let is_formatting_tag = matches!(
                    tag_name.as_str(),
                    "b" | "strong" | "i" | "em" | "s" | "del" | "strike" |
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" |
                    "code" | "li" | "ul" | "ol" | "blockquote" | "br" | "hr" | "p" | "img"
                );

                if is_formatting_tag {
                    // Push the tag itself as hidden/gray
                    let tag_font = if in_code && !is_closing {
                        FontId::new(tag_size, FontFamily::Monospace)
                    } else {
                        FontId::new(tag_size, current_family(bold, italic, heading_size.is_some()))
                    };
                    push_section(&mut job, pos..tag_end + 1, tag_font, tag_color, None);

                    // Update formatting state
                    if !is_closing {
                        match tag_name.as_str() {
                            "b" | "strong" => bold = true,
                            "i" | "em" => italic = true,
                            "s" | "del" | "strike" => strikethrough = true,
                            "h1" => heading_size = Some(font_size * 1.5),
                            "h2" => heading_size = Some(font_size * 1.3),
                            "h3" => heading_size = Some(font_size * 1.1),
                            "h4" | "h5" | "h6" => heading_size = Some(font_size * 1.05),
                            "code" => in_code = true,
                            _ => {}
                        }
                    } else {
                        match tag_name.as_str() {
                            "b" | "strong" => bold = false,
                            "i" | "em" => italic = false,
                            "s" | "del" | "strike" => strikethrough = false,
                            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => heading_size = None,
                            "code" => in_code = false,
                            _ => {}
                        }
                    }

                    pos = tag_end + 1;
                    continue;
                }
            }
        }

        // Regular text — find the next tag or end of text
        let text_start = pos;
        while pos < len && bytes[pos] != b'<' {
            pos += 1;
        }

        if text_start < pos {
            let fs = heading_size.unwrap_or(font_size);
            let family = if in_code {
                FontFamily::Monospace
            } else {
                current_family(bold, italic, heading_size.is_some())
            };
            let color = Color32::BLACK;

            if strikethrough {
                push_section_strikethrough(&mut job, text_start..pos, FontId::new(fs, family), color);
            } else {
                push_section(&mut job, text_start..pos, FontId::new(fs, family), color, None);
            }
        }
    }

    // If text is empty, add a default section
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

    apply_search_highlights(&mut job, search_ranges, current_search);
    job
}

/// Determine the font family based on current formatting state
fn current_family(bold: bool, italic: bool, is_heading: bool) -> FontFamily {
    match (bold || is_heading, italic) {
        (true, true) => FontFamily::Name("BoldItalic".into()),
        (true, false) => FontFamily::Name("Bold".into()),
        (false, true) => FontFamily::Name("Italic".into()),
        (false, false) => FontFamily::Proportional,
    }
}

/// Find the closing '>' of an HTML tag starting at `pos` (which should be '<')
fn find_tag_end(bytes: &[u8], pos: usize) -> Option<usize> {
    let max_tag_len = 30; // reasonable limit for tag length
    let limit = (pos + max_tag_len).min(bytes.len());
    for i in (pos + 1)..limit {
        if bytes[i] == b'>' {
            return Some(i);
        }
        if bytes[i] == b'\n' {
            return None; // tags don't span lines
        }
    }
    None
}

/// Parse the tag name from an HTML tag string like "<b>" or "</h1>" or "<img src=...>"
fn parse_tag_name(tag: &str) -> String {
    let inner = tag.trim_start_matches('<').trim_start_matches('/');
    let name: String = inner.chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    name.to_lowercase()
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

/// Apply search highlight backgrounds to existing sections
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

        let mut splits: Vec<(usize, usize, bool)> = Vec::new();
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
            if cursor < ms {
                let mut fmt = section.format.clone();
                fmt.background = Color32::TRANSPARENT;
                new_sections.push(LayoutSection {
                    leading_space: if cursor == s_start { section.leading_space } else { 0.0 },
                    byte_range: cursor..ms,
                    format: fmt,
                });
            }
            let mut fmt = section.format.clone();
            fmt.background = if is_current { CURRENT_SEARCH_BG } else { SEARCH_BG };
            new_sections.push(LayoutSection {
                leading_space: if cursor == s_start && cursor == ms { section.leading_space } else { 0.0 },
                byte_range: ms..me,
                format: fmt,
            });
            cursor = me;
        }
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
