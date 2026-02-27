//! slowView application
//!
//! Minimal image and PDF viewer for the slow computer.
//! Loads images at display resolution (max 640x480) to stay within
//! the constraints of e-ink and Raspberry Pi hardware.

use crate::loader::{self, LoadedImage};
use egui::{
    ColorImage, Context, Key, Rect, Stroke, TextureHandle,
    TextureOptions, Vec2,
};
use slowcore::repaint::RepaintController;
use slowcore::storage::{documents_dir, FileBrowser};
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::{status_bar, window_control_buttons, WindowAction};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Undoable file operation
#[derive(Clone)]
enum UndoAction {
    /// File was moved to trash (stores original path)
    Trashed(PathBuf),
}

/// Content that can be viewed
enum ViewContent {
    Image,
    Pdf(PdfContent),
}

/// Rendered PDF content — pages are rendered via hayro (pure Rust)
struct PdfContent {
    current_page: usize,
    total_pages: usize,
    path: PathBuf,
    file_size: u64,
    /// Cached rendered page textures
    page_textures: HashMap<usize, TextureHandle>,
    /// Pages that failed to render (don't retry)
    failed_pages: HashSet<usize>,
    /// Fallback text per page (extracted via lopdf)
    page_text: HashMap<usize, String>,
    /// Raw PDF data for hayro rendering
    pdf_data: Vec<u8>,
}

pub struct SlowViewApp {
    repaint: RepaintController,
    /// Currently loaded image (display-resolution copy)
    current: Option<LoadedImage>,
    /// Texture handle for egui rendering
    texture: Option<TextureHandle>,
    /// All viewable files in the current directory
    siblings: Vec<PathBuf>,
    /// Current index within siblings
    current_index: usize,
    /// Error message from last load attempt
    error: Option<String>,
    /// File browser dialog
    show_file_browser: bool,
    file_browser: FileBrowser,
    /// Info panel
    show_info: bool,
    /// About dialog
    show_about: bool,
    /// Keyboard shortcuts dialog
    show_shortcuts: bool,
    /// Loading state
    loading: bool,
    /// Current view content type
    view_content: Option<ViewContent>,
    /// Zoom level (1.0 = fit to window)
    zoom: f32,
    /// Previous zoom for calculating scroll adjustment
    prev_zoom: f32,
    /// Scroll offset for centering (relative to center, 0.5 = centered)
    scroll_center: Vec2,
    /// Undo stack for file operations
    undo_stack: Vec<UndoAction>,
    /// Fullscreen mode
    fullscreen: bool,
    /// Show menu bar temporarily in fullscreen when cursor near top
    fullscreen_menu_visible: bool,
}

impl SlowViewApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, initial_path: Option<PathBuf>) -> Self {
        let mut extensions: Vec<String> = loader::supported_extensions()
            .iter()
            .map(|s| s.to_string())
            .collect();
        extensions.push("pdf".to_string());

        let mut app = Self {
            repaint: RepaintController::new(),
            current: None,
            texture: None,
            siblings: Vec::new(),
            current_index: 0,
            error: None,
            show_file_browser: false,
            file_browser: FileBrowser::new(documents_dir()).with_filter(extensions),
            show_info: false,
            show_about: false,
            show_shortcuts: false,
            loading: false,
            view_content: None,
            zoom: 1.0,
            prev_zoom: 1.0,
            scroll_center: Vec2::new(0.5, 0.5),
            undo_stack: Vec::new(),
            fullscreen: false,
            fullscreen_menu_visible: false,
        };

        if let Some(path) = initial_path {
            app.open_file(path);
        }

        app
    }

    fn is_pdf(path: &PathBuf) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase() == "pdf")
            .unwrap_or(false)
    }

    fn open_file(&mut self, path: PathBuf) {
        self.zoom = 1.0;
        self.prev_zoom = 1.0;
        self.scroll_center = Vec2::new(0.5, 0.5);
        if Self::is_pdf(&path) {
            self.load_pdf(path);
        } else {
            self.load_image(path);
        }
    }

    fn load_pdf(&mut self, path: PathBuf) {
        self.error = None;
        self.loading = true;
        self.current = None;
        self.texture = None;

        let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        // Load PDF with hayro (pure Rust PDF renderer)
        match std::fs::read(&path) {
            Ok(data) => {
                use hayro::hayro_syntax::Pdf;
                use std::sync::Arc;
                let arc_data: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(data.clone());
                match Pdf::new(arc_data) {
                    Ok(pdf) => {
                        let total_pages = pdf.pages().len();

                        self.siblings = sibling_viewable_files(&path);
                        self.current_index = self.siblings.iter()
                            .position(|p| p == &path)
                            .unwrap_or(0);

                        self.view_content = Some(ViewContent::Pdf(PdfContent {
                            current_page: 0,
                            total_pages,
                            path,
                            file_size,
                            page_textures: HashMap::new(),
                            failed_pages: HashSet::new(),
                            page_text: HashMap::new(),
                            pdf_data: data,
                        }));
                        self.loading = false;
                    }
                    Err(e) => {
                        self.error = Some(format!("PDF error: {:?}", e));
                        self.view_content = None;
                        self.loading = false;
                    }
                }
            }
            Err(e) => {
                self.error = Some(format!("File read error: {}", e));
                self.view_content = None;
                self.loading = false;
            }
        }
    }

    /// Render a single PDF page to a texture using hayro (pure Rust)
    fn ensure_pdf_page_texture(&mut self, ctx: &Context, page: usize) {
        if let Some(ViewContent::Pdf(ref mut pdf)) = self.view_content {
            if pdf.page_textures.contains_key(&page) || pdf.failed_pages.contains(&page) {
                return;
            }

            let mut rendered = false;

            // Re-parse PDF (hayro doesn't store parsed state across borrows)
            use hayro::hayro_syntax::Pdf;
            use hayro::hayro_interpret::InterpreterSettings;
            use hayro::RenderSettings;
            use std::sync::Arc;

            let arc_data: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(pdf.pdf_data.clone());
            if let Ok(pdf_doc) = Pdf::new(arc_data) {
                if let Some(pdf_page) = pdf_doc.pages().get(page) {
                    // Render at 150 DPI scale
                    let scale = 150.0 / 72.0; // 72 is standard PDF points per inch

                    let interpreter_settings = InterpreterSettings::default();
                    let render_settings = RenderSettings {
                        x_scale: scale,
                        y_scale: scale,
                        ..Default::default()
                    };

                    let pixmap = hayro::render(pdf_page, &interpreter_settings, &render_settings);
                    let width = pixmap.width() as usize;
                    let height = pixmap.height() as usize;

                    // Convert pixmap to PNG then load as image
                    if let Ok(png_data) = pixmap.into_png() {
                        if let Ok(img) = image::load_from_memory(&png_data) {
                            // Convert to grayscale for e-ink display
                            let grey = img.grayscale();
                            let mut rgba = grey.to_rgba8();
                            let (mut w, mut h) = rgba.dimensions();

                            // Limit texture size to GPU maximum (16384 pixels)
                            const MAX_TEXTURE_SIZE: u32 = 16384;
                            if w > MAX_TEXTURE_SIZE || h > MAX_TEXTURE_SIZE {
                                let scale = (MAX_TEXTURE_SIZE as f32 / w.max(h) as f32).min(1.0);
                                let new_w = (w as f32 * scale) as u32;
                                let new_h = (h as f32 * scale) as u32;
                                rgba = image::imageops::resize(&rgba, new_w, new_h, image::imageops::FilterType::Nearest);
                                w = new_w;
                                h = new_h;
                            }

                            let color_image = ColorImage::from_rgba_unmultiplied(
                                [w as usize, h as usize],
                                rgba.as_raw(),
                            );
                            let texture = ctx.load_texture(
                                format!("pdf_page_{}", page),
                                color_image,
                                TextureOptions::NEAREST,
                            );
                            pdf.page_textures.insert(page, texture);
                            rendered = true;
                        }
                    } else {
                        // Try to get raw pixel data directly if PNG encoding fails
                        // (This shouldn't happen, but just in case)
                        let _ = width;
                        let _ = height;
                    }
                }
            }

            // If hayro rendering failed, try text extraction as fallback
            if !rendered {
                pdf.failed_pages.insert(page);
                let page_num = (page + 1) as u32;
                if let Ok(doc) = lopdf::Document::load(&pdf.path) {
                    let text = doc.extract_text(&[page_num])
                        .unwrap_or_else(|_| format!("[could not render page {}]", page_num));
                    pdf.page_text.insert(page, text);
                }
            }
        }
    }

    fn load_image(&mut self, path: PathBuf) {
        self.error = None;
        self.loading = true;
        self.view_content = None;

        match LoadedImage::open(&path) {
            Ok(loaded) => {
                // Update sibling list
                self.siblings = sibling_viewable_files(&path);
                self.current_index = self.siblings.iter()
                    .position(|p| p == &path)
                    .unwrap_or(0);

                // Upload texture to egui
                self.texture = None; // Drop old texture
                self.current = Some(loaded);
                self.view_content = Some(ViewContent::Image);
                self.loading = false;
            }
            Err(e) => {
                self.error = Some(e.to_string());
                self.current = None;
                self.texture = None;
                self.view_content = None;
                self.loading = false;
            }
        }
    }

    fn ensure_texture(&mut self, ctx: &Context) {
        if self.texture.is_some() || self.current.is_none() {
            return;
        }

        if let Some(ref img) = self.current {
            let rgba = img.rgba_bytes();
            let color_image = ColorImage::from_rgba_unmultiplied(
                [img.display_width as usize, img.display_height as usize],
                &rgba,
            );
            self.texture = Some(ctx.load_texture(
                "slowview_image",
                color_image,
                TextureOptions::NEAREST,
            ));
        }
    }

    fn next_file(&mut self) {
        if self.siblings.is_empty() { return; }
        self.current_index = (self.current_index + 1) % self.siblings.len();
        let path = self.siblings[self.current_index].clone();
        self.texture = None;
        self.open_file(path);
    }

    fn prev_file(&mut self) {
        if self.siblings.is_empty() { return; }
        if self.current_index == 0 {
            self.current_index = self.siblings.len() - 1;
        } else {
            self.current_index -= 1;
        }
        let path = self.siblings[self.current_index].clone();
        self.texture = None;
        self.open_file(path);
    }

    fn zoom_in(&mut self) { self.zoom = (self.zoom + 0.25).min(5.0); }
    fn zoom_out(&mut self) { self.zoom = (self.zoom - 0.25).max(0.25); }
    fn zoom_reset(&mut self) {
        self.zoom = 1.0;
        self.prev_zoom = 1.0;
        self.scroll_center = Vec2::new(0.5, 0.5);
    }

    fn delete_current(&mut self) {
        let path = match &self.current {
            Some(img) => img.path.clone(),
            None => {
                if let Some(ViewContent::Pdf(pdf)) = &self.view_content {
                    pdf.path.clone()
                } else {
                    return;
                }
            }
        };

        // Try to move to trash
        if trash::move_to_trash(&path).is_ok() {
            // Add to undo stack
            self.undo_stack.push(UndoAction::Trashed(path.clone()));

            // Remove from siblings list
            if let Some(idx) = self.siblings.iter().position(|p| *p == path) {
                self.siblings.remove(idx);
                // Adjust current_index
                if self.siblings.is_empty() {
                    // No more files
                    self.current = None;
                    self.texture = None;
                    self.view_content = None;
                    self.error = Some("No more files to view".into());
                } else {
                    // Navigate to next file (or prev if at end)
                    self.current_index = self.current_index.min(self.siblings.len() - 1);
                    let next_path = self.siblings[self.current_index].clone();
                    self.texture = None;
                    self.open_file(next_path);
                }
            }
        }
    }

    /// Undo the last file operation
    fn undo_last(&mut self) {
        if let Some(action) = self.undo_stack.pop() {
            match action {
                UndoAction::Trashed(original_path) => {
                    if trash::restore_from_trash(&original_path).is_ok() {
                        // Re-add to siblings and open
                        self.siblings.push(original_path.clone());
                        self.siblings.sort();
                        if let Some(idx) = self.siblings.iter().position(|p| *p == original_path) {
                            self.current_index = idx;
                        }
                        self.texture = None;
                        self.open_file(original_path);
                    }
                }
            }
        }
    }

    fn handle_keyboard(&mut self, ctx: &Context) {
        slowcore::theme::consume_special_keys(ctx);

        // Check if we're viewing a PDF (arrow keys navigate pages, not files)
        let is_pdf = matches!(self.view_content, Some(ViewContent::Pdf(_)));

        ctx.input(|i| {
            let cmd = i.modifiers.command;
            let shift = i.modifiers.shift;

            if cmd && i.key_pressed(Key::O) {
                self.show_file_browser = true;
            }
            if i.key_pressed(Key::I) {
                self.show_info = !self.show_info;
            }
            if i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals) {
                self.zoom_in();
            }
            if i.key_pressed(Key::Minus) {
                self.zoom_out();
            }
            if i.key_pressed(Key::Num0) {
                self.zoom_reset();
            }
            // Fullscreen toggle with F key
            if i.key_pressed(Key::F) {
                self.fullscreen = !self.fullscreen;
            }
            if i.key_pressed(Key::Escape) {
                if self.fullscreen { self.fullscreen = false; }
                else if self.show_info { self.show_info = false; }
                else if self.show_file_browser { self.show_file_browser = false; }
            }
            // Delete current file (move to trash)
            if i.key_pressed(Key::Backspace) || i.key_pressed(Key::Delete) {
                self.delete_current();
            }
            // Undo with Cmd+Z
            if cmd && i.key_pressed(Key::Z) {
                self.undo_last();
            }

            // Spacebar/arrows for scrolling within content
            // Spacebar: jump to bottom (or top with shift)
            if i.key_pressed(Key::Space) {
                if shift {
                    self.scroll_center.y = 0.0; // Top
                } else {
                    self.scroll_center.y = 1.0; // Bottom
                }
            }
            // Up/Down arrows for vertical scroll
            if i.key_pressed(Key::ArrowUp) {
                self.scroll_center.y = (self.scroll_center.y - 0.25).max(0.0);
            }
            if i.key_pressed(Key::ArrowDown) {
                self.scroll_center.y = (self.scroll_center.y + 0.25).min(1.0);
            }
        });

        // Apply OS-level fullscreen
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));

        // Left/Right arrow key navigation (for page changes)
        let (left, right) = ctx.input(|i| {
            (i.key_pressed(Key::ArrowLeft), i.key_pressed(Key::ArrowRight))
        });

        if is_pdf {
            // PDF mode: left/right arrows navigate pages within the PDF
            if let Some(ViewContent::Pdf(ref mut pdf)) = self.view_content {
                if left && pdf.current_page > 0 {
                    pdf.current_page -= 1;
                    self.scroll_center.y = 0.0; // Reset to top of new page
                }
                if right && pdf.current_page + 1 < pdf.total_pages {
                    pdf.current_page += 1;
                    self.scroll_center.y = 0.0; // Reset to top of new page
                }
            }
        } else {
            // Image mode: arrow keys navigate between files
            if left { self.prev_file(); }
            if right { self.next_file(); }
        }
    }

    fn render_menu_bar(&mut self, ui: &mut egui::Ui) -> WindowAction {
        let mut action = WindowAction::None;
        menu_bar(ui, |ui| {
            action = window_control_buttons(ui);
            ui.menu_button("file", |ui| {
                if ui.button("open...  ⌘O").clicked() {
                    self.show_file_browser = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("next file    →").clicked() {
                    self.next_file();
                    ui.close_menu();
                }
                if ui.button("prev file    ←").clicked() {
                    self.prev_file();
                    ui.close_menu();
                }
                ui.separator();
                let has_file = self.current.is_some() || matches!(self.view_content, Some(ViewContent::Pdf(_)));
                if ui.add_enabled(has_file, egui::Button::new("move to trash  ⌫")).clicked() {
                    self.delete_current();
                    ui.close_menu();
                }
            });
            ui.menu_button("edit", |ui| {
                let can_undo = !self.undo_stack.is_empty();
                if ui.add_enabled(can_undo, egui::Button::new("undo          ⌘Z")).clicked() {
                    self.undo_last();
                    ui.close_menu();
                }
            });
            ui.menu_button("view", |ui| {
                let fullscreen_label = if self.fullscreen { "exit fullscreen  F" } else { "fullscreen       F" };
                if ui.button(fullscreen_label).clicked() {
                    self.fullscreen = !self.fullscreen;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("zoom in      +").clicked() {
                    self.zoom_in();
                    ui.close_menu();
                }
                if ui.button("zoom out     -").clicked() {
                    self.zoom_out();
                    ui.close_menu();
                }
                if ui.button("reset zoom   0").clicked() {
                    self.zoom_reset();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("file info    I").clicked() {
                    self.show_info = !self.show_info;
                    ui.close_menu();
                }
            });
            ui.menu_button("help", |ui| {
                if ui.button("keyboard shortcuts").clicked() {
                    self.show_shortcuts = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("about").clicked() {
                    self.show_about = true;
                    ui.close_menu();
                }
            });
        });
        action
    }

    fn render_content(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();

        match &self.view_content {
            Some(ViewContent::Image) => self.render_image(ui, rect),
            Some(ViewContent::Pdf(_)) => self.render_pdf(ui, rect),
            None => {
                if self.error.is_none() {
                    // No file loaded — show welcome
                    ui.vertical_centered(|ui| {
                        ui.add_space(rect.height() / 3.0);
                        ui.label("slowView");
                        ui.add_space(10.0);
                        ui.label("open a file with ⌘O");
                        ui.label("or drag a file onto this window");
                        ui.add_space(20.0);
                        ui.label("supported: PNG, JPEG, GIF, BMP, TIFF, WebP, PDF");
                    });
                }
            }
        }

        // Show error
        if let Some(ref err) = self.error {
            ui.vertical_centered(|ui| {
                ui.add_space(rect.height() / 3.0);
                ui.label(format!("error: {}", err));
                ui.add_space(10.0);
                if ui.button("open another file").clicked() {
                    self.show_file_browser = true;
                }
            });
        }
    }

    fn render_image(&mut self, ui: &mut egui::Ui, rect: Rect) {
        if let Some(ref tex) = self.texture {
            let tex_size = tex.size_vec2();
            let fit_scale_x = rect.width() / tex_size.x;
            let fit_scale_y = rect.height() / tex_size.y;
            let fit_scale = fit_scale_x.min(fit_scale_y).min(1.0);
            let scale = fit_scale * self.zoom;

            let display_size = Vec2::new(tex_size.x * scale, tex_size.y * scale);

            // White background
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, SlowColors::WHITE);

            // Calculate scroll offset for center-based zooming
            let content_size = display_size;
            let view_size = Vec2::new(rect.width(), rect.height());

            // Calculate the scroll offset to center on scroll_center
            let max_scroll = Vec2::new(
                (content_size.x - view_size.x).max(0.0),
                (content_size.y - view_size.y).max(0.0),
            );

            // When zoom changes, adjust scroll_center to maintain the same view center
            if self.zoom != self.prev_zoom && self.prev_zoom > 0.0 {
                // Keep the same relative center point
                // scroll_center stays the same, representing which part of the image we're viewing
                self.prev_zoom = self.zoom;
            }

            // Calculate actual scroll offset from scroll_center
            let scroll_offset = Vec2::new(
                max_scroll.x * self.scroll_center.x,
                max_scroll.y * self.scroll_center.y,
            );

            // Use scroll area only when content is larger than view
            let needs_scroll = content_size.x > view_size.x || content_size.y > view_size.y;
            let scroll_response = egui::ScrollArea::both()
                .max_width(rect.width())
                .max_height(rect.height())
                .scroll_offset(scroll_offset)
                .scroll_bar_visibility(if needs_scroll {
                    egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded
                } else {
                    egui::scroll_area::ScrollBarVisibility::AlwaysHidden
                })
                .show(ui, |ui| {
                    // Add padding to center small images
                    let padding = Vec2::new(
                        ((view_size.x - content_size.x) / 2.0).max(0.0),
                        ((view_size.y - content_size.y) / 2.0).max(0.0),
                    );

                    if padding.x > 0.0 || padding.y > 0.0 {
                        ui.add_space(padding.y);
                        ui.horizontal(|ui| {
                            ui.add_space(padding.x);
                            let (img_rect, _) = ui.allocate_exact_size(display_size, egui::Sense::drag());
                            let painter = ui.painter();
                            painter.rect_stroke(img_rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));
                            painter.image(
                                tex.id(),
                                img_rect,
                                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                egui::Color32::WHITE,
                            );
                            ui.add_space(padding.x);
                        });
                        ui.add_space(padding.y);
                    } else {
                        let (img_rect, _) = ui.allocate_exact_size(display_size, egui::Sense::drag());
                        let painter = ui.painter();
                        painter.rect_stroke(img_rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));
                        painter.image(
                            tex.id(),
                            img_rect,
                            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }
                });

            // Update scroll_center based on user scrolling
            let new_offset = scroll_response.state.offset;
            if max_scroll.x > 0.0 {
                self.scroll_center.x = new_offset.x / max_scroll.x;
            }
            if max_scroll.y > 0.0 {
                self.scroll_center.y = new_offset.y / max_scroll.y;
            }
        }
    }

    fn render_pdf(&mut self, ui: &mut egui::Ui, rect: Rect) {
        if let Some(ViewContent::Pdf(ref mut pdf)) = self.view_content {
            // Page navigation header
            ui.horizontal(|ui| {
                if ui.add_enabled(pdf.current_page > 0, egui::Button::new("◀ prev")).clicked() {
                    pdf.current_page -= 1;
                }
                ui.label(format!("page {} of {}", pdf.current_page + 1, pdf.total_pages));
                if ui.add_enabled(pdf.current_page + 1 < pdf.total_pages, egui::Button::new("next ▶")).clicked() {
                    pdf.current_page += 1;
                }
            });
            ui.separator();

            // Rendered page image
            let page = pdf.current_page;
            let zoom = self.zoom;
            let scroll_center_y = self.scroll_center.y;
            if let Some(tex) = pdf.page_textures.get(&page) {
                let available = ui.available_rect_before_wrap();
                let tex_size = tex.size_vec2();
                let fit_scale_x = available.width() / tex_size.x;
                let fit_scale_y = available.height() / tex_size.y;
                let fit_scale = fit_scale_x.min(fit_scale_y).min(1.0);
                let scale = fit_scale * zoom;
                let display_size = Vec2::new(tex_size.x * scale, tex_size.y * scale);

                // Check if content needs scrolling
                let needs_scroll = display_size.y > available.height() || display_size.x > available.width();

                if needs_scroll {
                    // Calculate scroll offset from scroll_center
                    let max_scroll_y = (display_size.y - available.height()).max(0.0);
                    let scroll_offset = Vec2::new(0.0, max_scroll_y * scroll_center_y);

                    let scroll_response = egui::ScrollArea::both()
                        .scroll_offset(scroll_offset)
                        .show(ui, |ui| {
                            let padding = Vec2::new(
                                (available.width() - display_size.x).max(0.0) / 2.0,
                                0.0,
                            );
                            ui.horizontal(|ui| {
                                ui.add_space(padding.x);
                                let (img_rect, _) = ui.allocate_exact_size(display_size, egui::Sense::drag());
                                let painter = ui.painter();
                                painter.image(
                                    tex.id(),
                                    img_rect,
                                    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                    egui::Color32::WHITE,
                                );
                                ui.add_space(padding.x);
                            });
                        });

                    // Update scroll_center from user scroll
                    let new_offset = scroll_response.state.offset;
                    if max_scroll_y > 0.0 {
                        self.scroll_center.y = new_offset.y / max_scroll_y;
                    }
                } else {
                    let offset = Vec2::new(
                        (available.width() - display_size.x) / 2.0,
                        (available.height() - display_size.y) / 2.0,
                    );
                    let img_rect = Rect::from_min_size(available.min + offset, display_size);

                    let _alloc = ui.allocate_rect(available, egui::Sense::hover());
                    let painter = ui.painter_at(available);
                    painter.rect_filled(available, 0.0, SlowColors::WHITE);
                    painter.image(
                        tex.id(),
                        img_rect,
                        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
            } else if let Some(text) = pdf.page_text.get(&page) {
                // Fallback: show extracted text when rendering failed
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label(text);
                });
            } else {
                // Texture not yet rendered — show loading text
                ui.vertical_centered(|ui| {
                    ui.add_space(rect.height() / 3.0);
                    ui.label("rendering page...");
                });
            }
        }
    }

    fn render_info_panel(&mut self, ctx: &Context) {
        match &self.view_content {
            Some(ViewContent::Image) => {
                if let Some(ref img) = self.current {
                    let resp = egui::Window::new("file info")
                        .collapsible(false)
                        .resizable(false)
                        .default_width(280.0)
                        .show(ctx, |ui| {
                            let filename = img.path.file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| "unknown".to_string());

                            ui.label(format!("file: {}", filename));
                            ui.label(format!("format: {}", img.format));
                            ui.label(format!("size: {}", img.size_string()));
                            ui.separator();
                            ui.label(format!("original: {}x{}", img.original_width, img.original_height));
                            ui.label(format!("display: {}x{}", img.display_width, img.display_height));

                            if img.original_width != img.display_width || img.original_height != img.display_height {
                                let scale = img.display_width as f64 / img.original_width as f64 * 100.0;
                                ui.label(format!("scale: {:.1}%", scale));
                            }

                            ui.separator();
                            let dir = img.path.parent()
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_default();
                            ui.label(format!("location: {}", dir));

                            if !self.siblings.is_empty() {
                                ui.label(format!(
                                    "file {} of {} in folder",
                                    self.current_index + 1,
                                    self.siblings.len()
                                ));
                            }

                            ui.add_space(8.0);
                            if ui.button("close").clicked() {
                                self.show_info = false;
                            }
                        });
                    if let Some(r) = &resp { slowcore::dither::draw_window_shadow(ctx, r.response.rect); }
                }
            }
            Some(ViewContent::Pdf(ref pdf)) => {
                let resp = egui::Window::new("file info")
                    .collapsible(false)
                    .resizable(false)
                    .default_width(280.0)
                    .show(ctx, |ui| {
                        let filename = pdf.path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "unknown".to_string());

                        ui.label(format!("file: {}", filename));
                        ui.label("format: PDF");
                        ui.label(format!("size: {}", format_size(pdf.file_size)));
                        ui.separator();
                        ui.label(format!("pages: {}", pdf.total_pages));
                        ui.label(format!("current page: {}", pdf.current_page + 1));

                        ui.separator();
                        let dir = pdf.path.parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                        ui.label(format!("location: {}", dir));

                        if !self.siblings.is_empty() {
                            ui.label(format!(
                                "file {} of {} in folder",
                                self.current_index + 1,
                                self.siblings.len()
                            ));
                        }

                        ui.add_space(8.0);
                        if ui.button("close").clicked() {
                            self.show_info = false;
                        }
                    });
                if let Some(r) = &resp { slowcore::dither::draw_window_shadow(ctx, r.response.rect); }
            }
            None => {}
        }
    }

    fn render_file_browser(&mut self, ctx: &Context) {
        let resp = egui::Window::new("open file")
            .collapsible(false)
            .resizable(false)
            .default_width(450.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("location:");
                    ui.label(self.file_browser.current_dir.to_string_lossy().to_string());
                });

                ui.separator();

                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    let mut clicked_idx = None;
                    let mut nav_path = None;
                    let mut open_path = None;
                    for (idx, entry) in self.file_browser.entries.iter().enumerate() {
                        let selected = self.file_browser.selected_index == Some(idx);
                        let response = ui.add(
                            slowcore::widgets::FileListItem::new(&entry.name, entry.is_directory)
                                .selected(selected),
                        );

                        if response.clicked() {
                            clicked_idx = Some(idx);
                        }

                        if response.double_clicked() {
                            if entry.is_directory {
                                nav_path = Some(entry.path.clone());
                            } else {
                                open_path = Some(entry.path.clone());
                            }
                        }
                    }
                    if let Some(idx) = clicked_idx { self.file_browser.selected_index = Some(idx); }
                    if let Some(path) = nav_path { self.file_browser.navigate_to(path); }
                    if let Some(path) = open_path {
                        self.open_file(path);
                        self.show_file_browser = false;
                    }
                });

                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button("cancel").clicked() {
                        self.show_file_browser = false;
                    }
                    if ui.button("open").clicked() {
                        if let Some(entry) = self.file_browser.selected_entry() {
                            if !entry.is_directory {
                                let path = entry.path.clone();
                                self.open_file(path);
                                self.show_file_browser = false;
                            }
                        }
                    }
                });
            });
        if let Some(r) = &resp { slowcore::dither::draw_window_shadow(ctx, r.response.rect); }
    }

    fn render_shortcuts(&mut self, ctx: &Context) {
        let screen = ctx.screen_rect();
        let resp = egui::Window::new("keyboard shortcuts")
            .collapsible(false)
            .resizable(false)
            .default_width(280.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(screen.height() - 80.0)
                    .show(ui, |ui| {
                    let shortcut = |ui: &mut egui::Ui, key: &str, desc: &str| {
                        ui.horizontal(|ui| {
                            ui.monospace(format!("{:<14}", key));
                            ui.label(desc);
                        });
                    };

                    ui.strong("navigation");
                    shortcut(ui, "← / →", "prev / next file");
                    shortcut(ui, "↑ / ↓", "scroll up / down");
                    shortcut(ui, "Space", "jump to bottom");
                    shortcut(ui, "Shift+Space", "jump to top");

                    ui.add_space(6.0);
                    ui.strong("view");
                    shortcut(ui, "+ / =", "zoom in");
                    shortcut(ui, "-", "zoom out");
                    shortcut(ui, "0", "reset zoom");
                    shortcut(ui, "F", "fullscreen");
                    shortcut(ui, "I", "file info");

                    ui.add_space(6.0);
                    ui.strong("file");
                    shortcut(ui, "⌘O", "open file");
                    shortcut(ui, "⌫ / Delete", "move to trash");
                    shortcut(ui, "⌘Z", "undo trash");
                });

                ui.separator();
                if ui.button("close").clicked() {
                    self.show_shortcuts = false;
                }
            });
        if let Some(r) = &resp { slowcore::dither::draw_window_shadow(ctx, r.response.rect); }
    }

    fn render_about(&mut self, ctx: &Context) {
        let screen = ctx.screen_rect();
        let max_h = (screen.height() - 60.0).max(120.0);

        let resp = egui::Window::new("about slowView")
            .collapsible(false)
            .resizable(false)
            .default_width(300.0)
            .max_height(max_h)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(max_h - 50.0).show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("slowView");
                        ui.label("version 0.2.2");
                        ui.add_space(8.0);
                        ui.label("image and PDF viewer for slowOS");
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label("supported formats:");
                    ui.label("  PNG, JPEG, GIF, BMP, TIFF, WebP, PDF");
                    ui.add_space(4.0);
                    ui.label("frameworks:");
                    ui.label("  egui/eframe (MIT), image-rs (MIT)");
                    ui.label("  hayro (Apache-2.0/MIT)");
                    ui.add_space(8.0);
                });
                ui.vertical_centered(|ui| {
                    if ui.button("ok").clicked() { self.show_about = false; }
                });
            });
        if let Some(r) = &resp { slowcore::dither::draw_window_shadow_large(ctx, r.response.rect); }
    }
}

impl eframe::App for SlowViewApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.repaint.begin_frame(ctx);
        self.handle_keyboard(ctx);
        self.ensure_texture(ctx);

        // Render current PDF page if needed
        if let Some(ViewContent::Pdf(ref pdf)) = self.view_content {
            let page = pdf.current_page;
            if !pdf.page_textures.contains_key(&page) {
                self.ensure_pdf_page_texture(ctx, page);
            }
        }

        // Handle dropped files (from OS or from Files app)
        let mut dropped: Option<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.first()
                .and_then(|f| f.path.clone())
                .filter(|p| loader::is_image(p) || Self::is_pdf(p))
        });

        // Check for files dragged from slowOS Files app
        let mouse_released = ctx.input(|i| i.pointer.primary_released());
        let mouse_in_window = ctx.input(|i| i.pointer.has_pointer());
        if dropped.is_none() && mouse_released && mouse_in_window {
            if let Some(paths) = slowcore::drag::get_drag_paths() {
                // Take the first supported file
                let valid = paths.into_iter()
                    .find(|p| loader::is_image(p) || Self::is_pdf(p));
                if valid.is_some() {
                    dropped = valid;
                    slowcore::drag::end_drag();
                }
            }
        }

        if let Some(path) = dropped {
            self.open_file(path);
        }

        // Menu bar: always visible in normal mode, hover-to-show in fullscreen
        if self.fullscreen {
            let near_top = ctx.input(|i| {
                i.pointer.hover_pos().map_or(false, |p| p.y < 40.0)
            });
            self.fullscreen_menu_visible = near_top;
        }
        let mut win_action = WindowAction::None;
        if !self.fullscreen || self.fullscreen_menu_visible {
            egui::TopBottomPanel::top("menu").show(ctx, |ui| {
                win_action = self.render_menu_bar(ui);
            });
        }
        match win_action {
            WindowAction::Close => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            WindowAction::Minimize => {
                let title = self.current.as_ref()
                    .and_then(|img| img.path.file_name())
                    .map(|n| format!("{} — slowView", n.to_string_lossy()))
                    .unwrap_or_else(|| "slowView".to_string());
                slowcore::minimize::write_minimized("slowview", &title);
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            WindowAction::None => {}
        }

        // Status bar (hidden in fullscreen)
        if !self.fullscreen {
            egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let status = match &self.view_content {
                Some(ViewContent::Image) => {
                    if let Some(ref img) = self.current {
                        let filename = img.path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let pos = if !self.siblings.is_empty() {
                            format!("  [{}/{}]", self.current_index + 1, self.siblings.len())
                        } else {
                            String::new()
                        };
                        format!(
                            "{}  |  {}x{} -> {}x{}  |  {}{}",
                            filename,
                            img.original_width, img.original_height,
                            img.display_width, img.display_height,
                            img.size_string(),
                            pos,
                        )
                    } else {
                        "no file loaded".to_string()
                    }
                }
                Some(ViewContent::Pdf(ref pdf)) => {
                    let filename = pdf.path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let pos = if !self.siblings.is_empty() {
                        format!("  [{}/{}]", self.current_index + 1, self.siblings.len())
                    } else {
                        String::new()
                    };
                    format!(
                        "{}  |  page {}/{}  |  {}{}",
                        filename,
                        pdf.current_page + 1,
                        pdf.total_pages,
                        format_size(pdf.file_size),
                        pos,
                    )
                }
                None if self.loading => "loading...".to_string(),
                None => "no file loaded  |  ⌘O to open".to_string(),
            };
            status_bar(ui, &status);
            });
        }

        // Main content
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE))
            .show(ctx, |ui| {
                self.render_content(ui);
            });

        // Dialogs
        if self.show_file_browser {
            self.render_file_browser(ctx);
        }
        if self.show_info {
            self.render_info_panel(ctx);
        }
        if self.show_about {
            self.render_about(ctx);
        }
        if self.show_shortcuts {
            self.render_shortcuts(ctx);
        }
        self.repaint.end_frame(ctx);
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 { format!("{} B", bytes) }
    else if bytes < 1024 * 1024 { format!("{:.1} KB", bytes as f64 / 1024.0) }
    else if bytes < 1024 * 1024 * 1024 { format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0)) }
    else { format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0)) }
}

/// Check if a path is a viewable file (image or PDF)
fn is_viewable(path: &std::path::Path) -> bool {
    loader::is_image(path) || path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase() == "pdf")
        .unwrap_or(false)
}

/// List all viewable files in the same directory
fn sibling_viewable_files(path: &std::path::Path) -> Vec<PathBuf> {
    let parent = match path.parent() {
        Some(p) => p,
        None => return vec![path.to_path_buf()],
    };

    let mut files: Vec<PathBuf> = std::fs::read_dir(parent)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| is_viewable(p))
                .collect()
        })
        .unwrap_or_default();

    files.sort();
    files
}
