//! SlowOS Desktop — System 6-inspired desktop environment
//!
//! Features:
//! - Dithered desktop background
//! - Menu bar with system menu, apps menu, date and clock
//! - Desktop icons for each application (double-click to launch)
//! - Smooth window open/close animations
//! - Running app indicators
//! - Keyboard navigation
//! - About dialog with system info

use crate::process_manager::{AppInfo, ProcessManager};
use chrono::Local;
use egui::{
    Align2, ColorImage, Context, FontId, Key, Pos2, Rect, Response, Sense, Stroke,
    TextureHandle, TextureOptions, Ui, Vec2,
};
use slowcore::animation::AnimationManager;
use slowcore::dither;
use slowcore::theme::SlowColors;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Desktop icon layout
const ICON_SIZE: f32 = 64.0;
const ICON_SPACING: f32 = 80.0;
const ICON_LABEL_HEIGHT: f32 = 16.0;
const ICON_TOTAL_HEIGHT: f32 = ICON_SIZE + ICON_LABEL_HEIGHT + 8.0;
const DESKTOP_PADDING: f32 = 24.0;
const MENU_BAR_HEIGHT: f32 = 22.0;
const ICONS_PER_COLUMN: usize = 6;

/// Double-click timing threshold in milliseconds
const DOUBLE_CLICK_MS: u128 = 400;

/// Desktop application state
pub struct DesktopApp {
    /// Process manager for launching/tracking apps
    process_manager: ProcessManager,
    /// Currently selected icon index
    selected_icon: Option<usize>,
    /// Time of last click (for double-click detection)
    last_click_time: Instant,
    /// Index of last clicked icon (for double-click detection)
    last_click_index: Option<usize>,
    /// Currently hovered icon index
    hovered_icon: Option<usize>,
    /// Show about dialog
    show_about: bool,
    /// Show shutdown dialog
    show_shutdown: bool,
    /// Status message (bottom of screen)
    status_message: String,
    /// Status message timestamp
    status_time: Instant,
    /// Frame counter for polling
    frame_count: u64,
    /// Animation manager for window open/close effects
    animations: AnimationManager,
    /// Cached icon positions for animations
    icon_rects: Vec<(String, Rect)>,
    /// Screen dimensions for animation targets
    screen_rect: Rect,
    /// Last frame time for delta calculation
    last_frame_time: Instant,
    /// Use 24-hour (military) time format
    use_24h_time: bool,
    /// Date format: 0 = "Mon Jan 15", 1 = "01/15", 2 = "15/01", 3 = "2024-01-15"
    date_format: u8,
    /// Spotlight search state
    show_search: bool,
    search_query: String,
    /// Icon textures loaded from embedded PNGs
    icon_textures: HashMap<String, TextureHandle>,
    /// Whether textures have been initialized
    icons_loaded: bool,
}

impl DesktopApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            process_manager: ProcessManager::new(),
            selected_icon: None,
            last_click_time: Instant::now(),
            last_click_index: None,
            hovered_icon: None,
            show_about: false,
            show_shutdown: false,
            status_message: "welcome to slowOS".to_string(),
            status_time: Instant::now(),
            frame_count: 0,
            animations: AnimationManager::new(),
            icon_rects: Vec::new(),
            screen_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 680.0)),
            last_frame_time: Instant::now(),
            use_24h_time: false, // Default to 12-hour AM/PM
            date_format: 0, // Default to "Mon Jan 15"
            show_search: false,
            search_query: String::new(),
            icon_textures: HashMap::new(),
            icons_loaded: false,
        }
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = msg.into();
        self.status_time = Instant::now();
    }

    /// Load embedded icon PNGs as egui textures
    fn load_icon_textures(&mut self, ctx: &Context) {
        if self.icons_loaded {
            return;
        }
        self.icons_loaded = true;

        let icons: &[(&str, &[u8])] = &[
            ("slowwrite", include_bytes!("../../icons/icons_write.png")),
            ("slowpaint", include_bytes!("../../icons/icons_paint.png")),
            ("slowreader", include_bytes!("../../icons/icons_reader.png")),
            ("slowsheets", include_bytes!("../../icons/icons_sheets_1.png")),
            ("slowchess", include_bytes!("../../icons/icons_chess.png")),
            ("slowfiles", include_bytes!("../../icons/icons_files.png")),
            ("slowmusic", include_bytes!("../../icons/icons_music.png")),
            ("slowslides", include_bytes!("../../icons/icons_slides.png")),
            ("slowtex", include_bytes!("../../icons/icons_latex.png")),
            ("trash", include_bytes!("../../icons/icons_trash.png")),
            ("slowview", include_bytes!("../../icons/icons_view.png")),
            ("credits", include_bytes!("../../icons/icons_credits.png")),
            ("slowmidi", include_bytes!("../../icons/icons_midi.png")),
            ("slowbreath", include_bytes!("../../icons/icons_breath.png")),
            ("settings", include_bytes!("../../icons/icons_settings.png")),
        ];

        for (binary, png_bytes) in icons {
            if let Ok(img) = image::load_from_memory(png_bytes) {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let color_image = ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    rgba.as_raw(),
                );
                let texture = ctx.load_texture(
                    format!("icon_{}", binary),
                    color_image,
                    TextureOptions::LINEAR,
                );
                self.icon_textures.insert(binary.to_string(), texture);
            }
        }
    }

    /// Get the icon rect for a given app binary
    fn get_icon_rect(&self, binary: &str) -> Option<Rect> {
        self.icon_rects
            .iter()
            .find(|(b, _)| b == binary)
            .map(|(_, r)| *r)
    }

    /// Calculate the target window rect for animations
    fn get_window_rect(&self) -> Rect {
        // Center of screen, standard app window size
        let center = self.screen_rect.center();
        Rect::from_center_size(center, Vec2::new(720.0, 520.0))
    }

    /// Launch an app with animation
    fn launch_app_animated(&mut self, binary: &str) {
        // Don't launch if already animating or running
        if self.animations.is_app_animating(binary) {
            return;
        }

        if self.process_manager.is_running(binary) {
            self.set_status(format!("{} is already running", binary));
            return;
        }

        // Get icon position for animation start
        if let Some(icon_rect) = self.get_icon_rect(binary) {
            let window_rect = self.get_window_rect();
            self.animations
                .start_open_to(icon_rect, window_rect, binary.to_string());
            self.set_status(format!("opening {}...", binary));
        } else {
            // Fallback: launch immediately without animation
            self.launch_app_direct(binary);
        }
    }

    /// Launch an app directly (after animation or as fallback)
    fn launch_app_direct(&mut self, binary: &str) {
        match self.process_manager.launch(binary) {
            Ok(true) => {
                self.set_status(format!("{} launched", binary));
            }
            Ok(false) => {
                self.set_status(format!("{} is already running", binary));
            }
            Err(e) => {
                self.set_status(format!("error: {}", e));
                eprintln!("[slowdesktop] launch error: {}", e);
            }
        }
    }

    /// Draw the dithered desktop background (classic Mac checkerboard)
    fn draw_background(&self, ui: &mut Ui) {
        let rect = ui.available_rect_before_wrap();
        let painter = ui.painter();

        // White base
        painter.rect_filled(rect, 0.0, SlowColors::WHITE);

        // Sparse dither pattern for that classic Mac desktop look
        let density = 3u32;
        let x0 = rect.min.x as i32;
        let y0 = rect.min.y as i32;
        let x1 = rect.max.x as i32;
        let y1 = rect.max.y as i32;

        let mut y = y0;
        while y < y1 {
            let offset = if ((y - y0) / density as i32) % 2 == 0 {
                0
            } else {
                density as i32
            };
            let mut x = x0 + offset;
            while x < x1 {
                painter.rect_filled(
                    Rect::from_min_size(Pos2::new(x as f32, y as f32), Vec2::splat(1.0)),
                    0.0,
                    SlowColors::BLACK,
                );
                x += density as i32 * 2;
            }
            y += density as i32;
        }
    }

    /// Draw a single desktop icon
    fn draw_icon(
        &self,
        ui: &mut Ui,
        pos: Pos2,
        app: &AppInfo,
        index: usize,
    ) -> Response {
        // Use a larger clickable area for easier interaction
        let total_rect =
            Rect::from_min_size(
                Pos2::new(pos.x - 8.0, pos.y),
                Vec2::new(ICON_SIZE + 16.0, ICON_TOTAL_HEIGHT + 4.0)
            );

        // Use Sense::click() for reliable click detection
        let response = ui.allocate_rect(total_rect, Sense::click());
        let painter = ui.painter();
        let is_selected = self.selected_icon == Some(index);
        let is_hovered = self.hovered_icon == Some(index) || response.hovered();
        let is_animating = self.animations.is_app_animating(&app.binary);

        // Icon box
        let icon_rect =
            Rect::from_min_size(Pos2::new(pos.x + (ICON_SIZE - 48.0) / 2.0, pos.y), Vec2::new(48.0, 48.0));

        // Draw icon background
        painter.rect_filled(icon_rect, 0.0, SlowColors::WHITE);
        painter.rect_stroke(icon_rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));

        // Hover effect: subtle dither overlay on icon
        if is_hovered && !is_selected && !is_animating {
            dither::draw_dither_hover(painter, icon_rect);
        }

        // Selected effect: dithered overlay on icon
        if is_selected && !is_animating {
            dither::draw_dither_selection(painter, icon_rect);
        }

        // Animating effect: pulsing dither
        if is_animating {
            dither::draw_dither_selection(painter, icon_rect);
        }

        // Running indicator: filled top-right corner
        if app.running {
            let indicator_rect = Rect::from_min_size(
                Pos2::new(icon_rect.max.x - 10.0, icon_rect.min.y),
                Vec2::new(10.0, 10.0),
            );
            painter.rect_filled(indicator_rect, 0.0, SlowColors::BLACK);
        }

        // Icon image or fallback glyph
        if let Some(tex) = self.icon_textures.get(&app.binary) {
            painter.image(
                tex.id(),
                icon_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        } else {
            let glyph_color = if is_selected || is_animating {
                SlowColors::WHITE
            } else {
                SlowColors::BLACK
            };
            painter.text(
                icon_rect.center(),
                Align2::CENTER_CENTER,
                &app.icon_label,
                FontId::proportional(20.0),
                glyph_color,
            );
        }

        // Label below icon
        let label_rect = Rect::from_min_size(
            Pos2::new(pos.x - 8.0, pos.y + ICON_SIZE + 4.0),
            Vec2::new(ICON_SIZE + 16.0, ICON_LABEL_HEIGHT),
        );

        if is_selected || is_animating {
            // Selected: dithered background with white text
            dither::draw_dither_selection(painter, label_rect);
            painter.text(
                label_rect.center(),
                Align2::CENTER_CENTER,
                &app.display_name,
                FontId::proportional(11.0),
                SlowColors::WHITE,
            );
        } else {
            // White background behind text for readability on dithered desktop
            painter.rect_filled(label_rect, 0.0, SlowColors::WHITE);
            painter.text(
                label_rect.center(),
                Align2::CENTER_CENTER,
                &app.display_name,
                FontId::proportional(11.0),
                SlowColors::BLACK,
            );
        }

        // Show tooltip on hover with app description
        response.clone().on_hover_text(&app.description)
    }

    /// Draw the menu bar
    fn draw_menu_bar(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("menu_bar")
            .exact_height(MENU_BAR_HEIGHT)
            .frame(
                egui::Frame::none()
                    .fill(SlowColors::WHITE)
                    .stroke(Stroke::new(1.0, SlowColors::BLACK))
                    .inner_margin(egui::Margin::symmetric(4.0, 0.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // Hourglass / system menu
                    ui.menu_button("slowOS", |ui| {
                        if ui.button("about slowOS").clicked() {
                            self.show_about = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("shut down...").clicked() {
                            self.show_shutdown = true;
                            ui.close_menu();
                        }
                    });

                    ui.separator();

                    // Apps menu
                    ui.menu_button("apps", |ui| {
                        let apps: Vec<(String, String)> = self
                            .process_manager
                            .apps()
                            .iter()
                            .map(|a| (a.binary.clone(), a.display_name.clone()))
                            .collect();
                        for (binary, display_name) in apps {
                            let running = self.process_manager.is_running(&binary);
                            let label = if running {
                                format!("{} (running)", display_name)
                            } else {
                                display_name
                            };
                            if ui.button(label).clicked() {
                                self.launch_app_animated(&binary);
                                ui.close_menu();
                            }
                        }
                    });

                    // Date, clock, and search on the right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Padding from right edge
                        ui.add_space(12.0);

                        // Search button
                        if ui.add(egui::Label::new(
                            egui::RichText::new("🔍")
                                .font(FontId::proportional(12.0))
                                .color(SlowColors::BLACK),
                        ).sense(Sense::click())).clicked() {
                            self.show_search = !self.show_search;
                            if self.show_search {
                                self.search_query.clear();
                            }
                        }

                        ui.add_space(8.0);

                        // Separator
                        ui.label(
                            egui::RichText::new("|")
                                .font(FontId::proportional(12.0))
                                .color(SlowColors::BLACK),
                        );

                        ui.add_space(8.0);

                        // Time (click to toggle format)
                        let now = Local::now();
                        let time = if self.use_24h_time {
                            now.format("%H:%M").to_string()
                        } else {
                            now.format("%l:%M %p").to_string().trim_start().to_string()
                        };
                        if ui.add(egui::Label::new(
                            egui::RichText::new(&time)
                                .font(FontId::proportional(12.0))
                                .color(SlowColors::BLACK),
                        ).sense(Sense::click())).clicked() {
                            self.use_24h_time = !self.use_24h_time;
                        }

                        ui.add_space(8.0);

                        // Separator
                        ui.label(
                            egui::RichText::new("|")
                                .font(FontId::proportional(12.0))
                                .color(SlowColors::BLACK),
                        );

                        ui.add_space(8.0);

                        // Date (click to cycle format)
                        let date = match self.date_format {
                            0 => now.format("%a %b %d").to_string(), // Mon Jan 15
                            1 => now.format("%m/%d").to_string(),    // 01/15
                            2 => now.format("%d/%m").to_string(),    // 15/01
                            _ => now.format("%Y-%m-%d").to_string(), // 2024-01-15
                        };
                        if ui.add(egui::Label::new(
                            egui::RichText::new(&date)
                                .font(FontId::proportional(12.0))
                                .color(SlowColors::BLACK),
                        ).sense(Sense::click())).clicked() {
                            self.date_format = (self.date_format + 1) % 4;
                        }
                    });
                });
            });
    }

    /// Draw the status bar at the bottom
    fn draw_status_bar(&self, ctx: &Context) {
        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(20.0)
            .frame(
                egui::Frame::none()
                    .fill(SlowColors::WHITE)
                    .stroke(Stroke::new(1.0, SlowColors::BLACK))
                    .inner_margin(egui::Margin::symmetric(8.0, 2.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Show status message if recent (last 5 seconds)
                    if self.status_time.elapsed().as_secs() < 5 {
                        ui.label(
                            egui::RichText::new(&self.status_message)
                                .font(FontId::proportional(11.0))
                                .color(SlowColors::BLACK),
                        );
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let running = self.process_manager.running_count();
                        let animating = self.animations.animation_count();

                        let text = if animating > 0 {
                            "loading...".to_string()
                        } else if running == 0 {
                            "no apps running".to_string()
                        } else if running == 1 {
                            "1 app running".to_string()
                        } else {
                            format!("{} apps running", running)
                        };
                        ui.label(
                            egui::RichText::new(text)
                                .font(FontId::proportional(11.0))
                                .color(SlowColors::BLACK),
                        );
                    });
                });
            });
    }

    /// Draw the about dialog
    fn draw_about(&mut self, ctx: &Context) {
        if !self.show_about {
            return;
        }
        egui::Window::new("about slowOS")
            .collapsible(false)
            .resizable(false)
            .default_width(320.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    ui.heading("slowOS");
                    ui.add_space(4.0);
                    ui.label("version 0.1.0");
                    ui.add_space(12.0);
                    ui.label("a minimal operating system");
                    ui.label("for focused computing");
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // System info
                    let num_apps = self.process_manager.apps().len();
                    ui.label(format!("{} applications installed", num_apps));

                    let running = self.process_manager.running_count();
                    if running > 0 {
                        ui.label(format!("{} currently running", running));
                    }

                    ui.add_space(4.0);

                    let date = Local::now().format("%A, %B %d, %Y").to_string();
                    ui.label(date);

                    ui.add_space(12.0);
                    ui.label("the slow computer company");

                    ui.add_space(12.0);
                    if ui.button("ok").clicked() {
                        self.show_about = false;
                    }
                    ui.add_space(4.0);
                });
            });
    }

    /// Draw the shutdown confirmation dialog
    fn draw_shutdown(&mut self, ctx: &Context) {
        if !self.show_shutdown {
            return;
        }
        egui::Window::new("shut down")
            .collapsible(false)
            .resizable(false)
            .default_width(320.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    let running = self.process_manager.running_count();
                    if running > 0 {
                        ui.label(format!(
                            "{} app{} still running.",
                            running,
                            if running == 1 { " is" } else { "s are" }
                        ));
                        ui.label("these will be closed.");
                    } else {
                        ui.label("choose an action:");
                    }
                    ui.add_space(12.0);
                });
                ui.horizontal(|ui| {
                    if ui.button("cancel").clicked() {
                        self.show_shutdown = false;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("shut down").clicked() {
                            self.process_manager.shutdown_all();
                            if std::path::Path::new("/sbin/poweroff").exists() {
                                let _ = std::process::Command::new("/sbin/poweroff").spawn();
                            }
                            std::process::exit(0);
                        }
                        if ui.button("restart").clicked() {
                            self.process_manager.shutdown_all();
                            // Try system reboot first (for embedded/buildroot)
                            if std::path::Path::new("/sbin/reboot").exists() {
                                let _ = std::process::Command::new("/sbin/reboot").spawn();
                            } else {
                                // Restart the desktop app itself
                                if let Ok(exe) = std::env::current_exe() {
                                    #[cfg(unix)]
                                    {
                                        use std::os::unix::process::CommandExt;
                                        // Fork a new process that's fully detached
                                        let _ = std::process::Command::new(&exe)
                                            .stdin(std::process::Stdio::null())
                                            .stdout(std::process::Stdio::null())
                                            .stderr(std::process::Stdio::null())
                                            .process_group(0)
                                            .spawn();
                                    }
                                    #[cfg(not(unix))]
                                    {
                                        let _ = std::process::Command::new(&exe).spawn();
                                    }
                                }
                            }
                            std::process::exit(0);
                        }
                    });
                });
                ui.add_space(4.0);
            });
    }

    /// Draw the spotlight search overlay
    fn draw_search(&mut self, ctx: &Context) {
        if !self.show_search {
            return;
        }

        // Anchor search window near top-right of screen
        egui::Window::new("search")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .default_width(280.0)
            .anchor(Align2::RIGHT_TOP, Vec2::new(-24.0, 4.0))
            .frame(
                egui::Frame::none()
                    .fill(SlowColors::WHITE)
                    .stroke(Stroke::new(1.0, SlowColors::BLACK))
                    .inner_margin(egui::Margin::same(8.0)),
            )
            .show(ctx, |ui| {
                // Search input
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .hint_text("search apps...")
                        .desired_width(260.0)
                );

                // Auto-focus the text field
                if r.gained_focus() || self.search_query.is_empty() {
                    r.request_focus();
                }

                let query = self.search_query.to_lowercase();
                let matches: Vec<(String, String, bool)> = self.process_manager.apps().iter()
                    .filter(|a| {
                        query.is_empty() ||
                        a.display_name.to_lowercase().contains(&query) ||
                        a.description.to_lowercase().contains(&query) ||
                        a.binary.to_lowercase().contains(&query)
                    })
                    .map(|a| (a.binary.clone(), a.display_name.clone(), a.running))
                    .collect();

                if !matches.is_empty() {
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    let mut launch_binary: Option<String> = None;

                    for (binary, display_name, running) in &matches {
                        let label = if *running {
                            format!("{} (running)", display_name)
                        } else {
                            display_name.clone()
                        };
                        if ui.selectable_label(false, &label).clicked() {
                            launch_binary = Some(binary.clone());
                        }
                    }

                    // Handle Enter to launch first match
                    let enter_pressed = ui.input(|i| i.key_pressed(Key::Enter));
                    if enter_pressed && !matches.is_empty() {
                        launch_binary = Some(matches[0].0.clone());
                    }

                    if let Some(binary) = launch_binary {
                        self.show_search = false;
                        self.search_query.clear();
                        self.launch_app_animated(&binary);
                    }
                } else if !query.is_empty() {
                    ui.add_space(4.0);
                    ui.label("no results");
                }
            });
    }

    /// Handle keyboard shortcuts
    fn handle_keys(&mut self, ctx: &Context) {
        ctx.input(|i| {
            let cmd = i.modifiers.command;

            // Cmd+Q: show shutdown dialog
            if cmd && i.key_pressed(Key::Q) {
                self.show_shutdown = true;
            }

            // Cmd+Space: toggle search
            if cmd && i.key_pressed(Key::Space) {
                self.show_search = !self.show_search;
                if self.show_search {
                    self.search_query.clear();
                }
            }

            // Escape: close search, dialogs, or deselect
            if i.key_pressed(Key::Escape) {
                if self.show_search {
                    self.show_search = false;
                    self.search_query.clear();
                } else if self.show_about {
                    self.show_about = false;
                } else if self.show_shutdown {
                    self.show_shutdown = false;
                } else {
                    self.selected_icon = None;
                }
            }

            // Arrow keys for navigation
            if i.key_pressed(Key::ArrowDown) {
                self.navigate_icons(1);
            }
            if i.key_pressed(Key::ArrowUp) {
                self.navigate_icons(-1);
            }
            if i.key_pressed(Key::ArrowLeft) {
                self.navigate_icons(ICONS_PER_COLUMN as i32);
            }
            if i.key_pressed(Key::ArrowRight) {
                self.navigate_icons(-(ICONS_PER_COLUMN as i32));
            }
        });

        // Handle Enter key launch outside of input closure
        // Shift+Enter: launch app but keep selection (for launching multiple apps)
        let (should_launch, keep_selection) = ctx.input(|i| {
            let enter = i.key_pressed(Key::Enter) && self.selected_icon.is_some();
            (enter, i.modifiers.shift)
        });

        if should_launch {
            let apps: Vec<String> = self
                .process_manager
                .apps()
                .iter()
                .map(|a| a.binary.clone())
                .collect();
            if let Some(index) = self.selected_icon {
                if let Some(binary) = apps.get(index) {
                    let binary = binary.clone();
                    if keep_selection {
                        // Shift+Enter: launch but move to next app
                        let num_apps = apps.len();
                        self.selected_icon = Some((index + 1) % num_apps);
                    } else {
                        self.selected_icon = None;
                    }
                    self.launch_app_animated(&binary);
                }
            }
        }
    }

    /// Navigate between icons with arrow keys
    fn navigate_icons(&mut self, delta: i32) {
        let app_count = self.process_manager.apps().len() as i32;
        if app_count == 0 {
            return;
        }

        let current = self.selected_icon.unwrap_or(0) as i32;
        let new_index = (current + delta).rem_euclid(app_count);
        self.selected_icon = Some(new_index as usize);
    }
}

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Load icon textures on first frame
        self.load_icon_textures(ctx);

        // Consume Tab key to prevent menu focus issues
        slowcore::theme::consume_tab_key(ctx);

        // Calculate delta time
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;

        // Update animations and get apps ready to launch
        let apps_to_launch = self.animations.update(dt);
        for binary in apps_to_launch {
            self.launch_app_direct(&binary);
        }

        // Poll running processes periodically (every ~30 frames ~ 0.5s)
        self.frame_count += 1;
        if self.frame_count % 30 == 0 {
            let exited = self.process_manager.poll();
            for binary in &exited {
                self.set_status(format!("{} has quit", binary));

                // Start close animation from center of screen to icon
                if let Some(icon_rect) = self.get_icon_rect(binary) {
                    let window_rect = self.get_window_rect();
                    self.animations.start_close(window_rect, icon_rect, binary.clone());
                }
            }
        }

        // Request repaint for animations, clock, and status updates
        if self.animations.is_animating() {
            ctx.request_repaint(); // Immediate repaint for smooth animation
        } else {
            ctx.request_repaint_after(Duration::from_secs(1));
        }

        self.handle_keys(ctx);
        self.draw_menu_bar(ctx);
        self.draw_status_bar(ctx);

        // Main desktop area
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE))
            .show(ctx, |ui| {
                // Update screen rect
                self.screen_rect = ui.available_rect_before_wrap();

                // Draw dithered background
                self.draw_background(ui);

                // Layout icons in columns from the right side (like classic Mac)
                let available = ui.available_rect_before_wrap();
                let start_x = available.max.x - DESKTOP_PADDING - ICON_SIZE;
                let start_y = available.min.y + DESKTOP_PADDING;

                let apps: Vec<(String, AppInfo)> = self
                    .process_manager
                    .apps()
                    .iter()
                    .map(|a| (a.binary.clone(), a.clone()))
                    .collect();

                // Clear and rebuild icon rects cache
                self.icon_rects.clear();

                // Track which icon was clicked this frame
                let mut clicked_icon: Option<(usize, String)> = None;
                let mut new_hovered: Option<usize> = None;

                for (index, (binary, app)) in apps.iter().enumerate() {
                    let col = index / ICONS_PER_COLUMN;
                    let row = index % ICONS_PER_COLUMN;

                    let x = start_x - col as f32 * ICON_SPACING;
                    let y = start_y + row as f32 * (ICON_TOTAL_HEIGHT + 8.0);

                    let pos = Pos2::new(x, y);
                    let response = self.draw_icon(ui, pos, app, index);

                    // Cache icon rect for animations
                    let icon_rect = Rect::from_min_size(
                        Pos2::new(pos.x + (ICON_SIZE - 48.0) / 2.0, pos.y),
                        Vec2::new(48.0, 48.0),
                    );
                    self.icon_rects.push((binary.clone(), icon_rect));

                    // Track hover
                    if response.hovered() {
                        new_hovered = Some(index);
                    }

                    // Track click - only register if this specific icon was clicked
                    if response.clicked() {
                        clicked_icon = Some((index, binary.clone()));
                    }
                }

                // Update hover state
                self.hovered_icon = new_hovered;

                // Handle icon click (separate from drawing to avoid borrow issues)
                let icon_was_clicked = if let Some((index, ref binary)) = clicked_icon {
                    let now = Instant::now();
                    let is_double_click = self.last_click_index == Some(index)
                        && now.duration_since(self.last_click_time).as_millis() < DOUBLE_CLICK_MS;

                    if is_double_click {
                        // Double-click: launch app with animation
                        self.selected_icon = None;
                        self.launch_app_animated(binary);
                    } else {
                        // Single click: select
                        self.selected_icon = Some(index);
                    }

                    self.last_click_time = now;
                    self.last_click_index = Some(index);
                    true
                } else {
                    false
                };

                // Click on empty area deselects - use pointer position check instead of interact
                if !icon_was_clicked && self.selected_icon.is_some() {
                    let pointer_clicked = ui.input(|i| i.pointer.any_click());
                    if pointer_clicked {
                        // Click was somewhere but not on an icon - deselect
                        self.selected_icon = None;
                    }
                }

                // Draw animations on top of everything
                let painter = ui.painter();
                self.animations.draw(painter);
            });

        // Dialogs
        self.draw_about(ctx);
        self.draw_shutdown(ctx);
        self.draw_search(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.process_manager.shutdown_all();
    }
}
