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
    pub bold: bool,
    pub italic: bool,
}

impl Default for TextBox {
    fn default() -> Self {
        Self {
            text: "Text".to_string(),
            font_size: 14.0,
            bold: false,
            italic: false,
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

impl Default for Document {
    fn default() -> Self {
        Self {
            elements: Vec::new(),
            next_id: 1,
            page_size: SerVec2 { x: 612.0, y: 792.0 }, // Letter size
        }
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
}

impl SlowDesignApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Create initial document with a text box at 1 inch from top/left (72 points = 1 inch)
        let mut document = Document::default();
        let initial_text_box = DesignElement {
            id: 1,
            rect: SerRect {
                min_x: 72.0,  // 1 inch from left
                min_y: 72.0,  // 1 inch from top
                max_x: 400.0, // Wide enough for typing
                max_y: 100.0, // Initial height
            },
            content: ElementContent::TextBox(TextBox {
                text: String::new(), // Empty, ready for typing
                font_size: 14.0,
                bold: false,
                italic: false,
            }),
        };
        document.elements.push(initial_text_box);
        document.next_id = 2;

        Self {
            document,
            current_file: None,
            modified: false,
            tool: Tool::Select,
            selected_id: Some(1), // Select the initial text box
            dragging: false,
            drag_offset: Vec2::ZERO,
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
        // Create fresh document with initial text box at 1 inch from top/left
        let mut document = Document::default();
        let initial_text_box = DesignElement {
            id: 1,
            rect: SerRect {
                min_x: 72.0,  // 1 inch from left
                min_y: 72.0,  // 1 inch from top
                max_x: 400.0,
                max_y: 100.0,
            },
            content: ElementContent::TextBox(TextBox {
                text: String::new(),
                font_size: 14.0,
                bold: false,
                italic: false,
            }),
        };
        document.elements.push(initial_text_box);
        document.next_id = 2;

        self.document = document;
        self.current_file = None;
        self.modified = false;
        self.selected_id = Some(1);
        self.editing_text = true;
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    fn save(&mut self) {
        if let Some(path) = &self.current_file.clone() {
            self.save_to_path(path.clone());
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
                    painter.text(
                        screen_rect.min + Vec2::new(4.0, 4.0),
                        egui::Align2::LEFT_TOP,
                        &tb.text,
                        FontId::proportional(tb.font_size * self.zoom),
                        SlowColors::BLACK,
                    );
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

        if response.drag_started() {
            if let Some(pos) = pointer_pos {
                let page_pos = self.to_page_pos(pos, page_origin);
                match self.tool {
                    Tool::Select => {
                        if let Some(id) = self.selected_id {
                            if let Some(elem) = self.document.elements.iter().find(|e| e.id == id) {
                                let r: Rect = elem.rect.into();
                                if r.contains(page_pos) {
                                    self.dragging = true;
                                    self.drag_offset = page_pos - r.min;
                                }
                            }
                        }
                    }
                    _ => { self.drawing_start = Some(pos); }
                }
            }
        }

        if response.dragged() && self.dragging {
            if let Some(pos) = pointer_pos {
                let page_pos = self.to_page_pos(pos, page_origin);
                if let Some(id) = self.selected_id {
                    if let Some(elem) = self.document.elements.iter_mut().find(|e| e.id == id) {
                        let r: Rect = elem.rect.into();
                        let new_min = page_pos - self.drag_offset;
                        elem.rect = Rect::from_min_size(new_min, r.size()).into();
                        self.modified = true;
                    }
                }
            }
        }

        if response.drag_stopped() {
            if self.dragging {
                self.save_undo_state();
                self.dragging = false;
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

        // Scroll
        let scroll = ctx.input(|i| i.raw_scroll_delta);
        if scroll.y != 0.0 { self.scroll_offset.y += scroll.y; }
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
                        let mut bold = tb.bold;
                        let mut italic = tb.italic;

                        ui.label("text:");
                        let text_resp = ui.text_edit_multiline(&mut text);
                        self.editing_text = text_resp.has_focus();

                        ui.add_space(8.0);
                        ui.label("font size:");
                        ui.add(egui::Slider::new(&mut font_size, 8.0..=72.0));

                        ui.add_space(8.0);
                        ui.checkbox(&mut bold, "bold");
                        ui.checkbox(&mut italic, "italic");

                        // Apply changes
                        if let Some(elem) = self.document.elements.iter_mut().find(|e| e.id == id) {
                            if let ElementContent::TextBox(ref mut t) = elem.content {
                                if t.text != text || t.font_size != font_size || t.bold != bold || t.italic != italic {
                                    t.text = text;
                                    t.font_size = font_size;
                                    t.bold = bold;
                                    t.italic = italic;
                                    self.modified = true;
                                }
                            }
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
                        if let Some(elem) = self.document.elements.iter_mut().find(|e| e.id == id) {
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
                if let Some(elem) = self.document.elements.iter_mut().find(|e| e.id == id) {
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
            let title = if self.fb_mode == FbMode::Open { "open document" } else { "save document" };
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

                if self.fb_mode == FbMode::Save {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("filename:");
                        ui.text_edit_singleline(&mut self.save_filename);
                    });
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("cancel").clicked() { close_browser = true; }
                    let action = if self.fb_mode == FbMode::Open { "open" } else { "save" };
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
                            FbMode::Save => {
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
            if let Some(path) = save_path { self.save_to_path(path); }
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
            egui::Window::new("about slowDesign").collapsible(false).resizable(false).default_width(280.0).show(ctx, |ui| {
                ui.label("slowDesign v0.1.0");
                ui.add_space(8.0);
                ui.label("a WYSIWYG document design app");
                ui.label("for the slow computer.");
                ui.add_space(16.0);
                if ui.button("ok").clicked() { self.show_about = false; }
            });
        }
    }
}
