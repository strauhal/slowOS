//! Reader - text rendering and reading position tracking

use crate::book::{Book, ContentBlock};
use egui::{ColorImage, FontId, Pos2, Rect, Response, Sense, Stroke, TextureHandle, Ui, Vec2};
use serde::{Deserialize, Serialize};
use slowcore::theme::SlowColors;
use std::collections::HashMap;

/// Reading position
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct ReadingPosition {
    pub chapter: usize,
    pub scroll_offset: f32,
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
    pub scroll_offset: f32,
    /// Cached content heights for scrolling
    content_height: f32,
    /// Last known view height for page navigation
    view_height: f32,
    /// Cached image textures (keyed by image data hash)
    image_cache: HashMap<u64, (TextureHandle, [u32; 2])>,
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
            content_height: 0.0,
            view_height: 600.0,
            image_cache: HashMap::new(),
        }
    }
    
    /// Go to next chapter
    pub fn next_chapter(&mut self, book: &Book) {
        if self.position.chapter < book.chapter_count().saturating_sub(1) {
            self.position.chapter += 1;
            self.scroll_offset = 0.0;
        }
    }
    
    /// Go to previous chapter
    pub fn prev_chapter(&mut self, _book: &Book) {
        if self.position.chapter > 0 {
            self.position.chapter -= 1;
            self.scroll_offset = 0.0;
        }
    }
    
    /// Go to specific chapter
    pub fn go_to_chapter(&mut self, chapter: usize, book: &Book) {
        if chapter < book.chapter_count() {
            self.position.chapter = chapter;
            self.scroll_offset = 0.0;
        }
    }
    
    /// Scroll by delta
    pub fn scroll(&mut self, delta: f32, view_height: f32) {
        self.scroll_offset = (self.scroll_offset - delta)
            .max(0.0)
            .min((self.content_height - view_height).max(0.0));
    }
    
    /// Page down - returns true if advanced to next chapter
    pub fn page_down(&mut self, view_height: f32, book: &Book) -> bool {
        let old_scroll = self.scroll_offset;
        self.scroll(-(view_height - 50.0), view_height);

        // If we didn't scroll (at bottom), advance to next chapter
        if (self.scroll_offset - old_scroll).abs() < 1.0 &&
           self.position.chapter < book.chapter_count().saturating_sub(1) {
            self.position.chapter += 1;
            self.scroll_offset = 0.0;
            return true;
        }
        false
    }

    /// Page up - returns true if went to previous chapter
    pub fn page_up(&mut self, view_height: f32, _book: &Book) -> bool {
        let old_scroll = self.scroll_offset;
        self.scroll(view_height - 50.0, view_height);

        // If we didn't scroll (at top), go to previous chapter
        if (self.scroll_offset - old_scroll).abs() < 1.0 && self.position.chapter > 0 {
            self.position.chapter -= 1;
            // Go to bottom of previous chapter
            self.scroll_offset = self.content_height;
            return true;
        }
        false
    }

    /// Get the current view height (for use in keyboard handling)
    pub fn last_view_height(&self) -> f32 {
        self.view_height
    }
    
    /// Increase font size
    pub fn increase_font_size(&mut self) {
        self.settings.font_size = (self.settings.font_size + 2.0).min(32.0);
    }
    
    /// Decrease font size
    pub fn decrease_font_size(&mut self) {
        self.settings.font_size = (self.settings.font_size - 2.0).max(12.0);
    }
    
    /// Render the current chapter
    pub fn render(&mut self, ui: &mut Ui, book: &Book, rect: Rect) -> Response {
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
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
        
        let max_width = text_rect.width();
        let mut y = text_rect.min.y - self.scroll_offset;
        
        // Render each content block
        for block in &chapter.content {
            let block_height = self.render_block(
                &painter,
                block,
                Pos2::new(text_rect.min.x, y),
                max_width,
                rect,
            );
            y += block_height + self.settings.paragraph_spacing;
        }
        
        // Update content height for scroll bounds
        self.content_height = y + self.scroll_offset - text_rect.min.y;
        // Store view height for page navigation
        self.view_height = rect.height();

        // Handle scroll
        if response.hovered() {
            ui.input(|i| {
                let scroll = i.raw_scroll_delta.y;
                if scroll != 0.0 {
                    self.scroll(scroll, rect.height());
                }
            });
        }
        
        response
    }
    
    /// Render a content block, return its height
    fn render_block(
        &mut self,
        painter: &egui::Painter,
        block: &ContentBlock,
        pos: Pos2,
        max_width: f32,
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
                self.render_text(painter, text, pos, max_width, font_size, true, clip_rect)
            }
            ContentBlock::Paragraph(text) => {
                self.render_text(painter, text, pos, max_width, self.settings.font_size, false, clip_rect)
            }
            ContentBlock::Quote(text) => {
                // Indent quotes
                let indent = 30.0;
                let quote_pos = Pos2::new(pos.x + indent, pos.y);
                
                // Draw quote bar
                if pos.y > clip_rect.min.y && pos.y < clip_rect.max.y {
                    painter.vline(
                        pos.x + indent / 2.0,
                        pos.y..=pos.y + self.settings.font_size * self.settings.line_height,
                        Stroke::new(2.0, SlowColors::BLACK),
                    );
                }
                
                self.render_text(painter, text, quote_pos, max_width - indent, self.settings.font_size, false, clip_rect)
            }
            ContentBlock::Code(text) => {
                let font = FontId::proportional(self.settings.font_size * 0.9);
                self.render_text_with_font(painter, text, pos, max_width, font, clip_rect)
            }
            ContentBlock::ListItem(text) => {
                let bullet_pos = Pos2::new(pos.x + 10.0, pos.y);
                let text_pos = Pos2::new(pos.x + 25.0, pos.y);
                
                if pos.y > clip_rect.min.y && pos.y < clip_rect.max.y {
                    painter.text(
                        bullet_pos,
                        egui::Align2::LEFT_TOP,
                        "•",
                        FontId::proportional(self.settings.font_size),
                        SlowColors::BLACK,
                    );
                }
                
                self.render_text(painter, text, text_pos, max_width - 25.0, self.settings.font_size, false, clip_rect)
            }
            ContentBlock::HorizontalRule => {
                if pos.y > clip_rect.min.y && pos.y < clip_rect.max.y {
                    painter.hline(
                        pos.x..=pos.x + max_width,
                        pos.y + 10.0,
                        Stroke::new(1.0, SlowColors::BLACK),
                    );
                }
                20.0
            }
            ContentBlock::Image { alt, data } => {
                if let Some(img_data) = data {
                    // Hash the image data for caching
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
                    
                    // Get or create texture
                    let ctx = painter.ctx();
                    if !self.image_cache.contains_key(&hash) {
                        if let Ok(img) = image::load_from_memory(img_data) {
                            let rgba = img.to_rgba8();
                            let (w, h) = (rgba.width(), rgba.height());
                            // Scale to fit max_width while preserving aspect ratio
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
                        let img_height = *h as f32;
                        if pos.y + img_height > clip_rect.min.y && pos.y < clip_rect.max.y {
                            let img_rect = Rect::from_min_size(pos, Vec2::new(*w as f32, img_height));
                            painter.image(tex.id(), img_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), egui::Color32::WHITE);
                        }
                        img_height + 8.0
                    } else {
                        // Fallback placeholder
                        if pos.y > clip_rect.min.y && pos.y < clip_rect.max.y {
                            let img_rect = Rect::from_min_size(pos, Vec2::new(max_width, 40.0));
                            painter.rect_stroke(img_rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));
                            painter.text(img_rect.center(), egui::Align2::CENTER_CENTER, format!("[{}]", alt), FontId::proportional(12.0), SlowColors::BLACK);
                        }
                        40.0
                    }
                } else {
                    // No image data — show placeholder
                    if pos.y > clip_rect.min.y && pos.y < clip_rect.max.y {
                        let img_rect = Rect::from_min_size(pos, Vec2::new(max_width, 40.0));
                        painter.rect_stroke(img_rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));
                        painter.text(img_rect.center(), egui::Align2::CENTER_CENTER, format!("[image: {}]", alt), FontId::proportional(12.0), SlowColors::BLACK);
                    }
                    40.0
                }
            }
        }
    }
    
    /// Render wrapped text, return height
    fn render_text(
        &self,
        painter: &egui::Painter,
        text: &str,
        pos: Pos2,
        max_width: f32,
        font_size: f32,
        bold: bool,
        clip_rect: Rect,
    ) -> f32 {
        let font = if bold {
            FontId::new(font_size, egui::FontFamily::Monospace)
        } else {
            FontId::proportional(font_size)
        };
        
        self.render_text_with_font(painter, text, pos, max_width, font, clip_rect)
    }
    
    fn render_text_with_font(
        &self,
        painter: &egui::Painter,
        text: &str,
        pos: Pos2,
        max_width: f32,
        font: FontId,
        clip_rect: Rect,
    ) -> f32 {
        let line_height = font.size * self.settings.line_height;
        let char_width = font.size * 0.6; // Approximate for monospace
        let chars_per_line = (max_width / char_width) as usize;
        
        if chars_per_line == 0 {
            return line_height;
        }
        
        let lines = wrap_text(text, chars_per_line);
        let mut y = pos.y;
        
        for line in &lines {
            // Only render if visible
            if y + line_height > clip_rect.min.y && y < clip_rect.max.y {
                painter.text(
                    Pos2::new(pos.x, y),
                    egui::Align2::LEFT_TOP,
                    line,
                    font.clone(),
                    SlowColors::BLACK,
                );
            }
            y += line_height;
        }
        
        lines.len() as f32 * line_height
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
