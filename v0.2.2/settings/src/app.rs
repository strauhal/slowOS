//! Settings application for slowOS

use chrono::Local;
use egui::{ColorImage, Context, Rect, Sense, Stroke, TextureHandle, TextureOptions, Vec2};
use serde::{Deserialize, Serialize};
use slowcore::repaint::RepaintController;
use slowcore::storage::config_dir;
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::{status_bar, window_control_buttons, WindowAction};
use std::collections::HashMap;
use std::path::PathBuf;

/// Get the path to the fun_icons folder
fn fun_icons_dir() -> PathBuf {
    // Look for icons/fun_icons in parent directories (for development)
    let mut path = std::env::current_exe().unwrap_or_default();
    for _ in 0..5 {
        path = path.parent().unwrap_or(&path).to_path_buf();
        let icons_path = path.join("icons").join("fun_icons");
        if icons_path.exists() {
            return icons_path;
        }
    }
    // Fallback - relative to current dir
    PathBuf::from("icons/fun_icons")
}

/// System settings that are persisted
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemSettings {
    /// Mouse sensitivity (1-10)
    pub mouse_sensitivity: u8,
    /// Double click speed in milliseconds (200-800)
    pub double_click_ms: u32,
    /// Cursor blink rate in milliseconds (0 = no blink, 200-1000)
    pub cursor_blink_ms: u32,
    /// 24-hour time format
    pub use_24h_time: bool,
    /// Show seconds in clock
    pub show_seconds: bool,
    /// Date format: 0 = "Jan 1, 2024", 1 = "1/1/2024", 2 = "2024-01-01"
    pub date_format: u8,
    /// Sound enabled
    pub sound_enabled: bool,
    /// System volume (0-100)
    pub volume: u8,
    /// User's display name
    #[serde(default)]
    pub user_name: String,
    /// User's selected icon filename (from fun_icons folder)
    #[serde(default)]
    pub user_icon: String,
}

impl Default for SystemSettings {
    fn default() -> Self {
        Self {
            mouse_sensitivity: 5,
            double_click_ms: 400,
            cursor_blink_ms: 500,
            use_24h_time: true,
            show_seconds: false,
            date_format: 0,
            sound_enabled: true,
            volume: 80,
            user_name: String::new(),
            user_icon: String::new(),
        }
    }
}

impl SystemSettings {
    fn config_path() -> PathBuf {
        config_dir("slowos").join("settings.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

/// Settings categories
#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsPane {
    Profile,
    DateTime,
    Mouse,
    Display,
    Sound,
    About,
}

pub struct SettingsApp {
    settings: SystemSettings,
    current_pane: SettingsPane,
    modified: bool,
    /// Cached icon textures (keyed by filename)
    icon_textures: HashMap<String, TextureHandle>,
    /// Available icon files from fun_icons folder
    available_icons: Vec<String>,
    repaint: RepaintController,
}

impl SettingsApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Scan for available icons
        let icons_dir = fun_icons_dir();
        let mut available_icons: Vec<String> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&icons_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("png") {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        available_icons.push(name.to_string());
                    }
                }
            }
        }
        available_icons.sort();

        Self {
            settings: SystemSettings::load(),
            current_pane: SettingsPane::Profile,
            modified: false,
            icon_textures: HashMap::new(),
            available_icons,
            repaint: RepaintController::new(),
        }
    }

    fn save_settings(&mut self) {
        self.settings.save();
        self.modified = false;
    }

    /// Draw a custom 0.0–1.0 slider bar. Returns Some(new_value) if changed.
    fn draw_slider(ui: &mut egui::Ui, fill_pct: f32, label: &str) -> Option<f32> {
        let desired = egui::vec2(200.0, 20.0);
        let (rect, response) = ui.allocate_exact_size(desired, Sense::click_and_drag());
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            painter.rect_filled(rect, 0.0, SlowColors::WHITE);
            painter.rect_stroke(rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));
            let fill_w = rect.width() * fill_pct;
            let fill_rect = Rect::from_min_size(rect.min, egui::vec2(fill_w, rect.height()));
            painter.rect_filled(fill_rect, 0.0, SlowColors::BLACK);
            let text_color = if fill_pct > 0.5 { SlowColors::WHITE } else { SlowColors::BLACK };
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(11.0), text_color);
        }
        if response.clicked() || response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                return Some(((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0));
            }
        }
        None
    }

    fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.add_space(10.0);
            ui.heading("settings");
            ui.add_space(10.0);

            let panes = [
                (SettingsPane::Profile, "profile"),
                (SettingsPane::DateTime, "date & time"),
                (SettingsPane::Mouse, "mouse"),
                (SettingsPane::Display, "display"),
                (SettingsPane::Sound, "sound"),
                (SettingsPane::About, "about"),
            ];

            for (pane, label) in panes {
                let selected = self.current_pane == pane;
                let text = if selected {
                    format!("> {}", label)
                } else {
                    format!("  {}", label)
                };

                if ui.selectable_label(selected, text).clicked() {
                    self.current_pane = pane;
                }
            }

            ui.add_space(20.0);

            if self.modified {
                if ui.button("save changes").clicked() {
                    self.save_settings();
                }
                ui.add_space(5.0);
                ui.label("(unsaved changes)");
            }
        });
    }

    fn render_datetime(&mut self, ui: &mut egui::Ui) {
        ui.heading("date & time");
        ui.add_space(10.0);

        // Current time display
        let now = Local::now();
        let time_str = if self.settings.use_24h_time {
            if self.settings.show_seconds {
                now.format("%H:%M:%S").to_string()
            } else {
                now.format("%H:%M").to_string()
            }
        } else {
            if self.settings.show_seconds {
                now.format("%I:%M:%S %p").to_string()
            } else {
                now.format("%I:%M %p").to_string()
            }
        };

        let date_str = match self.settings.date_format {
            0 => now.format("%b %d, %Y").to_string(),
            1 => now.format("%m/%d/%Y").to_string(),
            _ => now.format("%Y-%m-%d").to_string(),
        };

        ui.group(|ui| {
            ui.label("current time:");
            ui.heading(&time_str);
            ui.label(&date_str);
        });

        ui.add_space(15.0);

        // Time format settings
        ui.group(|ui| {
            ui.strong("time format");
            ui.add_space(5.0);

            if ui.checkbox(&mut self.settings.use_24h_time, "use 24-hour time").changed() {
                self.modified = true;
            }

            if ui.checkbox(&mut self.settings.show_seconds, "show seconds").changed() {
                self.modified = true;
            }
        });

        ui.add_space(10.0);

        // Date format settings
        ui.group(|ui| {
            ui.strong("date format");
            ui.add_space(5.0);

            let formats = ["January 1, 2024", "1/1/2024", "2024-01-01"];
            for (i, format) in formats.iter().enumerate() {
                if ui.radio_value(&mut self.settings.date_format, i as u8, *format).changed() {
                    self.modified = true;
                }
            }
        });

        ui.add_space(15.0);
        ui.label("note: date and time are read from the system clock.");
    }

    fn render_mouse(&mut self, ui: &mut egui::Ui) {
        ui.heading("mouse");
        ui.add_space(10.0);

        // Mouse sensitivity
        ui.group(|ui| {
            ui.strong("tracking speed");
            ui.add_space(5.0);

            ui.horizontal(|ui| {
                ui.label("slow");
                let mut sens = self.settings.mouse_sensitivity as i32;
                if ui.add(egui::Slider::new(&mut sens, 1..=10).show_value(false)).changed() {
                    self.settings.mouse_sensitivity = sens as u8;
                    self.modified = true;
                }
                ui.label("fast");
            });

            ui.label(format!("current: {}", self.settings.mouse_sensitivity));
        });

        ui.add_space(15.0);

        // Double click speed
        ui.group(|ui| {
            ui.strong("double-click speed");
            ui.add_space(5.0);

            let val = (self.settings.double_click_ms as f32 - 200.0) / 600.0;
            if let Some(new_val) = Self::draw_slider(ui, val, &format!("{}ms", self.settings.double_click_ms)) {
                self.settings.double_click_ms = (200.0 + new_val * 600.0) as u32;
                self.modified = true;
            }

            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.label("fast");
                ui.add_space(160.0);
                ui.label("slow");
            });
        });

        ui.add_space(15.0);
        ui.label("note: these settings affect system behavior.");
    }

    fn render_display(&mut self, ui: &mut egui::Ui) {
        ui.heading("display");
        ui.add_space(10.0);

        // Cursor blink rate
        ui.group(|ui| {
            ui.strong("cursor blink rate");
            ui.add_space(5.0);

            ui.horizontal(|ui| {
                ui.label("off");
                let mut blink = self.settings.cursor_blink_ms as i32;
                if ui.add(egui::Slider::new(&mut blink, 0..=1000).show_value(false)).changed() {
                    self.settings.cursor_blink_ms = blink as u32;
                    self.modified = true;
                }
                ui.label("slow");
            });

            let desc = if self.settings.cursor_blink_ms == 0 {
                "cursor does not blink".to_string()
            } else {
                format!("blink every {}ms", self.settings.cursor_blink_ms)
            };
            ui.label(desc);
        });

        ui.add_space(15.0);
    }

    fn render_sound(&mut self, ui: &mut egui::Ui) {
        ui.heading("sound");
        ui.add_space(10.0);

        // Sound enabled
        ui.group(|ui| {
            if ui.checkbox(&mut self.settings.sound_enabled, "enable system sounds").changed() {
                self.modified = true;
            }
        });

        ui.add_space(15.0);

        // Volume
        ui.group(|ui| {
            ui.strong("volume");
            ui.add_space(5.0);

            ui.add_enabled_ui(self.settings.sound_enabled, |ui| {
                let val = self.settings.volume as f32 / 100.0;
                if let Some(new_val) = Self::draw_slider(ui, val, &format!("{}%", self.settings.volume)) {
                    self.settings.volume = (new_val * 100.0) as u8;
                    self.modified = true;
                }
            });
        });

        ui.add_space(15.0);
        ui.label("note: volume affects all slowOS applications.");
    }

    fn render_about(&self, ui: &mut egui::Ui) {
        ui.heading("about slowOS");
        ui.add_space(10.0);

        ui.group(|ui| {
            ui.heading("slowOS");
            ui.label("version 0.2.2");
            ui.add_space(10.0);
            ui.label("a minimal operating system");
            ui.label("for focused computing");
        });

        ui.add_space(15.0);

        ui.group(|ui| {
            ui.strong("system information");
            ui.add_space(5.0);

            if let Ok(hostname) = std::env::var("HOSTNAME") {
                ui.label(format!("hostname: {}", hostname));
            }

            #[cfg(target_os = "linux")]
            ui.label("platform: linux");
            #[cfg(target_os = "macos")]
            ui.label("platform: macOS");
            #[cfg(target_os = "windows")]
            ui.label("platform: windows");

            ui.label(format!("rust version: {}", env!("CARGO_PKG_VERSION")));
        });

        ui.add_space(15.0);

        ui.group(|ui| {
            ui.strong("the slow computer company");
            ui.add_space(5.0);
            ui.label("slowOS is open source software");
            ui.label("licensed under the MIT license");
        });
    }

    fn render_content(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        match self.current_pane {
            SettingsPane::Profile => self.render_profile(ui, ctx),
            SettingsPane::DateTime => self.render_datetime(ui),
            SettingsPane::Mouse => self.render_mouse(ui),
            SettingsPane::Display => self.render_display(ui),
            SettingsPane::Sound => self.render_sound(ui),
            SettingsPane::About => self.render_about(ui),
        }
    }

    fn render_profile(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        ui.heading("profile");
        ui.add_space(10.0);

        // User name
        ui.group(|ui| {
            ui.strong("your name");
            ui.add_space(5.0);
            if ui.text_edit_singleline(&mut self.settings.user_name).changed() {
                self.modified = true;
            }
            ui.add_space(5.0);
            ui.label("this name is displayed in the system menu.");
        });

        ui.add_space(15.0);

        // Icon selection
        ui.group(|ui| {
            ui.strong("choose your icon");
            ui.add_space(10.0);

            // Display current icon if set
            if !self.settings.user_icon.is_empty() {
                ui.horizontal(|ui| {
                    ui.label("current icon:");
                    self.render_icon(ui, ctx, &self.settings.user_icon.clone(), 48.0, false);
                });
                ui.add_space(10.0);
            }

            // Icon grid - 5 per row
            let icons = self.available_icons.clone();
            let icons_per_row = 5;
            let icon_size = 48.0;

            for chunk in icons.chunks(icons_per_row) {
                ui.horizontal(|ui| {
                    for icon_name in chunk {
                        let is_selected = self.settings.user_icon == *icon_name;
                        if self.render_icon(ui, ctx, icon_name, icon_size, is_selected) {
                            self.settings.user_icon = icon_name.clone();
                            self.modified = true;
                        }
                    }
                });
            }
        });
    }

    /// Render an icon, returns true if clicked
    fn render_icon(&mut self, ui: &mut egui::Ui, ctx: &Context, icon_name: &str, size: f32, selected: bool) -> bool {
        // Load texture if not cached
        if !self.icon_textures.contains_key(icon_name) {
            let icon_path = fun_icons_dir().join(icon_name);
            if let Ok(bytes) = std::fs::read(&icon_path) {
                if let Ok(img) = image::load_from_memory(&bytes) {
                    let rgba = img.to_rgba8();
                    let (w, h) = (rgba.width(), rgba.height());
                    let color_image = ColorImage::from_rgba_unmultiplied(
                        [w as usize, h as usize],
                        rgba.as_raw(),
                    );
                    let texture = ctx.load_texture(
                        format!("icon_{}", icon_name),
                        color_image,
                        TextureOptions::NEAREST,
                    );
                    self.icon_textures.insert(icon_name.to_string(), texture);
                }
            }
        }

        // Render the icon
        let response = if let Some(texture) = self.icon_textures.get(icon_name) {
            let rect = ui.allocate_exact_size(Vec2::new(size + 8.0, size + 8.0), Sense::click());
            if ui.is_rect_visible(rect.0) {
                let painter = ui.painter();

                // Draw selection border if selected
                if selected {
                    painter.rect_stroke(rect.0, 0.0, Stroke::new(2.0, SlowColors::BLACK));
                }

                // Draw the icon centered in the rect
                let icon_rect = Rect::from_center_size(
                    rect.0.center(),
                    Vec2::new(size, size),
                );
                painter.image(
                    texture.id(),
                    icon_rect,
                    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
            rect.1
        } else {
            // Placeholder if image not loaded
            let (rect, response) = ui.allocate_exact_size(Vec2::new(size + 8.0, size + 8.0), Sense::click());
            if ui.is_rect_visible(rect) {
                let painter = ui.painter();
                painter.rect_stroke(rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, "?", egui::FontId::proportional(14.0), SlowColors::BLACK);
            }
            response
        };

        response.clicked()
    }
}

impl eframe::App for SettingsApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.repaint.begin_frame(ctx);
        slowcore::theme::consume_special_keys(ctx);

        // Menu bar
        let mut win_action = WindowAction::None;
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            menu_bar(ui, |ui| {
                win_action = window_control_buttons(ui);
                ui.menu_button("file", |ui| {
                    if ui.button("save").clicked() {
                        self.save_settings();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("reset to defaults").clicked() {
                        self.settings = SystemSettings::default();
                        self.modified = true;
                        ui.close_menu();
                    }
                });
                ui.menu_button("help", |ui| {
                    if ui.button("about").clicked() {
                        self.current_pane = SettingsPane::About;
                        ui.close_menu();
                    }
                });
            });
        });
        match win_action {
            WindowAction::Close => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            WindowAction::Minimize => {
                slowcore::minimize::write_minimized("settings", "settings");
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            WindowAction::None => {}
        }

        // Status bar
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let status = if self.modified {
                "settings modified - click 'save changes' to apply"
            } else {
                "settings"
            };
            status_bar(ui, status);
        });

        // Sidebar
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .default_width(150.0)
            .show(ctx, |ui| {
                self.render_sidebar(ui);
            });

        // Main content
        let ctx_clone = ctx.clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(20.0)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.render_content(ui, &ctx_clone);
                });
            });
        self.repaint.end_frame(ctx);
    }
}
