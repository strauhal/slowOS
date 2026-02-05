//! Reader - page-based text rendering (horizontal navigation only)

use crate::book::{Book, ContentBlock};
use egui::{ColorImage, FontId, Pos2, Rect, Response, Sense, Stroke, TextureHandle, Ui, Vec2};
use serde::{Deserialize, Serialize};
use slowcore::theme::SlowColors;
use std::collections::HashMap;

/// Reading position
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct ReadingPosition {
    pub chapter: usize,
    pub page: usize, // Page within current chapter
}

/// Reader settings
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReaderSettings {
    pub font_size: f32,
    pub line_height: f32,
    pub margin: f32,
    pub paragraph_spacing: f32,
}

impl Default for ReaderSettings {
    fn default() -> Self {
        Self {
            font_size: 18.0,
            line_height: 1.5,
            margin: 40.0,
            paragraph_spacing: 16.0,
        }
    }
}

/// Reader state
pub struct Reader {
    pub position: ReadingPosition,
    pub settings: ReaderSettings,
    /// For backward compatibility with library saving
    pub scroll_offset: f32,
    /// Total pages in current chapter (calculated during render)
    total_pages: usize,
    /// Cached image textures (keyed by image data hash)
    image_cache: HashMap<u64, (TextureHandle, [u32; 2])>,
    /// Last view dimensions for calculations
    last_view_width: f32,
    last_view_height: f32,
}

impl Default for Reader {
    fn default() -> Self {
        Self::new()
    }
}

impl Reader {
    pub fn new() -> Self {
        Self {
            position: ReadingPosition::default(),
            settings: ReaderSettings::default(),
            scroll_offset: 0.0,
            total_pages: 1,
            image_cache: HashMap::new(),
            last_view_width: 600.0,
            last_view_height: 400.0,
        }
    }

    /// Go to next page (or next chapter if at end)
    pub fn next_page(&mut self, book: &Book) {
        if self.position.page + 1 < self.total_pages {
            self.position.page += 1;
        } else if self.position.chapter < book.chapter_count().saturating_sub(1) {
            self.position.chapter += 1;
            self.position.page = 0;
        }
    }

    /// Go to previous page (or previous chapter if at start)
    pub fn prev_page(&mut self, _book: &Book) {
        if self.position.page > 0 {
            self.position.page -= 1;
        } else if self.position.chapter > 0 {
            self.position.chapter -= 1;
            // Go to last page of previous chapter
            self.position.page = usize::MAX; // Will be clamped during render
        }
    }

    /// Go to next chapter
    pub fn next_chapter(&mut self, book: &Book) {
        if self.position.chapter < book.chapter_count().saturating_sub(1) {
            self.position.chapter += 1;
            self.position.page = 0;
        }
    }

    /// Go to previous chapter
    pub fn prev_chapter(&mut self, _book: &Book) {
        if self.position.chapter > 0 {
            self.position.chapter -= 1;
            self.position.page = 0;
        }
    }

    /// Go to specific chapter
    pub fn go_to_chapter(&mut self, chapter: usize, book: &Book) {
        if chapter < book.chapter_count() {
            self.position.chapter = chapter;
            self.position.page = 0;
        }
    }

    // Legacy methods for compatibility
    pub fn page_down(&mut self, _view_height: f32, book: &Book) -> bool {
        self.next_page(book);
        false
    }

    pub fn page_up(&mut self, _view_height: f32, book: &Book) -> bool {
        self.prev_page(book);
        false
    }

    pub fn last_view_height(&self) -> f32 {
        self.last_view_height
    }

    /// Increase font size
    pub fn increase_font_size(&mut self) {
        self.settings.font_size = (self.settings.font_size + 2.0).min(32.0);
        self.position.page = 0; // Reset to first page when font changes
    }

    /// Decrease font size
    pub fn decrease_font_size(&mut self) {
        self.settings.font_size = (self.settings.font_size - 2.0).max(12.0);
        self.position.page = 0;
    }

    /// Get current page info for status bar
    pub fn page_info(&self) -> (usize, usize) {
        (self.position.page + 1, self.total_pages.max(1))
    }

    /// Render the current page of the current chapter
    pub fn render(&mut self, ui: &mut Ui, book: &Book, rect: Rect) -> Response {
        let response = ui.allocate_rect(rect, Sense::click());
        let painter = ui.painter_at(rect);

        // Background
        painter.rect_filled(rect, 0.0, SlowColors::WHITE);

        // Get current chapter
        let chapter = match book.chapters.get(self.position.chapter) {
            Some(c) => c,
            None => return response,
        };

        // Calculate text area
        let text_rect = Rect::from_min_max(
            rect.min + Vec2::new(self.settings.margin, self.settings.margin),
            rect.max - Vec2::new(self.settings.margin, self.settings.margin),
        );

        self.last_view_width = text_rect.width();
        self.last_view_height = text_rect.height();

        // Paginate the content - figure out what fits on each page
        let pages = self.paginate_chapter(&chapter.content, text_rect.width(), text_rect.height(), &painter);
        self.total_pages = pages.len().max(1);

        // Clamp page number
        if self.position.page >= self.total_pages {
            self.position.page = self.total_pages.saturating_sub(1);
        }

        // Render current page
        if let Some(page_content) = pages.get(self.position.page) {
            let mut y = text_rect.min.y;
            for (block_idx, start_line, end_line) in page_content {
                if let Some(block) = chapter.content.get(*block_idx) {
                    y += self.render_block_lines(
                        &painter,
                        block,
                        Pos2::new(text_rect.min.x, y),
                        text_rect.width(),
                        *start_line,
                        *end_line,
                        rect,
                    );
                    y += self.settings.paragraph_spacing;
                }
            }
        }

        // Draw page turn hints at edges
        let hint_color = egui::Color32::from_gray(200);
        if self.position.page > 0 || self.position.chapter > 0 {
            // Left arrow hint
            painter.text(
                Pos2::new(rect.min.x + 10.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                "‹",
                FontId::proportional(24.0),
                hint_color,
            );
        }
        if self.position.page + 1 < self.total_pages ||
           self.position.chapter < book.chapter_count().saturating_sub(1) {
            // Right arrow hint
            painter.text(
                Pos2::new(rect.max.x - 10.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                "›",
                FontId::proportional(24.0),
                hint_color,
            );
        }

        // Change cursor to I-beam when hovering over text area
        if let Some(pointer_pos) = ui.ctx().pointer_hover_pos() {
            if text_rect.contains(pointer_pos) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
            }
        }

        // Handle click for page turning
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let mid = rect.center().x;
                if pos.x < mid {
                    self.prev_page(book);
                } else {
                    self.next_page(book);
                }
            }
        }

        response
    }

    /// Paginate chapter content into pages
    /// Returns Vec of pages, where each page is Vec of (block_idx, start_line, end_line)
    fn paginate_chapter(
        &self,
        content: &[ContentBlock],
        width: f32,
        height: f32,
        painter: &egui::Painter,
    ) -> Vec<Vec<(usize, usize, usize)>> {
        let mut pages: Vec<Vec<(usize, usize, usize)>> = Vec::new();
        let mut current_page: Vec<(usize, usize, usize)> = Vec::new();
        let mut current_height = 0.0;

        for (block_idx, block) in content.iter().enumerate() {
            let lines = self.get_block_lines(block, width, painter);
            let line_height = self.get_line_height(block);
            let block_overhead = self.settings.paragraph_spacing;

            if lines.is_empty() {
                // Empty block (like horizontal rule)
                let block_height = self.get_block_fixed_height(block) + block_overhead;
                if current_height + block_height > height && !current_page.is_empty() {
                    pages.push(current_page);
                    current_page = Vec::new();
                    current_height = 0.0;
                }
                current_page.push((block_idx, 0, 1));
                current_height += block_height;
            } else {
                // Text block - can be split across pages
                let mut line_idx = 0;
                while line_idx < lines.len() {
                    let remaining_height = height - current_height;
                    let lines_that_fit = (remaining_height / line_height).floor() as usize;

                    if lines_that_fit == 0 {
                        // Start new page
                        if !current_page.is_empty() {
                            pages.push(current_page);
                            current_page = Vec::new();
                            current_height = 0.0;
                        } else {
                            // Can't fit even one line - force at least one
                            let end = (line_idx + 1).min(lines.len());
                            current_page.push((block_idx, line_idx, end));
                            line_idx = end;
                            pages.push(current_page);
                            current_page = Vec::new();
                            current_height = 0.0;
                        }
                    } else {
                        let lines_to_render = lines_that_fit.min(lines.len() - line_idx);
                        let end = line_idx + lines_to_render;
                        current_page.push((block_idx, line_idx, end));
                        current_height += lines_to_render as f32 * line_height + block_overhead;
                        line_idx = end;
                    }
                }
            }
        }

        if !current_page.is_empty() {
            pages.push(current_page);
        }

        if pages.is_empty() {
            pages.push(Vec::new()); // At least one empty page
        }

        pages
    }

    fn get_line_height(&self, block: &ContentBlock) -> f32 {
        let font_size = match block {
            ContentBlock::Heading { level, .. } => match level {
                1 => self.settings.font_size * 1.8,
                2 => self.settings.font_size * 1.5,
                3 => self.settings.font_size * 1.3,
                _ => self.settings.font_size * 1.1,
            },
            ContentBlock::Code(_) => self.settings.font_size * 0.9,
            _ => self.settings.font_size,
        };
        font_size * self.settings.line_height
    }

    fn get_block_fixed_height(&self, block: &ContentBlock) -> f32 {
        match block {
            ContentBlock::HorizontalRule => 20.0,
            ContentBlock::Image { .. } => 200.0, // Approximate
            _ => 0.0,
        }
    }

    /// Get wrapped lines for a block
    fn get_block_lines(&self, block: &ContentBlock, width: f32, _painter: &egui::Painter) -> Vec<String> {
        let (text, font_size) = match block {
            ContentBlock::Heading { level, text } => {
                let size = match level {
                    1 => self.settings.font_size * 1.8,
                    2 => self.settings.font_size * 1.5,
                    3 => self.settings.font_size * 1.3,
                    _ => self.settings.font_size * 1.1,
                };
                (text.as_str(), size)
            }
            ContentBlock::Paragraph(text) => (text.as_str(), self.settings.font_size),
            ContentBlock::Quote(text) => (text.as_str(), self.settings.font_size),
            ContentBlock::Code(text) => (text.as_str(), self.settings.font_size * 0.9),
            ContentBlock::ListItem(text) => (text.as_str(), self.settings.font_size),
            ContentBlock::HorizontalRule | ContentBlock::Image { .. } => {
                return Vec::new(); // No text lines
            }
        };

        let char_width = font_size * 0.5; // Better estimate for proportional fonts
        let effective_width = match block {
            ContentBlock::Quote(_) => width - 30.0,
            ContentBlock::ListItem(_) => width - 25.0,
            _ => width,
        };
        let chars_per_line = (effective_width / char_width) as usize;

        if chars_per_line == 0 {
            return vec![text.to_string()];
        }

        wrap_text(text, chars_per_line)
    }

    /// Render specific lines of a block
    fn render_block_lines(
        &mut self,
        painter: &egui::Painter,
        block: &ContentBlock,
        pos: Pos2,
        max_width: f32,
        start_line: usize,
        end_line: usize,
        clip_rect: Rect,
    ) -> f32 {
        match block {
            ContentBlock::Heading { level, text } => {
                let font_size = match level {
                    1 => self.settings.font_size * 1.8,
                    2 => self.settings.font_size * 1.5,
                    3 => self.settings.font_size * 1.3,
                    _ => self.settings.font_size * 1.1,
                };
                self.render_text_lines(painter, text, pos, max_width, font_size, true, start_line, end_line, clip_rect)
            }
            ContentBlock::Paragraph(text) => {
                self.render_text_lines(painter, text, pos, max_width, self.settings.font_size, false, start_line, end_line, clip_rect)
            }
            ContentBlock::Quote(text) => {
                let indent = 30.0;
                let quote_pos = Pos2::new(pos.x + indent, pos.y);

                // Draw quote bar
                let line_height = self.settings.font_size * self.settings.line_height;
                let bar_height = (end_line - start_line) as f32 * line_height;
                painter.vline(
                    pos.x + indent / 2.0,
                    pos.y..=pos.y + bar_height,
                    Stroke::new(2.0, SlowColors::BLACK),
                );

                self.render_text_lines(painter, text, quote_pos, max_width - indent, self.settings.font_size, false, start_line, end_line, clip_rect)
            }
            ContentBlock::Code(text) => {
                self.render_text_lines(painter, text, pos, max_width, self.settings.font_size * 0.9, false, start_line, end_line, clip_rect)
            }
            ContentBlock::ListItem(text) => {
                let text_pos = Pos2::new(pos.x + 25.0, pos.y);

                // Only draw bullet on first line of item
                if start_line == 0 {
                    painter.text(
                        Pos2::new(pos.x + 10.0, pos.y),
                        egui::Align2::LEFT_TOP,
                        "•",
                        FontId::proportional(self.settings.font_size),
                        SlowColors::BLACK,
                    );
                }

                self.render_text_lines(painter, text, text_pos, max_width - 25.0, self.settings.font_size, false, start_line, end_line, clip_rect)
            }
            ContentBlock::HorizontalRule => {
                painter.hline(
                    pos.x..=pos.x + max_width,
                    pos.y + 10.0,
                    Stroke::new(1.0, SlowColors::BLACK),
                );
                20.0
            }
            ContentBlock::Image { alt, data } => {
                if let Some(img_data) = data {
                    let hash = {
                        use std::hash::{Hash, Hasher};
                        let mut hasher = std::collections::hash_map::DefaultHasher::new();
                        img_data.len().hash(&mut hasher);
                        if img_data.len() >= 8 {
                            img_data[..8].hash(&mut hasher);
                            img_data[img_data.len()-8..].hash(&mut hasher);
                        }
                        hasher.finish()
                    };

                    let ctx = painter.ctx();
                    if !self.image_cache.contains_key(&hash) {
                        // Check if this is SVG data (starts with <svg)
                        let is_svg = img_data.starts_with(b"<svg") ||
                            (img_data.len() > 100 && String::from_utf8_lossy(&img_data[..100]).contains("<svg"));

                        if is_svg {
                            // Render SVG using resvg
                            if let Some((rgba_data, width, height)) = render_svg(img_data, max_width as u32) {
                                let color_image = ColorImage::from_rgba_unmultiplied(
                                    [width as usize, height as usize],
                                    &rgba_data,
                                );
                                let tex = ctx.load_texture(
                                    format!("svg_img_{}", hash),
                                    color_image,
                                    egui::TextureOptions::LINEAR,
                                );
                                self.image_cache.insert(hash, (tex, [width, height]));
                            }
                        } else if let Ok(img) = image::load_from_memory(img_data) {
                            let grey = img.grayscale();
                            let rgba = grey.to_rgba8();
                            let (w, h) = (rgba.width(), rgba.height());
                            let scale = (max_width / w as f32).min(1.0);
                            let dw = (w as f32 * scale) as u32;
                            let dh = (h as f32 * scale) as u32;
                            let resized = image::imageops::resize(&rgba, dw, dh, image::imageops::FilterType::Triangle);
                            let color_image = ColorImage::from_rgba_unmultiplied(
                                [dw as usize, dh as usize],
                                resized.as_raw(),
                            );
                            let tex = ctx.load_texture(
                                format!("epub_img_{}", hash),
                                color_image,
                                egui::TextureOptions::LINEAR,
                            );
                            self.image_cache.insert(hash, (tex, [dw, dh]));
                        }
                    }

                    if let Some((tex, [w, h])) = self.image_cache.get(&hash) {
                        let img_rect = Rect::from_min_size(pos, Vec2::new(*w as f32, *h as f32));
                        painter.image(tex.id(), img_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), egui::Color32::WHITE);
                        *h as f32 + 8.0
                    } else {
                        self.render_placeholder(painter, pos, max_width, alt)
                    }
                } else {
                    self.render_placeholder(painter, pos, max_width, alt)
                }
            }
        }
    }

    fn render_placeholder(&self, painter: &egui::Painter, pos: Pos2, max_width: f32, alt: &str) -> f32 {
        let img_rect = Rect::from_min_size(pos, Vec2::new(max_width, 40.0));
        painter.rect_stroke(img_rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));
        painter.text(img_rect.center(), egui::Align2::CENTER_CENTER, format!("[{}]", alt), FontId::proportional(12.0), SlowColors::BLACK);
        40.0
    }

    /// Render specific lines of wrapped text
    fn render_text_lines(
        &self,
        painter: &egui::Painter,
        text: &str,
        pos: Pos2,
        max_width: f32,
        font_size: f32,
        bold: bool,
        start_line: usize,
        end_line: usize,
        _clip_rect: Rect,
    ) -> f32 {
        let font = if bold {
            FontId::new(font_size, egui::FontFamily::Monospace)
        } else {
            FontId::proportional(font_size)
        };

        let line_height = font_size * self.settings.line_height;
        let char_width = font_size * 0.5;
        let chars_per_line = (max_width / char_width) as usize;

        if chars_per_line == 0 {
            return line_height;
        }

        let lines = wrap_text(text, chars_per_line);
        let mut y = pos.y;

        for (i, line) in lines.iter().enumerate() {
            if i >= start_line && i < end_line {
                painter.text(
                    Pos2::new(pos.x, y),
                    egui::Align2::LEFT_TOP,
                    line,
                    font.clone(),
                    SlowColors::BLACK,
                );
                y += line_height;
            }
        }

        (end_line - start_line) as f32 * line_height
    }
}

/// Simple word-wrap implementation
fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= max_chars {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

/// Render SVG data to RGBA bitmap
/// Returns (rgba_data, width, height) or None if rendering fails
fn render_svg(svg_data: &[u8], max_width: u32) -> Option<(Vec<u8>, u32, u32)> {
    // Parse SVG
    let svg_str = std::str::from_utf8(svg_data).ok()?;
    let opt = resvg::usvg::Options::default();
    let fontdb = resvg::usvg::fontdb::Database::new();
    let tree = resvg::usvg::Tree::from_str(svg_str, &opt, &fontdb).ok()?;

    // Get original size
    let size = tree.size();
    let orig_width = size.width();
    let orig_height = size.height();

    if orig_width <= 0.0 || orig_height <= 0.0 {
        return None;
    }

    // Calculate scaled size to fit within max_width
    let scale = (max_width as f32 / orig_width).min(1.0);
    let width = (orig_width * scale) as u32;
    let height = (orig_height * scale) as u32;

    if width == 0 || height == 0 {
        return None;
    }

    // Create pixmap
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;

    // Fill with white background
    pixmap.fill(resvg::tiny_skia::Color::WHITE);

    // Render SVG
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Return RGBA data
    Some((pixmap.data().to_vec(), width, height))
}
