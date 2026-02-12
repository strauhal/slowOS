//! slowDesign — WYSIWYG document design application

use egui::{
    Color32, ColorImage, Context, FontId, Key, Pos2, Rect, Sense, Stroke,
    TextureHandle, TextureOptions, Vec2,
};
use serde::{Deserialize, Serialize};
use slowcore::storage::{documents_dir, FileBrowser};
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::{status_bar, FileListItem};
use std::collections::HashMap;
use std::path::PathBuf;

// ---------------------------------------------------------------
// Serializable rectangle (egui::Rect doesn't impl serde)
// ---------------------------------------------------------------

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct SerRect {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl From<Rect> for SerRect {
    fn from(r: Rect) -> Self {
        Self {
            min_x: r.min.x,
            min_y: r.min.y,
            max_x: r.max.x,
            max_y: r.max.y,
        }
    }
}

impl From<SerRect> for Rect {
    fn from(r: SerRect) -> Self {
        Rect::from_min_max(Pos2::new(r.min_x, r.min_y), Pos2::new(r.max_x, r.max_y))
    }
}

// ---------------------------------------------------------------
// Element types
// ---------------------------------------------------------------

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub enum ShapeType {
    Rectangle,
    Ellipse,
    Line,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TextBox {
    pub text: String,
    pub font_size: f32,
}

impl Default for TextBox {
    fn default() -> Self {
        Self {
            text: "Text".to_string(),
            font_size: 14.0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ImageElement {
    pub path: PathBuf,
    #[serde(skip)]
    pub texture_id: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ShapeElement {
    pub shape_type: ShapeType,
    pub fill: bool,
    pub stroke_width: f32,
}

impl Default for ShapeElement {
    fn default() -> Self {
        Self {
            shape_type: ShapeType::Rectangle,
            fill: false,
            stroke_width: 2.0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ElementContent {
    TextBox(TextBox),
    Image(ImageElement),
    Shape(ShapeElement),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DesignElement {
    pub id: u64,
    pub rect: SerRect,
    pub content: ElementContent,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SerVec2 {
    pub x: f32,
    pub y: f32,
}

impl From<Vec2> for SerVec2 {
    fn from(v: Vec2) -> Self { Self { x: v.x, y: v.y } }
}

impl From<SerVec2> for Vec2 {
    fn from(v: SerVec2) -> Self { Vec2::new(v.x, v.y) }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Document {
    pub elements: Vec<DesignElement>,
    pub next_id: u64,
    pub page_size: SerVec2,
}

/// Page margin in points (1 inch)
const PAGE_MARGIN: f32 = 72.0;

impl Default for Document {
    fn default() -> Self {
        Self {
            elements: Vec::new(),
            next_id: 1,
            page_size: SerVec2 { x: 612.0, y: 792.0 }, // Letter size
        }
    }
}

impl Document {
    fn get(&self, id: u64) -> Option<&DesignElement> {
        self.elements.iter().find(|e| e.id == id)
    }

    fn get_mut(&mut self, id: u64) -> Option<&mut DesignElement> {
        self.elements.iter_mut().find(|e| e.id == id)
    }

    /// Create a new document with a full-page text box (1 inch margins)
    fn with_initial_text_box() -> Self {
        let mut doc = Self::default();
        let ps = &doc.page_size;
        doc.elements.push(DesignElement {
            id: 1,
            rect: SerRect {
                min_x: PAGE_MARGIN,
                min_y: PAGE_MARGIN,
                max_x: ps.x - PAGE_MARGIN,
                max_y: ps.y - PAGE_MARGIN,
            },
            content: ElementContent::TextBox(TextBox {
                text: String::new(),
                font_size: 14.0,
            }),
        });
        doc.next_id = 2;
        doc
    }
}

// ---------------------------------------------------------------
// Tool types
// ---------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
pub enum Tool {
    Select,
    TextBox,
    Image,
    Rectangle,
    Ellipse,
    Line,
}

// ---------------------------------------------------------------
// Main app state
// ---------------------------------------------------------------

pub struct SlowDesignApp {
    document: Document,
    current_file: Option<PathBuf>,
    modified: bool,

    // Tool state
    tool: Tool,
    selected_id: Option<u64>,

    // Drag state
    dragging: bool,
    drag_offset: Vec2,
    /// Which corner is being resized (0=top-left, 1=top-right, 2=bottom-right, 3=bottom-left)
    resizing_corner: Option<usize>,

    // Drawing state
    drawing_start: Option<Pos2>,

    // Text editing state
    editing_text: bool,

    // Textures
    image_textures: HashMap<String, TextureHandle>,

    // File browser
    show_file_browser: bool,
    file_browser: FileBrowser,
    fb_mode: FbMode,
    save_filename: String,

    // Image picker
    show_image_picker: bool,
    image_browser: FileBrowser,
    pending_image_rect: Option<Rect>,

    // Dialogs
    show_about: bool,
    show_close_confirm: bool,
    close_confirmed: bool,

    // Undo/redo
    undo_stack: Vec<Document>,
    redo_stack: Vec<Document>,

    // Canvas
    scroll_offset: Vec2,
    zoom: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum FbMode {
    Open,
    Save,
    ExportPng,
    ExportPdf,
}

impl SlowDesignApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            document: Document::with_initial_text_box(),
            current_file: None,
            modified: false,
            tool: Tool::Select,
            selected_id: Some(1), // Select the initial text box
            dragging: false,
            drag_offset: Vec2::ZERO,
            resizing_corner: None,
            drawing_start: None,
            editing_text: true, // Start in editing mode
            image_textures: HashMap::new(),
            show_file_browser: false,
            file_browser: FileBrowser::new(documents_dir())
                .with_filter(vec!["sld".to_string()]),
            fb_mode: FbMode::Open,
            save_filename: String::new(),
            show_image_picker: false,
            image_browser: FileBrowser::new(documents_dir())
                .with_filter(vec!["png".to_string(), "jpg".to_string(), "jpeg".to_string(), "gif".to_string(), "bmp".to_string()]),
            pending_image_rect: None,
            show_about: false,
            show_close_confirm: false,
            close_confirmed: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            scroll_offset: Vec2::ZERO,
            zoom: 1.0,
        }
    }

    fn save_undo_state(&mut self) {
        self.undo_stack.push(self.document.clone());
        self.redo_stack.clear();
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
    }

    fn undo(&mut self) {
        if let Some(state) = self.undo_stack.pop() {
            self.redo_stack.push(self.document.clone());
            self.document = state;
            self.selected_id = None;
        }
    }

    fn redo(&mut self) {
        if let Some(state) = self.redo_stack.pop() {
            self.undo_stack.push(self.document.clone());
            self.document = state;
            self.selected_id = None;
        }
    }

    fn new_document(&mut self) {
        self.document = Document::with_initial_text_box();
        self.current_file = None;
        self.modified = false;
        self.selected_id = Some(1);
        self.editing_text = true;
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    fn save(&mut self) {
        if let Some(path) = self.current_file.clone() {
            self.save_to_path(path);
        } else {
            self.fb_mode = FbMode::Save;
            self.show_file_browser = true;
        }
    }

    fn save_to_path(&mut self, path: PathBuf) {
        if let Ok(json) = serde_json::to_string_pretty(&self.document) {
            let path = if path.extension().is_none() {
                path.with_extension("sld")
            } else {
                path
            };
            if std::fs::write(&path, json).is_ok() {
                self.current_file = Some(path);
                self.modified = false;
            }
        }
    }

    fn export_png(&self, path: &PathBuf) {
        let w = self.document.page_size.x as u32;
        let h = self.document.page_size.y as u32;
        let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([255, 255, 255, 255]));
        // Render elements
        for elem in &self.document.elements {
            let r: Rect = elem.rect.into();
            match &elem.content {
                ElementContent::TextBox(tb) => {
                    use ab_glyph::{FontRef, PxScale, Font as AbFont, ScaleFont};
                    let font_data = include_bytes!("../../fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf");
                    let font = FontRef::try_from_slice(font_data).unwrap();
                    let scale = PxScale::from(tb.font_size);
                    let scaled_font = font.as_scaled(scale);
                    let x0 = r.min.x;
                    let y0 = r.min.y;
                    let max_x = r.max.x;
                    let max_y = r.max.y;
                    let line_height = scaled_font.height() + scaled_font.line_gap();
                    let ascent = scaled_font.ascent();
                    let mut cy = y0 + 4.0;
                    for line in tb.text.split('\n') {
                        let baseline_y = cy + ascent;
                        if baseline_y > max_y { break; }
                        let mut cx = x0 + 4.0;
                        for ch in line.chars() {
                            let glyph_id = scaled_font.glyph_id(ch);
                            let advance = scaled_font.h_advance(glyph_id);
                            if cx + advance > max_x { break; }
                            let glyph = glyph_id.with_scale_and_position(scale, ab_glyph::point(cx, baseline_y));
                            if let Some(outlined) = font.outline_glyph(glyph) {
                                let bounds = outlined.px_bounds();
                                outlined.draw(|px, py, cov| {
                                    if cov > 0.5 {
                                        let ix = (bounds.min.x as i32 + px as i32) as u32;
                                        let iy = (bounds.min.y as i32 + py as i32) as u32;
                                        if ix < w && iy < h {
                                            img.put_pixel(ix, iy, image::Rgba([0, 0, 0, 255]));
                                        }
                                    }
                                });
                            }
                            cx += advance;
                        }
                        cy += line_height;
                    }
                }
                ElementContent::Image(ie) => {
                    // Load and render image
                    if let Ok(file_img) = image::open(&ie.path) {
                        let resized = file_img.resize_exact(
                            r.width() as u32,
                            r.height() as u32,
                            image::imageops::FilterType::Nearest,
                        );
                        image::imageops::overlay(&mut img, &resized.to_rgba8(), r.min.x as i64, r.min.y as i64);
                    }
                }
                ElementContent::Shape(shape) => {
                    let x0 = r.min.x as i32;
                    let y0 = r.min.y as i32;
                    let x1 = r.max.x as i32;
                    let y1 = r.max.y as i32;
                    let black = image::Rgba([0, 0, 0, 255]);
                    match shape.shape_type {
                        ShapeType::Rectangle => {
                            if shape.fill {
                                for y in y0.max(0)..y1.min(h as i32) {
                                    for x in x0.max(0)..x1.min(w as i32) {
                                        img.put_pixel(x as u32, y as u32, black);
                                    }
                                }
                            } else {
                                for x in x0.max(0)..x1.min(w as i32) {
                                    if y0 >= 0 && (y0 as u32) < h { img.put_pixel(x as u32, y0 as u32, black); }
                                    if y1 > 0 && (y1 as u32) < h { img.put_pixel(x as u32, (y1 - 1) as u32, black); }
                                }
                                for y in y0.max(0)..y1.min(h as i32) {
                                    if x0 >= 0 && (x0 as u32) < w { img.put_pixel(x0 as u32, y as u32, black); }
                                    if x1 > 0 && (x1 as u32) < w { img.put_pixel((x1 - 1) as u32, y as u32, black); }
                                }
                            }
                        }
                        ShapeType::Line => {
                            // Bresenham line
                            let dx = (x1 - x0).abs();
                            let dy = -(y1 - y0).abs();
                            let sx = if x0 < x1 { 1 } else { -1 };
                            let sy = if y0 < y1 { 1 } else { -1 };
                            let mut err = dx + dy;
                            let mut cx = x0;
                            let mut cy = y0;
                            loop {
                                if cx >= 0 && cx < w as i32 && cy >= 0 && cy < h as i32 {
                                    img.put_pixel(cx as u32, cy as u32, black);
                                }
                                if cx == x1 && cy == y1 { break; }
                                let e2 = 2 * err;
                                if e2 >= dy { err += dy; cx += sx; }
                                if e2 <= dx { err += dx; cy += sy; }
                            }
                        }
                        _ => {
                            // Ellipse or other: draw bounding rect outline as fallback
                            for x in x0.max(0)..x1.min(w as i32) {
                                if y0 >= 0 && (y0 as u32) < h { img.put_pixel(x as u32, y0 as u32, black); }
                                if y1 > 0 && (y1 as u32) < h { img.put_pixel(x as u32, (y1 - 1) as u32, black); }
                            }
                            for y in y0.max(0)..y1.min(h as i32) {
                                if x0 >= 0 && (x0 as u32) < w { img.put_pixel(x0 as u32, y as u32, black); }
                                if x1 > 0 && (x1 as u32) < w { img.put_pixel((x1 - 1) as u32, y as u32, black); }
                            }
                        }
                    }
                }
            }
        }
        let path = if path.extension().is_none() { path.with_extension("png") } else { path.clone() };
        let _ = img.save(&path);
    }

    fn export_pdf(&self, path: &PathBuf) {
        use printpdf::{BuiltinFont, Mm, PdfDocument};

        let pdf_path = if path.extension().is_none() { path.with_extension("pdf") } else { path.clone() };
        let to_mm = |px: f32| -> f32 { px * 25.4 / 96.0 };
        let pw = to_mm(self.document.page_size.x);
        let ph = to_mm(self.document.page_size.y);

        let (doc, page1, layer1) = PdfDocument::new("slowDesign Export", Mm(pw), Mm(ph), "Layer 1");
        let font = doc.add_builtin_font(BuiltinFont::Helvetica).unwrap();

        let layer = doc.get_page(page1).get_layer(layer1);

        for elem in &self.document.elements {
            let r: egui::Rect = elem.rect.into();

            match &elem.content {
                ElementContent::TextBox(tb) => {
                    let font_size_pt = tb.font_size;
                    let line_height_pt = font_size_pt * 1.3;
                    let mut y_offset_pt = 0.0_f32;

                    for line in tb.text.split('\n') {
                        if line.is_empty() {
                            y_offset_pt += line_height_pt;
                            continue;
                        }
                        let x = to_mm(r.min.x) + 1.0;
                        let y = ph - to_mm(r.min.y) - y_offset_pt * 25.4 / 72.0 - font_size_pt * 25.4 / 72.0;
                        layer.use_text(line, font_size_pt, Mm(x), Mm(y), &font);
                        y_offset_pt += line_height_pt;
                    }
                }
                ElementContent::Image(ie) => {
                    if let Ok(file_img) = image::open(&ie.path) {
                        let rgb = file_img.to_rgb8();
                        let (iw, ih) = rgb.dimensions();
                        let image_data = rgb.into_raw();

                        let pdf_img = printpdf::Image::from(printpdf::ImageXObject {
                            width: printpdf::Px(iw as usize),
                            height: printpdf::Px(ih as usize),
                            color_space: printpdf::ColorSpace::Rgb,
                            bits_per_component: printpdf::ColorBits::Bit8,
                            interpolate: true,
                            image_data,
                            image_filter: None,
                            smask: None,
                            clipping_bbox: None,
                        });
                        {
                            let native_w_mm = iw as f32 * 25.4 / 96.0;
                            let native_h_mm = ih as f32 * 25.4 / 96.0;
                            pdf_img.add_to_layer(
                                layer.clone(), printpdf::ImageTransform {
                                    translate_x: Some(Mm(to_mm(r.min.x))),
                                    translate_y: Some(Mm(ph - to_mm(r.max.y))),
                                    scale_x: Some(to_mm(r.width()) / native_w_mm),
                                    scale_y: Some(to_mm(r.height()) / native_h_mm),
                                    ..Default::default()
                                },
                            );
                        }
                    }
                }
                ElementContent::Shape(shape) => {
                    let x0 = to_mm(r.min.x);
                    let y0 = ph - to_mm(r.max.y);
                    let x1 = to_mm(r.max.x);
                    let y1 = ph - to_mm(r.min.y);
                    let outline = printpdf::Line {
                        points: vec![
                            (printpdf::Point::new(Mm(x0), Mm(y0)), false),
                            (printpdf::Point::new(Mm(x1), Mm(y0)), false),
                            (printpdf::Point::new(Mm(x1), Mm(y1)), false),
                            (printpdf::Point::new(Mm(x0), Mm(y1)), false),
                        ],
                        is_closed: true,
                    };
                    layer.set_outline_thickness(1.0);
                    if shape.fill {
                        layer.set_fill_color(printpdf::Color::Rgb(printpdf::Rgb::new(0.0, 0.0, 0.0, None)));
                    }
                    layer.set_outline_color(printpdf::Color::Rgb(printpdf::Rgb::new(0.0, 0.0, 0.0, None)));
                    layer.add_line(outline);
                }
            }
        }

        if let Ok(file) = std::fs::File::create(&pdf_path) {
            let _ = doc.save(&mut std::io::BufWriter::new(file));
        }
    }

    fn open(&mut self, path: PathBuf) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(doc) = serde_json::from_str::<Document>(&content) {
                self.document = doc;
                self.current_file = Some(path);
                self.modified = false;
                self.selected_id = None;
                self.undo_stack.clear();
                self.redo_stack.clear();
            }
        }
    }

    fn add_element(&mut self, content: ElementContent, rect: Rect) {
        self.save_undo_state();
        let id = self.document.next_id;
        self.document.next_id += 1;
        self.document.elements.push(DesignElement { id, rect: rect.into(), content });
        self.selected_id = Some(id);
        self.modified = true;
    }

    /// Calculate required height for a text box based on content
    fn calculate_text_height(text: &str, font_size: f32, box_width: f32) -> f32 {
        let char_width = font_size * 0.6; // Approximate character width
        let line_height = font_size * 1.4; // Line height with spacing
        let padding = 8.0; // Padding inside box

        let chars_per_line = ((box_width - padding * 2.0) / char_width).max(1.0) as usize;
        let mut line_count = 0;

        for paragraph in text.split('\n') {
            if paragraph.is_empty() {
                line_count += 1;
            } else {
                // Word wrap
                let words: Vec<&str> = paragraph.split_whitespace().collect();
                let mut current_line_len = 0;
                for word in words {
                    if current_line_len == 0 {
                        current_line_len = word.len();
                    } else if current_line_len + 1 + word.len() <= chars_per_line {
                        current_line_len += 1 + word.len();
                    } else {
                        line_count += 1;
                        current_line_len = word.len();
                    }
                }
                line_count += 1; // Last line of paragraph
            }
        }

        // Minimum 1 line for empty text
        let line_count = line_count.max(1);
        (line_count as f32 * line_height + padding * 2.0).max(font_size * 2.0)
    }

    /// Auto-resize a text box element to fit its content
    fn auto_resize_text_box(&mut self, element_id: u64) {
        if let Some(elem) = self.document.get_mut(element_id) {
            if let ElementContent::TextBox(ref tb) = elem.content {
                let current_rect: Rect = elem.rect.into();
                let new_height = Self::calculate_text_height(&tb.text, tb.font_size, current_rect.width());
                let new_max_y = current_rect.min.y + new_height;

                // Only grow, don't shrink below minimum
                if new_max_y > current_rect.max.y || (current_rect.max.y - new_max_y > tb.font_size * 2.0) {
                    elem.rect = SerRect {
                        min_x: current_rect.min.x,
                        min_y: current_rect.min.y,
                        max_x: current_rect.max.x,
                        max_y: new_max_y.max(current_rect.min.y + tb.font_size * 2.0),
                    };
                }
            }
        }
    }

    fn delete_selected(&mut self) {
        if let Some(id) = self.selected_id {
            self.save_undo_state();
            self.document.elements.retain(|e| e.id != id);
            self.selected_id = None;
            self.modified = true;
        }
    }

    fn load_image_texture(&mut self, ctx: &Context, path: &PathBuf) -> Option<String> {
        let key = path.to_string_lossy().to_string();
        if self.image_textures.contains_key(&key) {
            return Some(key);
        }

        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(img) = image::load_from_memory(&bytes) {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let color_image = ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    rgba.as_raw(),
                );
                let texture = ctx.load_texture(
                    format!("design_img_{}", key),
                    color_image,
                    TextureOptions::NEAREST,
                );
                self.image_textures.insert(key.clone(), texture);
                return Some(key);
            }
        }
        None
    }

    fn handle_keyboard(&mut self, ctx: &Context) {
        slowcore::theme::consume_special_keys(ctx);

        // Zoom shortcuts — consume before egui handles them
        let zoom_in = ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::COMMAND, Key::Plus) ||
            i.consume_key(egui::Modifiers::COMMAND, Key::Equals)
        });
        let zoom_out = ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::COMMAND, Key::Minus)
        });
        if zoom_in { self.zoom = (self.zoom + 0.25).min(4.0); }
        if zoom_out { self.zoom = (self.zoom - 0.25).max(0.25); }

        ctx.input(|i| {
            let cmd = i.modifiers.command;

            if cmd && i.key_pressed(Key::N) { self.new_document(); }
            if cmd && i.key_pressed(Key::O) {
                self.fb_mode = FbMode::Open;
                self.show_file_browser = true;
            }
            if cmd && i.key_pressed(Key::S) { self.save(); }
            if cmd && i.key_pressed(Key::Z) && !i.modifiers.shift { self.undo(); }
            if cmd && i.key_pressed(Key::Z) && i.modifiers.shift { self.redo(); }
            if (i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) && !self.editing_text {
                self.delete_selected();
            }
            if i.key_pressed(Key::Escape) {
                self.selected_id = None;
                self.editing_text = false;
                self.tool = Tool::Select;
            }

            // Tool shortcuts (only when not editing text)
            if !self.editing_text {
                if i.key_pressed(Key::V) { self.tool = Tool::Select; }
                if i.key_pressed(Key::T) { self.tool = Tool::TextBox; }
                if i.key_pressed(Key::I) { self.tool = Tool::Image; }
                if i.key_pressed(Key::R) { self.tool = Tool::Rectangle; }
                if i.key_pressed(Key::E) { self.tool = Tool::Ellipse; }
                if i.key_pressed(Key::L) { self.tool = Tool::Line; }
            }
        });
    }

    fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let tools = [
                (Tool::Select, "select (V)"),
                (Tool::TextBox, "text (T)"),
                (Tool::Image, "image (I)"),
                (Tool::Rectangle, "rect (R)"),
                (Tool::Ellipse, "ellipse (E)"),
                (Tool::Line, "line (L)"),
            ];

            for (tool, label) in tools {
                if ui.selectable_label(self.tool == tool, label).clicked() {
                    self.tool = tool;
                }
            }
        });
    }

    fn render_canvas(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        let available = ui.available_size();
        let (response, painter) = ui.allocate_painter(available, Sense::click_and_drag());
        let canvas_rect = response.rect;

        // Background
        painter.rect_filled(canvas_rect, 0.0, Color32::from_gray(200));

        // Page
        let page_size = Vec2::from(self.document.page_size.clone()) * self.zoom;
        let page_origin = Pos2::new(
            canvas_rect.center().x - page_size.x / 2.0 + self.scroll_offset.x,
            canvas_rect.min.y + 20.0 + self.scroll_offset.y,
        );
        let page_rect = Rect::from_min_size(page_origin, page_size);

        // Page shadow and background
        painter.rect_filled(
            Rect::from_min_size(page_origin + Vec2::new(4.0, 4.0), page_size),
            0.0, Color32::from_gray(150),
        );
        painter.rect_filled(page_rect, 0.0, SlowColors::WHITE);
        painter.rect_stroke(page_rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));

        // Draw elements
        for element in &self.document.elements {
            let elem_rect: Rect = element.rect.into();
            let screen_rect = self.to_screen_rect(elem_rect, page_origin);
            let is_selected = self.selected_id == Some(element.id);

            match &element.content {
                ElementContent::TextBox(tb) => {
                    if is_selected {
                        painter.rect_stroke(screen_rect, 0.0, Stroke::new(1.0, Color32::BLUE));
                    }
                    // Render text with word wrapping
                    let font_size = tb.font_size * self.zoom;
                    let padding = 4.0 * self.zoom;
                    let text_width = screen_rect.width() - padding * 2.0;
                    let line_height = font_size * 1.4;
                    let char_width = font_size * 0.55; // Approximate
                    let chars_per_line = (text_width / char_width).max(1.0) as usize;

                    // Word wrap the text (preserving multiple spaces)
                    let mut lines: Vec<String> = Vec::new();
                    for paragraph in tb.text.split('\n') {
                        if paragraph.is_empty() {
                            lines.push(String::new());
                        } else {
                            let mut current_line = String::new();
                            for word in paragraph.split(' ') {
                                if current_line.is_empty() {
                                    current_line = word.to_string();
                                } else if current_line.len() + 1 + word.len() <= chars_per_line {
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
                        }
                    }

                    // Render each line
                    for (i, line) in lines.iter().enumerate() {
                        let y = screen_rect.min.y + padding + i as f32 * line_height;
                        if y + line_height > screen_rect.max.y {
                            break; // Stop if we've exceeded the box
                        }
                        painter.text(
                            Pos2::new(screen_rect.min.x + padding, y),
                            egui::Align2::LEFT_TOP,
                            line,
                            FontId::proportional(font_size),
                            SlowColors::BLACK,
                        );
                    }
                }
                ElementContent::Image(img) => {
                    if let Some(key) = &img.texture_id {
                        if let Some(tex) = self.image_textures.get(key) {
                            painter.image(
                                tex.id(), screen_rect,
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                Color32::WHITE,
                            );
                        }
                    }
                    if is_selected {
                        painter.rect_stroke(screen_rect, 0.0, Stroke::new(2.0, Color32::BLUE));
                    }
                }
                ElementContent::Shape(shape) => {
                    let stroke = Stroke::new(shape.stroke_width * self.zoom, SlowColors::BLACK);
                    match shape.shape_type {
                        ShapeType::Rectangle => {
                            if shape.fill {
                                painter.rect_filled(screen_rect, 0.0, SlowColors::BLACK);
                            } else {
                                painter.rect_stroke(screen_rect, 0.0, stroke);
                            }
                        }
                        ShapeType::Ellipse => {
                            let center = screen_rect.center();
                            let radius = screen_rect.size() / 2.0;
                            let points: Vec<Pos2> = (0..64).map(|i| {
                                let t = i as f32 * std::f32::consts::TAU / 64.0;
                                Pos2::new(center.x + radius.x * t.cos(), center.y + radius.y * t.sin())
                            }).collect();
                            if shape.fill {
                                painter.add(egui::Shape::convex_polygon(points, SlowColors::BLACK, Stroke::NONE));
                            } else {
                                painter.add(egui::Shape::closed_line(points, stroke));
                            }
                        }
                        ShapeType::Line => {
                            painter.line_segment([screen_rect.min, screen_rect.max], stroke);
                        }
                    }
                    if is_selected {
                        painter.rect_stroke(screen_rect.expand(2.0), 0.0, Stroke::new(1.0, Color32::BLUE));
                    }
                }
            }

            // Selection handles
            if is_selected {
                for corner in [screen_rect.min, Pos2::new(screen_rect.max.x, screen_rect.min.y),
                              screen_rect.max, Pos2::new(screen_rect.min.x, screen_rect.max.y)] {
                    let h = Rect::from_center_size(corner, Vec2::splat(6.0));
                    painter.rect_filled(h, 0.0, Color32::WHITE);
                    painter.rect_stroke(h, 0.0, Stroke::new(1.0, Color32::BLUE));
                }
            }
        }

        // Drawing preview
        if let Some(start) = self.drawing_start {
            if let Some(current) = response.interact_pointer_pos() {
                painter.rect_stroke(Rect::from_two_pos(start, current), 0.0, Stroke::new(1.0, Color32::BLUE));
            }
        }

        self.handle_canvas_input(&response, page_origin, ctx);
    }

    fn to_screen_rect(&self, r: Rect, page_origin: Pos2) -> Rect {
        Rect::from_min_max(
            page_origin + r.min.to_vec2() * self.zoom,
            page_origin + r.max.to_vec2() * self.zoom,
        )
    }

    fn to_page_pos(&self, screen_pos: Pos2, page_origin: Pos2) -> Pos2 {
        Pos2::new(
            (screen_pos.x - page_origin.x) / self.zoom,
            (screen_pos.y - page_origin.y) / self.zoom,
        )
    }

    fn handle_canvas_input(&mut self, response: &egui::Response, page_origin: Pos2, ctx: &Context) {
        let pointer_pos = response.interact_pointer_pos();

        if response.clicked() {
            if let Some(pos) = pointer_pos {
                let page_pos = self.to_page_pos(pos, page_origin);
                if self.tool == Tool::Select {
                    self.selected_id = None;
                    for element in self.document.elements.iter().rev() {
                        let r: Rect = element.rect.into();
                        if r.contains(page_pos) {
                            self.selected_id = Some(element.id);
                            break;
                        }
                    }
                }
            }
        }

        // Double-click to edit text box
        if response.double_clicked() {
            if let Some(pos) = pointer_pos {
                let page_pos = self.to_page_pos(pos, page_origin);
                for element in self.document.elements.iter().rev() {
                    let r: Rect = element.rect.into();
                    if r.contains(page_pos) {
                        if matches!(element.content, ElementContent::TextBox(_)) {
                            self.selected_id = Some(element.id);
                            self.editing_text = true;
                        }
                        break;
                    }
                }
            }
        }

        if response.drag_started() {
            if let Some(pos) = pointer_pos {
                let page_pos = self.to_page_pos(pos, page_origin);
                match self.tool {
                    Tool::Select => {
                        // First, check if we're clicking on a corner of the currently selected element
                        let mut handled = false;
                        if let Some(id) = self.selected_id {
                            if let Some(elem) = self.document.get(id) {
                                let r: Rect = elem.rect.into();
                                // Check if clicking on a corner handle (for resizing)
                                let handle_size = 6.0 / self.zoom;
                                let corners = [
                                    r.min, // 0: top-left
                                    Pos2::new(r.max.x, r.min.y), // 1: top-right
                                    r.max, // 2: bottom-right
                                    Pos2::new(r.min.x, r.max.y), // 3: bottom-left
                                ];
                                for (i, corner) in corners.iter().enumerate() {
                                    let handle_rect = Rect::from_center_size(*corner, Vec2::splat(handle_size * 2.0));
                                    if handle_rect.contains(page_pos) {
                                        self.resizing_corner = Some(i);
                                        handled = true;
                                        break;
                                    }
                                }
                                // If not on corner, check if on element body for dragging
                                if !handled && r.contains(page_pos) {
                                    self.dragging = true;
                                    self.drag_offset = page_pos - r.min;
                                    handled = true;
                                }
                            }
                        }
                        // If not handled, try to select an element under the pointer
                        if !handled {
                            for element in self.document.elements.iter().rev() {
                                let r: Rect = element.rect.into();
                                if r.contains(page_pos) {
                                    self.selected_id = Some(element.id);
                                    self.dragging = true;
                                    self.drag_offset = page_pos - r.min;
                                    break;
                                }
                            }
                        }
                    }
                    _ => { self.drawing_start = Some(pos); }
                }
            }
        }

        // Handle resizing
        if response.dragged() && self.resizing_corner.is_some() {
            if let Some(pos) = pointer_pos {
                let page_pos = self.to_page_pos(pos, page_origin);
                if let Some(id) = self.selected_id {
                    if let Some(elem) = self.document.get_mut(id) {
                        let r: Rect = elem.rect.into();
                        let new_rect = match self.resizing_corner.unwrap() {
                            0 => Rect::from_min_max(page_pos, r.max), // top-left
                            1 => Rect::from_min_max(Pos2::new(r.min.x, page_pos.y), Pos2::new(page_pos.x, r.max.y)), // top-right
                            2 => Rect::from_min_max(r.min, page_pos), // bottom-right
                            3 => Rect::from_min_max(Pos2::new(page_pos.x, r.min.y), Pos2::new(r.max.x, page_pos.y)), // bottom-left
                            _ => r,
                        };
                        // Ensure minimum size
                        if new_rect.width() > 10.0 && new_rect.height() > 10.0 {
                            elem.rect = new_rect.into();
                            self.modified = true;
                        }
                    }
                }
            }
        }

        if response.dragged() && self.dragging {
            if let Some(pos) = pointer_pos {
                let page_pos = self.to_page_pos(pos, page_origin);
                if let Some(id) = self.selected_id {
                    if let Some(elem) = self.document.get_mut(id) {
                        let r: Rect = elem.rect.into();
                        let new_min = page_pos - self.drag_offset;
                        elem.rect = Rect::from_min_size(new_min, r.size()).into();
                        self.modified = true;
                    }
                }
            }
        }

        if response.drag_stopped() {
            if self.dragging || self.resizing_corner.is_some() {
                self.save_undo_state();
                self.dragging = false;
                self.resizing_corner = None;
            }
            if let Some(start) = self.drawing_start.take() {
                if let Some(end) = pointer_pos {
                    let page_start = self.to_page_pos(start, page_origin);
                    let page_end = self.to_page_pos(end, page_origin);
                    let rect = Rect::from_two_pos(page_start, page_end);
                    if rect.width() > 5.0 && rect.height() > 5.0 {
                        match self.tool {
                            Tool::TextBox => self.add_element(ElementContent::TextBox(TextBox::default()), rect),
                            Tool::Rectangle => self.add_element(ElementContent::Shape(ShapeElement { shape_type: ShapeType::Rectangle, ..Default::default() }), rect),
                            Tool::Ellipse => self.add_element(ElementContent::Shape(ShapeElement { shape_type: ShapeType::Ellipse, ..Default::default() }), rect),
                            Tool::Line => self.add_element(ElementContent::Shape(ShapeElement { shape_type: ShapeType::Line, ..Default::default() }), rect),
                            Tool::Image => {
                                self.pending_image_rect = Some(rect);
                                self.show_image_picker = true;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Scroll with limits
        let scroll = ctx.input(|i| i.raw_scroll_delta);
        if scroll.y != 0.0 {
            self.scroll_offset.y += scroll.y;
            let page_height = self.document.page_size.y * self.zoom;
            let canvas_height = response.rect.height();
            let max_scroll = 50.0;
            let min_scroll = -(page_height + 50.0 - canvas_height).max(0.0);
            self.scroll_offset.y = self.scroll_offset.y.clamp(min_scroll, max_scroll);
        }
        if scroll.x != 0.0 {
            self.scroll_offset.x += scroll.x;
            let page_width = self.document.page_size.x * self.zoom;
            let canvas_width = response.rect.width();
            // Allow just enough scroll to see the page edge + 1px margin
            let limit = ((page_width - canvas_width) / 2.0).max(0.0) + 1.0;
            self.scroll_offset.x = self.scroll_offset.x.clamp(-limit, limit);
        }
    }

    fn render_properties_panel(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(180.0);
        ui.set_max_width(180.0);
        ui.heading("properties");
        ui.separator();

        if let Some(id) = self.selected_id {
            // Clone needed data first
            let elem_data = self.document.elements.iter()
                .find(|e| e.id == id)
                .map(|e| (e.rect.clone(), e.content.clone()));

            if let Some((rect, content)) = elem_data {
                match content {
                    ElementContent::TextBox(tb) => {
                        ui.label("text box");
                        ui.separator();

                        // Text editing
                        let mut text = tb.text.clone();
                        let mut font_size = tb.font_size;

                        ui.label("text:");
                        let text_resp = ui.text_edit_multiline(&mut text);
                        // Request focus if we just entered editing mode (e.g., from double-click)
                        // but NOT while file browser is open (so filename input works)
                        if self.editing_text && !text_resp.has_focus() && !self.show_file_browser {
                            text_resp.request_focus();
                        }
                        self.editing_text = text_resp.has_focus();

                        ui.add_space(8.0);
                        ui.label("font size:");
                        ui.add(egui::Slider::new(&mut font_size, 8.0..=72.0));

                        // Apply changes and auto-resize
                        let mut text_changed = false;
                        if let Some(elem) = self.document.get_mut(id) {
                            if let ElementContent::TextBox(ref mut t) = elem.content {
                                if t.text != text || t.font_size != font_size {
                                    text_changed = t.text != text || t.font_size != font_size;
                                    t.text = text;
                                    t.font_size = font_size;
                                    self.modified = true;
                                }
                            }
                        }
                        // Auto-resize text box to fit content
                        if text_changed {
                            self.auto_resize_text_box(id);
                        }
                    }
                    ElementContent::Image(img) => {
                        ui.label("image");
                        ui.separator();
                        ui.label("file:");
                        ui.label(img.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
                        let r: Rect = rect.into();
                        ui.label(format!("size: {:.0}x{:.0}", r.width(), r.height()));
                    }
                    ElementContent::Shape(shape) => {
                        let name = match shape.shape_type {
                            ShapeType::Rectangle => "rectangle",
                            ShapeType::Ellipse => "ellipse",
                            ShapeType::Line => "line",
                        };
                        ui.label(name);
                        ui.separator();

                        let mut fill = shape.fill;
                        let mut stroke_width = shape.stroke_width;

                        if shape.shape_type != ShapeType::Line {
                            ui.checkbox(&mut fill, "filled");
                        }
                        ui.add_space(8.0);
                        ui.label("stroke:");
                        ui.add(egui::Slider::new(&mut stroke_width, 1.0..=10.0));

                        // Apply
                        if let Some(elem) = self.document.get_mut(id) {
                            if let ElementContent::Shape(ref mut s) = elem.content {
                                if s.fill != fill || s.stroke_width != stroke_width {
                                    s.fill = fill;
                                    s.stroke_width = stroke_width;
                                    self.modified = true;
                                }
                            }
                        }

                        let r: Rect = rect.into();
                        ui.label(format!("size: {:.0}x{:.0}", r.width(), r.height()));
                    }
                }

                // Position/size
                ui.add_space(16.0);
                ui.separator();
                let r: Rect = rect.into();
                let mut x = r.min.x;
                let mut y = r.min.y;
                let mut w = r.width();
                let mut h = r.height();

                ui.label("position:");
                ui.horizontal(|ui| {
                    ui.label("x:");
                    ui.add(egui::DragValue::new(&mut x).speed(1.0));
                    ui.label("y:");
                    ui.add(egui::DragValue::new(&mut y).speed(1.0));
                });
                ui.label("size:");
                ui.horizontal(|ui| {
                    ui.label("w:");
                    ui.add(egui::DragValue::new(&mut w).speed(1.0).clamp_range(10.0..=1000.0));
                    ui.label("h:");
                    ui.add(egui::DragValue::new(&mut h).speed(1.0).clamp_range(10.0..=1000.0));
                });

                // Apply position changes
                let new_rect = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
                if let Some(elem) = self.document.get_mut(id) {
                    let old: Rect = elem.rect.into();
                    if old != new_rect {
                        elem.rect = new_rect.into();
                        self.modified = true;
                    }
                }

                ui.add_space(16.0);
                if ui.button("delete").clicked() {
                    self.delete_selected();
                }
            }
        } else {
            ui.label("no selection");
            ui.add_space(8.0);
            ui.label("select an element\nto edit properties");
        }
    }

    fn render_menu_bar(&mut self, ui: &mut egui::Ui) {
        menu_bar(ui, |ui| {
            ui.menu_button("file", |ui| {
                if ui.button("new          ⌘N").clicked() { self.new_document(); ui.close_menu(); }
                if ui.button("open...      ⌘O").clicked() { self.fb_mode = FbMode::Open; self.show_file_browser = true; ui.close_menu(); }
                if ui.button("save         ⌘S").clicked() { self.save(); ui.close_menu(); }
                if ui.button("save as...").clicked() { self.fb_mode = FbMode::Save; self.show_file_browser = true; ui.close_menu(); }
                ui.separator();
                if ui.button("export as PNG...").clicked() { self.fb_mode = FbMode::ExportPng; self.show_file_browser = true; ui.close_menu(); }
                if ui.button("export as PDF...").clicked() { self.fb_mode = FbMode::ExportPdf; self.show_file_browser = true; ui.close_menu(); }
            });
            ui.menu_button("edit", |ui| {
                if ui.add_enabled(!self.undo_stack.is_empty(), egui::Button::new("undo         ⌘Z")).clicked() { self.undo(); ui.close_menu(); }
                if ui.add_enabled(!self.redo_stack.is_empty(), egui::Button::new("redo        ⇧⌘Z")).clicked() { self.redo(); ui.close_menu(); }
                ui.separator();
                if ui.add_enabled(self.selected_id.is_some(), egui::Button::new("delete       ⌫")).clicked() { self.delete_selected(); ui.close_menu(); }
            });
            ui.menu_button("insert", |ui| {
                if ui.button("text box     T").clicked() { self.tool = Tool::TextBox; ui.close_menu(); }
                if ui.button("image        I").clicked() { self.tool = Tool::Image; ui.close_menu(); }
                if ui.button("rectangle    R").clicked() { self.tool = Tool::Rectangle; ui.close_menu(); }
                if ui.button("ellipse      E").clicked() { self.tool = Tool::Ellipse; ui.close_menu(); }
                if ui.button("line         L").clicked() { self.tool = Tool::Line; ui.close_menu(); }
            });
            ui.menu_button("view", |ui| {
                if ui.button("zoom in       ⌘+").clicked() {
                    self.zoom = (self.zoom + 0.25).min(4.0);
                    ui.close_menu();
                }
                if ui.button("zoom out      ⌘-").clicked() {
                    self.zoom = (self.zoom - 0.25).max(0.25);
                    ui.close_menu();
                }
                if ui.button("zoom to fit").clicked() {
                    self.zoom = 1.0;
                    self.scroll_offset = egui::Vec2::ZERO;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("50%").clicked() { self.zoom = 0.5; ui.close_menu(); }
                if ui.button("75%").clicked() { self.zoom = 0.75; ui.close_menu(); }
                if ui.button("100%").clicked() { self.zoom = 1.0; ui.close_menu(); }
                if ui.button("150%").clicked() { self.zoom = 1.5; ui.close_menu(); }
                if ui.button("200%").clicked() { self.zoom = 2.0; ui.close_menu(); }
            });
            ui.menu_button("help", |ui| {
                if ui.button("about").clicked() { self.show_about = true; ui.close_menu(); }
            });
        });
    }
}

impl eframe::App for SlowDesignApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Load image textures - collect paths first to avoid borrow conflicts
        let images_to_load: Vec<(usize, PathBuf)> = self.document.elements.iter()
            .enumerate()
            .filter_map(|(idx, e)| {
                if let ElementContent::Image(img) = &e.content {
                    if img.texture_id.is_none() {
                        return Some((idx, img.path.clone()));
                    }
                }
                None
            })
            .collect();

        for (idx, path) in images_to_load {
            let texture_id = self.load_image_texture(ctx, &path);
            if let ElementContent::Image(ref mut img) = self.document.elements[idx].content {
                img.texture_id = texture_id;
            }
        }

        self.handle_keyboard(ctx);

        egui::TopBottomPanel::top("menu").show(ctx, |ui| self.render_menu_bar(ui));
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| self.render_toolbar(ui));
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let status = if self.modified { "modified" } else { "saved" };
            let tool_name = match self.tool {
                Tool::Select => "select",
                Tool::TextBox => "text",
                Tool::Image => "image",
                Tool::Rectangle => "rect",
                Tool::Ellipse => "ellipse",
                Tool::Line => "line",
            };
            status_bar(ui, &format!("tool: {}  |  {}  |  zoom: {:.0}%", tool_name, status, self.zoom * 100.0));
        });

        egui::SidePanel::right("properties").exact_width(200.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.render_properties_panel(ui));
        });

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::from_gray(180)))
            .show(ctx, |ui| self.render_canvas(ui, ctx));

        // File browser
        if self.show_file_browser {
            let title = match self.fb_mode {
                FbMode::Open => "open document",
                FbMode::Save => "save document",
                FbMode::ExportPng => "export as PNG",
                FbMode::ExportPdf => "export as PDF",
            };
            let mut close_browser = false;
            let mut open_path: Option<PathBuf> = None;
            let mut save_path: Option<PathBuf> = None;

            egui::Window::new(title).collapsible(false).resizable(false).default_width(380.0).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("location:");
                    ui.label(self.file_browser.current_dir.to_string_lossy().to_string());
                });
                ui.separator();

                egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    let entries = self.file_browser.entries.clone();
                    for (idx, entry) in entries.iter().enumerate() {
                        let selected = self.file_browser.selected_index == Some(idx);
                        let response = ui.add(FileListItem::new(&entry.name, entry.is_directory).selected(selected));
                        if response.clicked() { self.file_browser.selected_index = Some(idx); }
                        if response.double_clicked() {
                            if entry.is_directory {
                                self.file_browser.navigate_to(entry.path.clone());
                            } else if self.fb_mode == FbMode::Open {
                                open_path = Some(entry.path.clone());
                                close_browser = true;
                            }
                        }
                    }
                });

                if self.fb_mode != FbMode::Open {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("filename:");
                        let fname_resp = ui.text_edit_singleline(&mut self.save_filename);
                        if fname_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            // Enter in filename = click save/export
                            if !self.save_filename.is_empty() {
                                save_path = Some(self.file_browser.save_directory().join(&self.save_filename));
                                close_browser = true;
                            }
                        }
                    });
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("cancel").clicked() { close_browser = true; }
                    let action = match self.fb_mode {
                        FbMode::Open => "open",
                        FbMode::Save => "save",
                        FbMode::ExportPng | FbMode::ExportPdf => "export",
                    };
                    if ui.button(action).clicked() {
                        match self.fb_mode {
                            FbMode::Open => {
                                if let Some(entry) = self.file_browser.selected_entry() {
                                    if !entry.is_directory {
                                        open_path = Some(entry.path.clone());
                                        close_browser = true;
                                    }
                                }
                            }
                            FbMode::Save | FbMode::ExportPng | FbMode::ExportPdf => {
                                if !self.save_filename.is_empty() {
                                    save_path = Some(self.file_browser.save_directory().join(&self.save_filename));
                                    close_browser = true;
                                }
                            }
                        }
                    }
                });
            });

            if let Some(path) = open_path { self.open(path); }
            if let Some(path) = save_path {
                match self.fb_mode {
                    FbMode::Save => self.save_to_path(path),
                    FbMode::ExportPng => self.export_png(&path),
                    FbMode::ExportPdf => self.export_pdf(&path),
                    _ => {}
                }
            }
            if close_browser { self.show_file_browser = false; }
        }

        // Image picker
        if self.show_image_picker {
            let mut close_picker = false;
            let mut picked_path: Option<PathBuf> = None;

            egui::Window::new("insert image").collapsible(false).resizable(false).default_width(380.0).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("location:");
                    ui.label(self.image_browser.current_dir.to_string_lossy().to_string());
                });
                ui.separator();

                egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    let entries = self.image_browser.entries.clone();
                    for (idx, entry) in entries.iter().enumerate() {
                        let selected = self.image_browser.selected_index == Some(idx);
                        let response = ui.add(FileListItem::new(&entry.name, entry.is_directory).selected(selected));
                        if response.clicked() { self.image_browser.selected_index = Some(idx); }
                        if response.double_clicked() {
                            if entry.is_directory {
                                self.image_browser.navigate_to(entry.path.clone());
                            } else {
                                picked_path = Some(entry.path.clone());
                                close_picker = true;
                            }
                        }
                    }
                });

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("cancel").clicked() { close_picker = true; }
                    if ui.button("insert").clicked() {
                        if let Some(entry) = self.image_browser.selected_entry() {
                            if !entry.is_directory {
                                picked_path = Some(entry.path.clone());
                                close_picker = true;
                            }
                        }
                    }
                });
            });

            if let Some(path) = picked_path {
                if let Some(rect) = self.pending_image_rect.take() {
                    let texture_id = self.load_image_texture(ctx, &path);
                    self.add_element(ElementContent::Image(ImageElement { path, texture_id }), rect);
                }
            }
            if close_picker { self.show_image_picker = false; self.pending_image_rect = None; }
        }

        // About
        if self.show_about {
            egui::Window::new("about slowDesign")
                .collapsible(false)
                .resizable(false)
                .default_width(280.0)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("slowDesign");
                        ui.label("version 0.1.0");
                        ui.add_space(8.0);
                        ui.label("layout program for slowOS");
                    });
                    ui.add_space(16.0);
                    if ui.button("ok").clicked() { self.show_about = false; }
                });
        }

        // Close confirmation dialog
        if self.show_close_confirm {
            egui::Window::new("unsaved changes")
                .collapsible(false)
                .resizable(false)
                .default_width(300.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("you have unsaved changes.");
                    ui.label("do you want to save before closing?");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("don't save").clicked() {
                            self.close_confirmed = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button("cancel").clicked() {
                            self.show_close_confirm = false;
                        }
                        if ui.button("save").clicked() {
                            self.save();
                            if !self.modified {
                                self.close_confirmed = true;
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                    });
                });
        }

        // Handle close request
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.modified && !self.close_confirmed {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.show_close_confirm = true;
            }
        }
    }
}
