//! SlowPaint application — e-ink edition
//!
//! Black and white only. Live shape preview outlines.
//! Pattern fills instead of colors.

use crate::canvas::Canvas;
use crate::tools::{BrushSize, Pattern, Tool, BLACK, WHITE};
use egui::{Context, Key, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2};
use image::Rgba;
use slowcore::storage::{FileBrowser, documents_dir};
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::status_bar;
use std::path::PathBuf;

pub struct SlowPaintApp {
    canvas: Canvas,
    texture: Option<TextureHandle>,
    texture_dirty: bool,
    current_tool: Tool,
    brush_size: BrushSize,
    /// true = draw black, false = draw white (erase)
    draw_black: bool,
    /// Fill pattern for filled shapes and fill tool
    fill_pattern: Pattern,
    // Drawing state
    is_drawing: bool,
    drag_start: Option<(i32, i32)>,
    last_point: Option<(i32, i32)>,
    /// Current mouse position in canvas coords (for shape preview)
    hover_canvas_pos: Option<(i32, i32)>,
    // View state
    zoom: f32,
    pan_offset: Vec2,
    /// The canvas rect from last frame (for coordinate conversion)
    last_canvas_rect: Option<Rect>,
    // Dialogs
    show_file_browser: bool,
    file_browser: FileBrowser,
    file_browser_mode: FileBrowserMode,
    save_filename: String,
    show_new_dialog: bool,
    new_width: String,
    new_height: String,
    show_about: bool,
    show_close_confirm: bool,
    close_confirmed: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum FileBrowserMode { Open, Save }

impl SlowPaintApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            canvas: Canvas::new(640, 480),
            texture: None,
            texture_dirty: true,
            current_tool: Tool::Pencil,
            brush_size: BrushSize::Size2,
            draw_black: true,
            fill_pattern: Pattern::Solid,
            is_drawing: false,
            drag_start: None,
            last_point: None,
            hover_canvas_pos: None,
            zoom: 1.0,
            pan_offset: Vec2::ZERO,
            last_canvas_rect: None,
            show_file_browser: false,
            file_browser: FileBrowser::new(documents_dir())
                .with_filter(vec!["png".into(), "bmp".into(), "jpg".into(), "jpeg".into()]),
            file_browser_mode: FileBrowserMode::Open,
            save_filename: String::new(),
            show_new_dialog: false,
            new_width: "640".to_string(),
            new_height: "480".to_string(),
            show_about: false,
            show_close_confirm: false,
            close_confirmed: false,
        }
    }

    fn draw_color(&self) -> Rgba<u8> {
        if self.draw_black { BLACK } else { WHITE }
    }

    fn erase_color(&self) -> Rgba<u8> {
        if self.draw_black { WHITE } else { BLACK }
    }

    fn new_canvas(&mut self, width: u32, height: u32) {
        self.canvas = Canvas::new(width, height);
        self.texture_dirty = true;
        self.zoom = 1.0;
        self.pan_offset = Vec2::ZERO;
    }

    pub fn open_file(&mut self, path: PathBuf) {
        match Canvas::open(path) {
            Ok(canvas) => {
                self.canvas = canvas;
                self.texture_dirty = true;
                self.zoom = 1.0;
                self.pan_offset = Vec2::ZERO;
            }
            Err(e) => eprintln!("failed to open: {}", e),
        }
    }

    fn save(&mut self) {
        if self.canvas.path.is_some() {
            if let Err(e) = self.canvas.save() {
                eprintln!("Failed to save: {}", e);
            }
        } else {
            self.show_save_dialog();
        }
    }

    fn save_as(&mut self, path: PathBuf) {
        if let Err(e) = self.canvas.save_as(path) {
            eprintln!("Failed to save: {}", e);
        }
    }

    fn show_open_dialog(&mut self) {
        self.file_browser = FileBrowser::new(documents_dir())
            .with_filter(vec!["png".into(), "bmp".into(), "jpg".into(), "jpeg".into()]);
        self.file_browser_mode = FileBrowserMode::Open;
        self.show_file_browser = true;
    }

    fn show_save_dialog(&mut self) {
        self.file_browser = FileBrowser::new(documents_dir());
        self.file_browser_mode = FileBrowserMode::Save;
        self.save_filename = "untitled.png".to_string();
        self.show_file_browser = true;
    }

    fn update_texture(&mut self, ctx: &Context) {
        if self.texture_dirty {
            let image = self.canvas.to_texture_data();
            self.texture = Some(ctx.load_texture("canvas", image, egui::TextureOptions::NEAREST));
            self.texture_dirty = false;
        }
    }

    fn screen_to_canvas(&self, screen_pos: Pos2, canvas_rect: Rect) -> (i32, i32) {
        let rel = screen_pos - canvas_rect.min - self.pan_offset;
        let x = (rel.x / self.zoom) as i32;
        let y = (rel.y / self.zoom) as i32;
        (x, y)
    }

    fn canvas_to_screen(&self, cx: i32, cy: i32, canvas_rect: Rect) -> Pos2 {
        Pos2::new(
            canvas_rect.min.x + self.pan_offset.x + cx as f32 * self.zoom,
            canvas_rect.min.y + self.pan_offset.y + cy as f32 * self.zoom,
        )
    }

    fn handle_drawing(&mut self, canvas_rect: Rect, response: &egui::Response) {
        // Track hover position for shape preview
        if let Some(pos) = response.hover_pos() {
            self.hover_canvas_pos = Some(self.screen_to_canvas(pos, canvas_rect));
        } else {
            self.hover_canvas_pos = None;
        }

        if let Some(pos) = response.interact_pointer_pos() {
            let (x, y) = self.screen_to_canvas(pos, canvas_rect);

            if response.drag_started() {
                self.is_drawing = true;
                self.drag_start = Some((x, y));
                self.last_point = Some((x, y));

                if self.current_tool.is_continuous() {
                    self.canvas.save_undo_state();
                }

                match self.current_tool {
                    Tool::Fill => {
                        self.canvas.save_undo_state();
                        if x >= 0 && y >= 0 {
                            // Use pattern fill
                            self.canvas.pattern_fill(
                                x as u32, y as u32,
                                self.draw_color(),
                                &self.fill_pattern,
                            );
                        }
                        self.texture_dirty = true;
                    }
                    Tool::Pencil | Tool::Brush => {
                        let size = self.brush_size.pixels();
                        self.canvas.draw_circle_filled(x, y, size as i32 / 2, self.draw_color());
                        self.texture_dirty = true;
                    }
                    Tool::Eraser => {
                        let size = self.brush_size.pixels();
                        self.canvas.draw_circle_filled(x, y, size as i32 / 2, self.erase_color());
                        self.texture_dirty = true;
                    }
                    _ => {}
                }
            }

            if response.dragged() && self.is_drawing {
                // Update hover for live preview
                self.hover_canvas_pos = Some((x, y));

                if self.current_tool.is_continuous() {
                    if let Some((lx, ly)) = self.last_point {
                        let color = if self.current_tool == Tool::Eraser {
                            self.erase_color()
                        } else {
                            self.draw_color()
                        };
                        self.canvas.draw_line(lx, ly, x, y, color, self.brush_size.pixels());
                        self.texture_dirty = true;
                    }
                    self.last_point = Some((x, y));
                }
            }

            if response.drag_stopped() && self.is_drawing {
                if let Some((sx, sy)) = self.drag_start {
                    if self.current_tool.is_shape() {
                        self.canvas.save_undo_state();
                        let color = self.draw_color();
                        match self.current_tool {
                            Tool::Line => {
                                self.canvas.draw_line(sx, sy, x, y, color, self.brush_size.pixels());
                            }
                            Tool::Rectangle => {
                                self.canvas.draw_rect_outline(sx, sy, x, y, color);
                            }
                            Tool::FilledRectangle => {
                                self.canvas.draw_rect_filled_pattern(sx, sy, x, y, color, &self.fill_pattern);
                            }
                            Tool::Ellipse => {
                                let cx = (sx + x) / 2;
                                let cy = (sy + y) / 2;
                                let rx = (x - sx).abs() / 2;
                                let ry = (y - sy).abs() / 2;
                                self.canvas.draw_ellipse_outline(cx, cy, rx, ry, color);
                            }
                            Tool::FilledEllipse => {
                                let cx = (sx + x) / 2;
                                let cy = (sy + y) / 2;
                                let rx = (x - sx).abs() / 2;
                                let ry = (y - sy).abs() / 2;
                                self.canvas.draw_ellipse_filled_pattern(cx, cy, rx, ry, color, &self.fill_pattern);
                            }
                            _ => {}
                        }
                        self.texture_dirty = true;
                    }
                }
                self.is_drawing = false;
                self.drag_start = None;
                self.last_point = None;
            }
        }
    }

    /// Draw a live preview outline of the shape being dragged
    fn render_shape_preview(&self, painter: &egui::Painter, canvas_rect: Rect) {
        if !self.is_drawing || !self.current_tool.is_shape() { return; }

        let (sx, sy) = match self.drag_start {
            Some(p) => p,
            None => return,
        };
        let (ex, ey) = match self.hover_canvas_pos {
            Some(p) => p,
            None => return,
        };

        let preview_stroke = Stroke::new(1.0, SlowColors::BLACK);

        match self.current_tool {
            Tool::Line => {
                let p1 = self.canvas_to_screen(sx, sy, canvas_rect);
                let p2 = self.canvas_to_screen(ex, ey, canvas_rect);
                painter.line_segment([p1, p2], preview_stroke);
            }
            Tool::Rectangle | Tool::FilledRectangle => {
                let p1 = self.canvas_to_screen(sx, sy, canvas_rect);
                let p2 = self.canvas_to_screen(ex, ey, canvas_rect);
                let rect = Rect::from_two_pos(p1, p2);
                painter.rect_stroke(rect, 0.0, preview_stroke);
            }
            Tool::Ellipse | Tool::FilledEllipse => {
                let p1 = self.canvas_to_screen(sx, sy, canvas_rect);
                let p2 = self.canvas_to_screen(ex, ey, canvas_rect);
                let center = p1 + (p2 - p1) * 0.5;
                let radius = Vec2::new(
                    (p2.x - p1.x).abs() / 2.0,
                    (p2.y - p1.y).abs() / 2.0,
                );
                // Approximate ellipse with line segments
                let n = 48;
                let mut points = Vec::with_capacity(n + 1);
                for i in 0..=n {
                    let angle = (i as f32 / n as f32) * std::f32::consts::TAU;
                    points.push(Pos2::new(
                        center.x + radius.x * angle.cos(),
                        center.y + radius.y * angle.sin(),
                    ));
                }
                for pair in points.windows(2) {
                    painter.line_segment([pair[0], pair[1]], preview_stroke);
                }
            }
            Tool::Select => {
                let p1 = self.canvas_to_screen(sx, sy, canvas_rect);
                let p2 = self.canvas_to_screen(ex, ey, canvas_rect);
                let rect = Rect::from_two_pos(p1, p2);
                // Dashed outline for selection
                painter.rect_stroke(rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));
            }
            _ => {}
        }
    }

    fn handle_keyboard(&mut self, ctx: &Context) {
        slowcore::theme::consume_special_keys(ctx);

        // Handle dropped image files
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter()
                .filter_map(|f| f.path.clone())
                .filter(|p| {
                    let ext = p.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).unwrap_or_default();
                    matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "bmp")
                })
                .collect()
        });
        if let Some(path) = dropped.into_iter().next() {
            self.open_file(path);
        }

        ctx.input(|i| {
            let cmd = i.modifiers.command;
            if cmd && i.key_pressed(Key::N) { self.show_new_dialog = true; }
            if cmd && i.key_pressed(Key::O) { self.show_open_dialog(); }
            if cmd && i.key_pressed(Key::S) {
                if i.modifiers.shift { self.show_save_dialog(); } else { self.save(); }
            }
            if cmd && i.key_pressed(Key::Z) {
                if i.modifiers.shift { self.canvas.redo(); } else { self.canvas.undo(); }
                self.texture_dirty = true;
            }

            // Tool shortcuts
            if !cmd {
                if i.key_pressed(Key::P) { self.current_tool = Tool::Pencil; }
                if i.key_pressed(Key::B) { self.current_tool = Tool::Brush; }
                if i.key_pressed(Key::E) { self.current_tool = Tool::Eraser; }
                if i.key_pressed(Key::L) { self.current_tool = Tool::Line; }
                if i.key_pressed(Key::R) { self.current_tool = Tool::Rectangle; }
                if i.key_pressed(Key::G) { self.current_tool = Tool::Fill; }
                // X to swap black/white
                if i.key_pressed(Key::X) { self.draw_black = !self.draw_black; }
            }

            // Zoom
            if i.key_pressed(Key::Equals) || i.key_pressed(Key::Plus) {
                self.zoom = (self.zoom * 1.5).min(16.0);
            }
            if i.key_pressed(Key::Minus) {
                self.zoom = (self.zoom / 1.5).max(0.25);
            }
            if i.key_pressed(Key::Num0) {
                self.zoom = 1.0;
                self.pan_offset = Vec2::ZERO;
            }
        });
    }

    fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for tool in Tool::all() {
                let selected = self.current_tool == *tool;
                // Use SlowButton for dither highlight when selected (readable text)
                let r = ui.add(slowcore::widgets::SlowButton::new(tool.icon()).selected(selected));
                if r.on_hover_text(tool.name()).clicked() {
                    self.current_tool = *tool;
                }
            }
        });
    }

    fn render_pattern_panel(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.label("draw (x to swap):");
            // Black/White color indicator
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
            let painter = ui.painter();
            if self.draw_black {
                painter.rect_filled(rect, 0.0, SlowColors::BLACK);
            } else {
                painter.rect_filled(rect, 0.0, SlowColors::WHITE);
                painter.rect_stroke(rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));
            }
            if response.clicked() { self.draw_black = !self.draw_black; }

            ui.add_space(8.0);
            ui.label("size:");
            ui.horizontal_wrapped(|ui| {
                for size in BrushSize::all() {
                    let selected = self.brush_size == *size;
                    let r = ui.add(slowcore::widgets::SlowButton::new(&format!("{}", size.pixels())).selected(selected));
                    if r.clicked() {
                        self.brush_size = *size;
                    }
                }
            });

            ui.add_space(8.0);
            ui.label("pattern:");

            // Pattern swatches
            for pattern in Pattern::all() {
                let selected = self.fill_pattern == *pattern;
                let size = Vec2::new(48.0, 16.0);
                let (rect, response) = ui.allocate_exact_size(size, Sense::click());
                let painter = ui.painter();

                // Draw pattern preview
                painter.rect_filled(rect, 0.0, SlowColors::WHITE);
                let x0 = rect.min.x as i32;
                let y0 = rect.min.y as i32;
                let x1 = rect.max.x as i32;
                let y1 = rect.max.y as i32;
                for py in y0..y1 {
                    for px in x0..x1 {
                        if pattern.should_fill((px - x0) as u32, (py - y0) as u32) {
                            painter.rect_filled(
                                Rect::from_min_size(
                                    Pos2::new(px as f32, py as f32),
                                    Vec2::splat(1.0),
                                ),
                                0.0,
                                SlowColors::BLACK,
                            );
                        }
                    }
                }

                let stroke_w = if selected { 2.0 } else { 1.0 };
                painter.rect_stroke(rect, 0.0, Stroke::new(stroke_w, SlowColors::BLACK));

                if response.on_hover_text(pattern.name()).clicked() {
                    self.fill_pattern = *pattern;
                }
            }
        });
    }

    fn render_canvas(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        self.update_texture(ctx);

        let available = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(available, Sense::click_and_drag());

        // Background — checkerboard to show canvas bounds
        let painter = ui.painter();
        painter.rect_filled(available, 0.0, SlowColors::WHITE);

        // Canvas
        if let Some(ref texture) = self.texture {
            let canvas_size = Vec2::new(
                self.canvas.width() as f32 * self.zoom,
                self.canvas.height() as f32 * self.zoom,
            );
            let canvas_rect = Rect::from_min_size(
                available.min + self.pan_offset,
                canvas_size,
            );

            self.last_canvas_rect = Some(canvas_rect);

            painter.image(
                texture.id(),
                canvas_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                egui::Color32::WHITE,
            );

            // Canvas border
            painter.rect_stroke(canvas_rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));

            self.handle_drawing(canvas_rect, &response);

            // Draw shape preview overlay AFTER drawing handling
            self.render_shape_preview(painter, canvas_rect);
        }

        // Pan with middle mouse
        if response.dragged_by(egui::PointerButton::Middle) {
            self.pan_offset += response.drag_delta();
        }
    }

    fn render_menu_bar(&mut self, ui: &mut egui::Ui) {
        menu_bar(ui, |ui| {
            ui.menu_button("file", |ui| {
                if ui.button("new...      ⌘n").clicked() { self.show_new_dialog = true; ui.close_menu(); }
                if ui.button("open...     ⌘o").clicked() { self.show_open_dialog(); ui.close_menu(); }
                ui.separator();
                if ui.button("save        ⌘s").clicked() { self.save(); ui.close_menu(); }
                if ui.button("save as...  ⇧⌘s").clicked() { self.show_save_dialog(); ui.close_menu(); }
            });

            ui.menu_button("edit", |ui| {
                if ui.button("undo  ⌘z").clicked() { self.canvas.undo(); self.texture_dirty = true; ui.close_menu(); }
                if ui.button("redo  ⇧⌘z").clicked() { self.canvas.redo(); self.texture_dirty = true; ui.close_menu(); }
                ui.separator();
                if ui.button("clear").clicked() { self.canvas.save_undo_state(); self.canvas.clear(); self.texture_dirty = true; ui.close_menu(); }
            });

            ui.menu_button("image", |ui| {
                if ui.button("invert").clicked() { self.canvas.save_undo_state(); self.canvas.invert(); self.texture_dirty = true; ui.close_menu(); }
                if ui.button("threshold").clicked() { self.canvas.save_undo_state(); self.canvas.threshold(); self.texture_dirty = true; ui.close_menu(); }
                ui.separator();
                if ui.button("flip horizontal").clicked() { self.canvas.save_undo_state(); self.canvas.flip_horizontal(); self.texture_dirty = true; ui.close_menu(); }
                if ui.button("flip vertical").clicked() { self.canvas.save_undo_state(); self.canvas.flip_vertical(); self.texture_dirty = true; ui.close_menu(); }
            });

            ui.menu_button("view", |ui| {
                if ui.button("zoom in    +").clicked() { self.zoom = (self.zoom * 1.5).min(16.0); ui.close_menu(); }
                if ui.button("zoom out   -").clicked() { self.zoom = (self.zoom / 1.5).max(0.25); ui.close_menu(); }
                if ui.button("actual size 0").clicked() { self.zoom = 1.0; self.pan_offset = Vec2::ZERO; ui.close_menu(); }
            });

            ui.menu_button("help", |ui| {
                if ui.button("about slowPaint").clicked() { self.show_about = true; ui.close_menu(); }
            });
        });
    }

    fn render_new_dialog(&mut self, ctx: &Context) {
        egui::Window::new("new image")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("width:");
                    ui.text_edit_singleline(&mut self.new_width);
                });
                ui.horizontal(|ui| {
                    ui.label("height:");
                    ui.text_edit_singleline(&mut self.new_height);
                });
                ui.horizontal(|ui| {
                    if ui.button("cancel").clicked() { self.show_new_dialog = false; }
                    if ui.button("create").clicked() {
                        if let (Ok(w), Ok(h)) = (self.new_width.parse::<u32>(), self.new_height.parse::<u32>()) {
                            if w > 0 && w <= 4096 && h > 0 && h <= 4096 {
                                self.new_canvas(w, h);
                                self.show_new_dialog = false;
                            }
                        }
                    }
                });
            });
    }

    fn render_file_browser(&mut self, ctx: &Context) {
        let title = match self.file_browser_mode {
            FileBrowserMode::Open => "open image",
            FileBrowserMode::Save => "save image",
        };

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("location:");
                    ui.label(self.file_browser.current_dir.to_string_lossy().to_string());
                });
                ui.separator();

                egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                    let entries = self.file_browser.entries.clone();
                    for (idx, entry) in entries.iter().enumerate() {
                        let selected = self.file_browser.selected_index == Some(idx);
                        let response = ui.add(
                            slowcore::widgets::FileListItem::new(&entry.name, entry.is_directory).selected(selected)
                        );
                        if response.clicked() { self.file_browser.selected_index = Some(idx); }
                        if response.double_clicked() {
                            if entry.is_directory {
                                self.file_browser.navigate_to(entry.path.clone());
                            } else if self.file_browser_mode == FileBrowserMode::Open {
                                self.open_file(entry.path.clone());
                                self.show_file_browser = false;
                            }
                        }
                    }
                });

                if self.file_browser_mode == FileBrowserMode::Save {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("filename:");
                        ui.text_edit_singleline(&mut self.save_filename);
                    });
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("cancel").clicked() { self.show_file_browser = false; }
                    let action = if self.file_browser_mode == FileBrowserMode::Open { "open" } else { "save" };
                    if ui.button(action).clicked() {
                        match self.file_browser_mode {
                            FileBrowserMode::Open => {
                                if let Some(entry) = self.file_browser.selected_entry() {
                                    if !entry.is_directory {
                                        self.open_file(entry.path.clone());
                                        self.show_file_browser = false;
                                    }
                                }
                            }
                            FileBrowserMode::Save => {
                                if !self.save_filename.is_empty() {
                                    let path = self.file_browser.save_directory().join(&self.save_filename);
                                    self.save_as(path);
                                    self.show_file_browser = false;
                                }
                            }
                        }
                    }
                });
            });
    }

    fn render_close_confirm(&mut self, ctx: &Context) {
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
                        if !self.canvas.modified {
                            self.close_confirmed = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                });
            });
    }

    fn render_about(&mut self, ctx: &Context) {
        egui::Window::new("about slowPaint")
            .collapsible(false)
            .resizable(false)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("slowPaint");
                    ui.label("version 0.1.0");
                    ui.add_space(8.0);
                    ui.label("bitmap editor for slowOS");
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.label("supported formats:");
                ui.label("  PNG, BMP, JPEG (open/save)");
                ui.add_space(4.0);
                ui.label("frameworks:");
                ui.label("  egui/eframe (MIT), image-rs (MIT)");
                ui.label("  tiny-skia (BSD-3)");
                ui.add_space(4.0);
                ui.label("tools: pencil, brush, eraser, line,");
                ui.label("rectangle, ellipse, fill, patterns");
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    if ui.button("ok").clicked() { self.show_about = false; }
                });
            });
    }
}

impl eframe::App for SlowPaintApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.handle_keyboard(ctx);

        egui::TopBottomPanel::top("menu").show(ctx, |ui| { self.render_menu_bar(ui); });
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| { self.render_toolbar(ui); });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let pos_str = match self.hover_canvas_pos {
                Some((x, y)) => format!("{}, {}", x, y),
                None => "—".into(),
            };
            status_bar(ui, &format!(
                "{}  |  {}×{}  |  zoom: {:.0}%  |  {}  |  pos: {}",
                self.canvas.display_title(),
                self.canvas.width(),
                self.canvas.height(),
                self.zoom * 100.0,
                self.current_tool.name(),
                pos_str,
            ));
        });
        egui::SidePanel::left("patterns").exact_width(80.0).show(ctx, |ui| { self.render_pattern_panel(ui); });
        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| { self.render_canvas(ui, ctx); });

        // Request repaint during drawing for live preview
        if self.is_drawing {
            ctx.request_repaint();
        }

        if self.show_new_dialog { self.render_new_dialog(ctx); }
        if self.show_file_browser { self.render_file_browser(ctx); }
        if self.show_close_confirm { self.render_close_confirm(ctx); }
        if self.show_about { self.render_about(ctx); }

        // Handle close request
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.canvas.modified && !self.close_confirmed {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.show_close_confirm = true;
            }
        }
    }
}
