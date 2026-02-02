//! SlowOS Desktop — System 6-inspired desktop environment
//!
//! Features:
//! - Dithered desktop background
//! - Menu bar with system menu, apps menu
//! - Desktop icons for each application (double-click to launch)
//! - Clock display
//! - Running app indicators
//! - About dialog with system info

use crate::process_manager::{AppInfo, ProcessManager};
use chrono::Local;
use egui::{
    Align2, Color32, Context, FontId, Key, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2,
};
use slowcore::dither;
use slowcore::theme::SlowColors;

/// Desktop icon layout
const ICON_SIZE: f32 = 64.0;
const ICON_SPACING: f32 = 80.0;
const ICON_LABEL_HEIGHT: f32 = 16.0;
const ICON_TOTAL_HEIGHT: f32 = ICON_SIZE + ICON_LABEL_HEIGHT + 8.0;
const DESKTOP_PADDING: f32 = 24.0;
const MENU_BAR_HEIGHT: f32 = 22.0;
const ICONS_PER_COLUMN: usize = 6;

/// Desktop application state
pub struct DesktopApp {
    /// Process manager for launching/tracking apps
    process_manager: ProcessManager,
    /// Currently selected icon index
    selected_icon: Option<usize>,
    /// Time of last click (for double-click detection)
    last_click_time: std::time::Instant,
    /// Index of last clicked icon (for double-click detection)
    last_click_index: Option<usize>,
    /// Show about dialog
    show_about: bool,
    /// Show shutdown dialog
    show_shutdown: bool,
    /// Status message (bottom of screen)
    status_message: String,
    /// Status message timestamp
    status_time: std::time::Instant,
    /// Frame counter for polling
    frame_count: u64,
}

impl DesktopApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            process_manager: ProcessManager::new(),
            selected_icon: None,
            last_click_time: std::time::Instant::now(),
            last_click_index: None,
            show_about: false,
            show_shutdown: false,
            status_message: String::new(),
            status_time: std::time::Instant::now(),
            frame_count: 0,
        }
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = msg.into();
        self.status_time = std::time::Instant::now();
    }

    /// Launch an app by binary name
    fn launch_app(&mut self, binary: &str) {
        match self.process_manager.launch(binary) {
            Ok(true) => {
                self.set_status(format!("launched {}", binary));
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
                    Rect::from_min_size(
                        Pos2::new(x as f32, y as f32),
                        Vec2::splat(1.0),
                    ),
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
        let total_rect = Rect::from_min_size(
            pos,
            Vec2::new(ICON_SIZE, ICON_TOTAL_HEIGHT),
        );

        let response = ui.allocate_rect(total_rect, Sense::click());
        let painter = ui.painter();
        let is_selected = self.selected_icon == Some(index);

        // Icon box
        let icon_rect = Rect::from_min_size(
            Pos2::new(
                pos.x + (ICON_SIZE - 48.0) / 2.0,
                pos.y,
            ),
            Vec2::new(48.0, 48.0),
        );

        // Draw icon background
        painter.rect_filled(icon_rect, 0.0, SlowColors::WHITE);
        painter.rect_stroke(icon_rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));

        // Running indicator: filled top-right corner
        if app.running {
            let indicator_rect = Rect::from_min_size(
                Pos2::new(icon_rect.max.x - 8.0, icon_rect.min.y),
                Vec2::new(8.0, 8.0),
            );
            painter.rect_filled(indicator_rect, 0.0, SlowColors::BLACK);
        }

        // Icon glyph
        painter.text(
            icon_rect.center(),
            Align2::CENTER_CENTER,
            &app.icon_label,
            FontId::proportional(20.0),
            SlowColors::BLACK,
        );

        // Label below icon
        let label_rect = Rect::from_min_size(
            Pos2::new(pos.x - 8.0, pos.y + ICON_SIZE + 4.0),
            Vec2::new(ICON_SIZE + 16.0, ICON_LABEL_HEIGHT),
        );

        if is_selected {
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
            painter.text(
                label_rect.center(),
                Align2::CENTER_CENTER,
                &app.display_name,
                FontId::proportional(11.0),
                SlowColors::BLACK,
            );
        }

        response
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
                    ui.menu_button("⏳ slowOS", |ui| {
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
                                format!("● {}", display_name)
                            } else {
                                format!("  {}", display_name)
                            };
                            if ui.button(label).clicked() {
                                self.launch_app(&binary);
                                ui.close_menu();
                            }
                        }
                    });

                    // Clock on the right
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            let time = Local::now().format("%H:%M").to_string();
                            ui.label(
                                egui::RichText::new(time)
                                    .font(FontId::proportional(12.0))
                                    .color(SlowColors::BLACK),
                            );
                        },
                    );
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

                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            let running = self.process_manager.running_count();
                            let text = if running == 0 {
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
                        },
                    );
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
                    ui.label(
                        egui::RichText::new("⏳")
                            .font(FontId::proportional(48.0))
                            .color(SlowColors::BLACK),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("slowOS")
                            .font(FontId::proportional(22.0))
                            .color(SlowColors::BLACK),
                    );
                    ui.label(
                        egui::RichText::new("version 0.1.0")
                            .font(FontId::proportional(12.0))
                            .color(SlowColors::BLACK),
                    );
                    ui.add_space(8.0);
                    ui.label("a minimal operating system");
                    ui.label("for focused computing");
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("by the slow computer company")
                            .font(FontId::proportional(11.0))
                            .color(SlowColors::BLACK),
                    );
                    ui.add_space(12.0);

                    // System info
                    ui.separator();
                    ui.add_space(4.0);

                    let num_apps = self.process_manager.apps().len();
                    ui.label(
                        egui::RichText::new(format!("{} applications installed", num_apps))
                            .font(FontId::proportional(11.0)),
                    );

                    let date = Local::now().format("%A, %B %d, %Y").to_string();
                    ui.label(
                        egui::RichText::new(date).font(FontId::proportional(11.0)),
                    );

                    ui.add_space(8.0);
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
            .default_width(280.0)
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
                        ui.label("shut down will close all apps.");
                    } else {
                        ui.label("are you sure you want to shut down?");
                    }
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button("cancel").clicked() {
                            self.show_shutdown = false;
                        }
                        ui.add_space(16.0);
                        if ui.button("shut down").clicked() {
                            self.process_manager.shutdown_all();
                            // On embedded, trigger system shutdown
                            if std::path::Path::new("/sbin/poweroff").exists() {
                                let _ = std::process::Command::new("/sbin/poweroff").spawn();
                            }
                            std::process::exit(0);
                        }
                    });
                    ui.add_space(4.0);
                });
            });
    }

    /// Handle keyboard shortcuts
    fn handle_keys(&mut self, ctx: &Context) {
        let input = ctx.input(|i| {
            let cmd = i.modifiers.command;
            (
                cmd && i.key_pressed(Key::Q),  // Quit
                i.key_pressed(Key::Escape),    // Deselect
            )
        });

        if input.0 {
            // Cmd+Q: show shutdown dialog
            self.show_shutdown = true;
        }
        if input.1 {
            // Escape: deselect
            self.selected_icon = None;
        }
    }
}

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Poll running processes periodically (every ~30 frames ≈ 0.5s)
        self.frame_count += 1;
        if self.frame_count % 30 == 0 {
            let exited = self.process_manager.poll();
            for binary in &exited {
                self.set_status(format!("{} has quit", binary));
            }
        }

        // Request repaint for clock and status updates
        ctx.request_repaint_after(std::time::Duration::from_secs(1));

        self.handle_keys(ctx);
        self.draw_menu_bar(ctx);
        self.draw_status_bar(ctx);

        // Main desktop area
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE))
            .show(ctx, |ui| {
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

                for (index, (binary, app)) in apps.iter().enumerate() {
                    let col = index / ICONS_PER_COLUMN;
                    let row = index % ICONS_PER_COLUMN;

                    let x = start_x - col as f32 * ICON_SPACING;
                    let y = start_y + row as f32 * (ICON_TOTAL_HEIGHT + 8.0);

                    let pos = Pos2::new(x, y);
                    let response = self.draw_icon(ui, pos, app, index);

                    if response.clicked() {
                        let now = std::time::Instant::now();
                        let double_click = self.last_click_index == Some(index)
                            && now.duration_since(self.last_click_time).as_millis() < 400;

                        if double_click {
                            // Double-click: launch app
                            self.launch_app(binary);
                            self.selected_icon = None;
                        } else {
                            // Single click: select
                            self.selected_icon = Some(index);
                        }

                        self.last_click_time = now;
                        self.last_click_index = Some(index);
                    }
                }

                // Click on empty area deselects
                let bg_response = ui.allocate_rect(available, Sense::click());
                if bg_response.clicked() && self.selected_icon.is_some() {
                    self.selected_icon = None;
                }
            });

        // Dialogs
        self.draw_about(ctx);
        self.draw_shutdown(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.process_manager.shutdown_all();
    }
}
