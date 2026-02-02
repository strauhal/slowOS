//! slowPics application
//!
//! Minimal image viewer for the slow computer.
//! Loads images at display resolution (max 640×480) to stay within
//! the constraints of e-ink and Raspberry Pi hardware.
//! Never modifies the original file.

use crate::loader::{self, LoadedImage};
use egui::{
    ColorImage, Context, Key, Pos2, Rect, Sense, Stroke, TextureHandle,
    TextureOptions, Vec2,
};
use slowcore::storage::{documents_dir, FileBrowser};
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::status_bar;
use std::path::PathBuf;

pub struct SlowPicsApp {
    /// Currently loaded image (display-resolution copy)
    current: Option<LoadedImage>,
    /// Texture handle for egui rendering
    texture: Option<TextureHandle>,
    /// All images in the current directory
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
    /// Loading state
    loading: bool,
}

impl SlowPicsApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, initial_path: Option<PathBuf>) -> Self {
        let extensions: Vec<String> = loader::supported_extensions()
            .iter()
            .map(|s| s.to_string())
            .collect();

        let mut app = Self {
            current: None,
            texture: None,
            siblings: Vec::new(),
            current_index: 0,
            error: None,
            show_file_browser: false,
            file_browser: FileBrowser::new(documents_dir()).with_filter(extensions),
            show_info: false,
            show_about: false,
            loading: false,
        };

        if let Some(path) = initial_path {
            app.load_image(path);
        }

        app
    }

    fn load_image(&mut self, path: PathBuf) {
        self.error = None;
        self.loading = true;

        match LoadedImage::open(&path) {
            Ok(loaded) => {
                // Update sibling list
                self.siblings = loader::sibling_images(&path);
                self.current_index = self.siblings.iter()
                    .position(|p| p == &path)
                    .unwrap_or(0);

                // Upload texture to egui
                self.texture = None; // Drop old texture
                self.current = Some(loaded);
                self.loading = false;
            }
            Err(e) => {
                self.error = Some(e.to_string());
                self.current = None;
                self.texture = None;
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
                "slowpics_image",
                color_image,
                TextureOptions::LINEAR,
            ));
        }
    }

    fn next_image(&mut self) {
        if self.siblings.is_empty() { return; }
        self.current_index = (self.current_index + 1) % self.siblings.len();
        let path = self.siblings[self.current_index].clone();
        self.texture = None;
        self.load_image(path);
    }

    fn prev_image(&mut self) {
        if self.siblings.is_empty() { return; }
        if self.current_index == 0 {
            self.current_index = self.siblings.len() - 1;
        } else {
            self.current_index -= 1;
        }
        let path = self.siblings[self.current_index].clone();
        self.texture = None;
        self.load_image(path);
    }

    fn handle_keyboard(&mut self, ctx: &Context) {
        // Consume Tab to prevent menu hover
        ctx.input_mut(|i| {
            if i.key_pressed(Key::Tab) {
                i.events.retain(|e| !matches!(e, egui::Event::Key { key: Key::Tab, .. }));
            }
        });
        ctx.input(|i| {
            let cmd = i.modifiers.command;

            if cmd && i.key_pressed(Key::O) {
                self.show_file_browser = true;
            }
            if i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::N) {
                self.next_image();
            }
            if i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::P) {
                self.prev_image();
            }
            if i.key_pressed(Key::I) {
                self.show_info = !self.show_info;
            }
            if i.key_pressed(Key::Escape) {
                if self.show_info { self.show_info = false; }
                else if self.show_file_browser { self.show_file_browser = false; }
            }
        });
    }

    fn render_menu_bar(&mut self, ui: &mut egui::Ui) {
        menu_bar(ui, |ui| {
            ui.menu_button("file", |ui| {
                if ui.button("open...  ⌘O").clicked() {
                    self.show_file_browser = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("next image   →").clicked() {
                    self.next_image();
                    ui.close_menu();
                }
                if ui.button("prev image   ←").clicked() {
                    self.prev_image();
                    ui.close_menu();
                }
            });
            ui.menu_button("view", |ui| {
                if ui.button("image info   I").clicked() {
                    self.show_info = !self.show_info;
                    ui.close_menu();
                }
            });
            ui.menu_button("help", |ui| {
                if ui.button("about slowPics").clicked() {
                    self.show_about = true;
                    ui.close_menu();
                }
            });
        });
    }

    fn render_image(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();

        if let Some(ref tex) = self.texture {
            // Center the image in the available space
            let tex_size = tex.size_vec2();
            let scale_x = rect.width() / tex_size.x;
            let scale_y = rect.height() / tex_size.y;
            let scale = scale_x.min(scale_y).min(1.0); // Don't upscale

            let display_size = Vec2::new(tex_size.x * scale, tex_size.y * scale);
            let offset = Vec2::new(
                (rect.width() - display_size.x) / 2.0,
                (rect.height() - display_size.y) / 2.0,
            );

            let img_rect = Rect::from_min_size(rect.min + offset, display_size);

            // Draw border
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, SlowColors::WHITE);
            painter.rect_stroke(img_rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));

            // Draw image
            ui.allocate_ui_at_rect(img_rect, |ui| {
                ui.image(egui::load::SizedTexture::new(tex.id(), display_size));
            });
        } else if self.current.is_none() && self.error.is_none() {
            // No image loaded — show welcome
            ui.vertical_centered(|ui| {
                ui.add_space(rect.height() / 3.0);
                ui.label("slowPics");
                ui.add_space(10.0);
                ui.label("open an image with ⌘O");
                ui.label("or drag a file onto this window");
                ui.add_space(20.0);
                ui.label("supported: PNG, JPEG, GIF, BMP, TIFF, WebP");
            });
        }

        // Show error
        if let Some(ref err) = self.error {
            ui.vertical_centered(|ui| {
                ui.add_space(rect.height() / 3.0);
                ui.label(format!("error: {}", err));
                ui.add_space(10.0);
                if ui.button("open another image").clicked() {
                    self.show_file_browser = true;
                }
            });
        }
    }

    fn render_info_panel(&mut self, ctx: &Context) {
        if let Some(ref img) = self.current {
            egui::Window::new("image info")
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
                    ui.label(format!("original: {}×{}", img.original_width, img.original_height));
                    ui.label(format!("display: {}×{}", img.display_width, img.display_height));

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
                            "image {} of {} in folder",
                            self.current_index + 1,
                            self.siblings.len()
                        ));
                    }

                    ui.add_space(8.0);
                    if ui.button("close").clicked() {
                        self.show_info = false;
                    }
                });
        }
    }

    fn render_file_browser(&mut self, ctx: &Context) {
        egui::Window::new("open image")
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
                    let entries = self.file_browser.entries.clone();
                    for (idx, entry) in entries.iter().enumerate() {
                        let selected = self.file_browser.selected_index == Some(idx);
                        let response = ui.add(
                            slowcore::widgets::FileListItem::new(&entry.name, entry.is_directory)
                                .selected(selected),
                        );

                        if response.clicked() {
                            self.file_browser.selected_index = Some(idx);
                        }

                        if response.double_clicked() {
                            if entry.is_directory {
                                self.file_browser.navigate_to(entry.path.clone());
                            } else {
                                self.load_image(entry.path.clone());
                                self.show_file_browser = false;
                            }
                        }
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
                                self.load_image(path);
                                self.show_file_browser = false;
                            }
                        }
                    }
                });
            });
    }

    fn render_about(&mut self, ctx: &Context) {
        egui::Window::new("about slowPics")
            .collapsible(false)
            .resizable(false)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("slowPics");
                    ui.label("version 0.1.0");
                    ui.add_space(8.0);
                    ui.label("minimal image viewer for e-ink");
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.label("supported formats:");
                ui.label("  PNG, JPEG, GIF, BMP, TIFF, WebP");
                ui.add_space(4.0);
                ui.label("frameworks:");
                ui.label("  egui/eframe (MIT), image-rs (MIT)");
                ui.add_space(4.0);
                ui.label("images converted to grayscale for e-ink");
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    if ui.button("ok").clicked() { self.show_about = false; }
                });
            });
    }
}

impl eframe::App for SlowPicsApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.handle_keyboard(ctx);
        self.ensure_texture(ctx);

        // Handle dropped files
        ctx.input(|i| {
            if let Some(file) = i.raw.dropped_files.first() {
                if let Some(ref path) = file.path {
                    if loader::is_image(path) {
                        // We'll load after input processing
                        // (can't borrow self mutably here)
                    }
                }
            }
        });
        // Check dropped files outside input closure
        let dropped: Option<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.first()
                .and_then(|f| f.path.clone())
                .filter(|p| loader::is_image(p))
        });
        if let Some(path) = dropped {
            self.load_image(path);
        }

        // Menu bar
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            self.render_menu_bar(ui);
        });

        // Status bar
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let status = if let Some(ref img) = self.current {
                let filename = img.path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let pos = if !self.siblings.is_empty() {
                    format!("  [{}/{}]", self.current_index + 1, self.siblings.len())
                } else {
                    String::new()
                };
                format!(
                    "{}  |  {}×{} → {}×{}  |  {}{}",
                    filename,
                    img.original_width, img.original_height,
                    img.display_width, img.display_height,
                    img.size_string(),
                    pos,
                )
            } else if self.loading {
                "loading...".to_string()
            } else {
                "no image loaded  |  ⌘O to open".to_string()
            };
            status_bar(ui, &status);
        });

        // Main content
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE))
            .show(ctx, |ui| {
                self.render_image(ui);
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
    }
}
