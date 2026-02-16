//! slowClock — a dedicated clock application for slowOS
//!
//! Features a normal view with time, date, and stopwatch/timer,
//! plus a full-screen view showing just the time in large type.

use chrono::Local;
use eframe::NativeOptions;
use egui::{Align2, CentralPanel, Context, FontId, Key, Sense, Stroke, TopBottomPanel, Vec2};
use slowcore::theme::{consume_special_keys, menu_bar, SlowColors};
use slowcore::widgets::status_bar;
use std::time::{Duration, Instant};

/// Clock view mode
#[derive(Clone, Copy, PartialEq)]
enum ViewMode {
    Normal,
    FullScreen,
}

/// Stopwatch state
#[derive(Clone, Copy, PartialEq)]
enum StopwatchState {
    Stopped,
    Running,
    Paused,
}

struct SlowClockApp {
    /// Current view mode
    view_mode: ViewMode,
    /// Use 24-hour format
    use_24h: bool,
    /// Show seconds
    show_seconds: bool,
    /// Date format: 0 = full, 1 = short, 2 = ISO
    date_format: u8,
    /// Stopwatch state
    stopwatch_state: StopwatchState,
    /// Stopwatch start time
    stopwatch_start: Instant,
    /// Accumulated stopwatch time (for pause/resume)
    stopwatch_accumulated: Duration,
    /// Show about dialog
    show_about: bool,
}

impl SlowClockApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            view_mode: ViewMode::Normal,
            use_24h: false,
            show_seconds: true,
            date_format: 0,
            stopwatch_state: StopwatchState::Stopped,
            stopwatch_start: Instant::now(),
            stopwatch_accumulated: Duration::ZERO,
            show_about: false,
        }
    }

    fn format_time(&self) -> String {
        let now = Local::now();
        match (self.use_24h, self.show_seconds) {
            (true, true) => now.format("%H:%M:%S").to_string(),
            (true, false) => now.format("%H:%M").to_string(),
            (false, true) => now.format("%l:%M:%S %p").to_string().trim_start().to_string(),
            (false, false) => now.format("%l:%M %p").to_string().trim_start().to_string(),
        }
    }

    fn format_date(&self) -> String {
        let now = Local::now();
        match self.date_format {
            0 => now.format("%A, %B %d, %Y").to_string(),
            1 => now.format("%a %b %d, %Y").to_string(),
            _ => now.format("%Y-%m-%d").to_string(),
        }
    }

    fn stopwatch_elapsed(&self) -> Duration {
        match self.stopwatch_state {
            StopwatchState::Stopped => Duration::ZERO,
            StopwatchState::Running => self.stopwatch_accumulated + self.stopwatch_start.elapsed(),
            StopwatchState::Paused => self.stopwatch_accumulated,
        }
    }

    fn format_stopwatch(&self) -> String {
        let elapsed = self.stopwatch_elapsed();
        let total_secs = elapsed.as_secs();
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        let secs = total_secs % 60;
        let centis = (elapsed.subsec_millis() / 10) as u64;

        if hours > 0 {
            format!("{:02}:{:02}:{:02}.{:02}", hours, mins, secs, centis)
        } else {
            format!("{:02}:{:02}.{:02}", mins, secs, centis)
        }
    }

    fn toggle_stopwatch(&mut self) {
        match self.stopwatch_state {
            StopwatchState::Stopped => {
                self.stopwatch_accumulated = Duration::ZERO;
                self.stopwatch_start = Instant::now();
                self.stopwatch_state = StopwatchState::Running;
            }
            StopwatchState::Running => {
                self.stopwatch_accumulated += self.stopwatch_start.elapsed();
                self.stopwatch_state = StopwatchState::Paused;
            }
            StopwatchState::Paused => {
                self.stopwatch_start = Instant::now();
                self.stopwatch_state = StopwatchState::Running;
            }
        }
    }

    fn reset_stopwatch(&mut self) {
        self.stopwatch_state = StopwatchState::Stopped;
        self.stopwatch_accumulated = Duration::ZERO;
    }

    fn draw_normal_view(&mut self, ctx: &Context) {
        // Menu bar
        TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            menu_bar(ui, |ui| {
                ui.menu_button("clock", |ui| {
                    if ui.button("full screen    ⌘F").clicked() {
                        self.view_mode = ViewMode::FullScreen;
                        ui.close_menu();
                    }
                    ui.separator();
                    let fmt_label = if self.use_24h { "12-hour format" } else { "24-hour format" };
                    if ui.button(fmt_label).clicked() {
                        self.use_24h = !self.use_24h;
                        ui.close_menu();
                    }
                    let sec_label = if self.show_seconds { "hide seconds" } else { "show seconds" };
                    if ui.button(sec_label).clicked() {
                        self.show_seconds = !self.show_seconds;
                        ui.close_menu();
                    }
                });
                ui.menu_button("help", |ui| {
                    if ui.button("about").clicked() {
                        self.show_about = true;
                        ui.close_menu();
                    }
                });
            });
        });

        // Title bar
        TopBottomPanel::top("title_bar").show(ctx, |ui| {
            slowcore::theme::SlowTheme::title_bar_frame().show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label("slowClock");
                });
            });
        });

        // Status bar
        TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            let status = if self.stopwatch_state == StopwatchState::Running {
                "stopwatch running"
            } else {
                "⌘F full screen  |  ⌘⇧F exit full screen"
            };
            status_bar(ui, status);
        });

        // Main content
        CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(16.0)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(24.0);

                    // Large time display
                    let time_str = self.format_time();
                    ui.label(
                        egui::RichText::new(&time_str)
                            .font(FontId::proportional(64.0))
                            .color(SlowColors::BLACK),
                    );

                    ui.add_space(8.0);

                    // Date (click to cycle format)
                    let date_str = self.format_date();
                    let date_response = ui.add(
                        egui::Label::new(
                            egui::RichText::new(&date_str)
                                .font(FontId::proportional(18.0))
                                .color(SlowColors::BLACK),
                        )
                        .sense(Sense::click()),
                    );
                    if date_response.clicked() {
                        self.date_format = (self.date_format + 1) % 3;
                    }

                    ui.add_space(32.0);
                    ui.separator();
                    ui.add_space(16.0);

                    // Stopwatch section
                    ui.label(
                        egui::RichText::new("stopwatch")
                            .font(FontId::proportional(14.0))
                            .color(SlowColors::BLACK),
                    );

                    ui.add_space(8.0);

                    let sw_str = self.format_stopwatch();
                    ui.label(
                        egui::RichText::new(&sw_str)
                            .font(FontId::monospace(36.0))
                            .color(SlowColors::BLACK),
                    );

                    ui.add_space(12.0);

                    ui.horizontal(|ui| {
                        let start_label = match self.stopwatch_state {
                            StopwatchState::Stopped => "start",
                            StopwatchState::Running => "pause",
                            StopwatchState::Paused => "resume",
                        };
                        if ui.button(start_label).clicked() {
                            self.toggle_stopwatch();
                        }
                        if self.stopwatch_state != StopwatchState::Stopped {
                            if ui.button("reset").clicked() {
                                self.reset_stopwatch();
                            }
                        }
                    });
                });
            });
    }

    fn draw_fullscreen_view(&mut self, ctx: &Context) {
        CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE))
            .show(ctx, |ui| {
                let available = ui.available_rect_before_wrap();

                // Click anywhere to show hint
                let response = ui.allocate_rect(available, Sense::click());

                let painter = ui.painter();
                painter.rect_filled(available, 0.0, SlowColors::WHITE);

                // Giant time display centered on screen
                let time_str = self.format_time();

                // Calculate font size to fill width (roughly)
                let char_count = time_str.len() as f32;
                let max_font_size = (available.width() / (char_count * 0.55)).min(available.height() * 0.5).min(200.0);

                painter.text(
                    available.center(),
                    Align2::CENTER_CENTER,
                    &time_str,
                    FontId::proportional(max_font_size),
                    SlowColors::BLACK,
                );

                // Date below the time
                let date_str = self.format_date();
                let date_pos = egui::Pos2::new(
                    available.center().x,
                    available.center().y + max_font_size * 0.45 + 16.0,
                );
                painter.text(
                    date_pos,
                    Align2::CENTER_TOP,
                    &date_str,
                    FontId::proportional(20.0),
                    SlowColors::BLACK,
                );

                // Hint at bottom
                let hint_pos = egui::Pos2::new(available.center().x, available.max.y - 24.0);
                painter.text(
                    hint_pos,
                    Align2::CENTER_BOTTOM,
                    "⌘⇧F to exit full screen",
                    FontId::proportional(11.0),
                    egui::Color32::from_gray(160),
                );

                // If stopwatch is running, show it below date
                if self.stopwatch_state == StopwatchState::Running {
                    let sw_str = self.format_stopwatch();
                    let sw_pos = egui::Pos2::new(
                        available.center().x,
                        date_pos.y + 32.0,
                    );
                    painter.text(
                        sw_pos,
                        Align2::CENTER_TOP,
                        &sw_str,
                        FontId::monospace(24.0),
                        SlowColors::BLACK,
                    );
                }

                if response.clicked() {
                    // Click to cycle date format
                    self.date_format = (self.date_format + 1) % 3;
                }
            });
    }

    fn draw_about(&mut self, ctx: &Context) {
        if !self.show_about {
            return;
        }
        egui::Window::new("about slowClock")
            .collapsible(false)
            .resizable(false)
            .default_width(280.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    ui.heading("slowClock");
                    ui.label("version 0.2.0");
                    ui.add_space(8.0);
                    ui.label("clock for slowOS");
                    ui.add_space(4.0);
                    ui.label("features:");
                    ui.label("  12/24 hour formats");
                    ui.label("  full-screen display");
                    ui.label("  stopwatch");
                    ui.add_space(12.0);
                    if ui.button("ok").clicked() {
                        self.show_about = false;
                    }
                    ui.add_space(4.0);
                });
            });
    }
}

impl eframe::App for SlowClockApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        consume_special_keys(ctx);

        // Keyboard shortcuts
        let toggle_fullscreen = ctx.input(|i| {
            let cmd = i.modifiers.command;
            let shift = i.modifiers.shift;
            (cmd && shift && i.key_pressed(Key::F))
                || (cmd && i.key_pressed(Key::F) && self.view_mode == ViewMode::Normal)
        });
        let escape = ctx.input(|i| i.key_pressed(Key::Escape));

        if toggle_fullscreen {
            self.view_mode = match self.view_mode {
                ViewMode::Normal => ViewMode::FullScreen,
                ViewMode::FullScreen => ViewMode::Normal,
            };
        }
        if escape && self.view_mode == ViewMode::FullScreen {
            self.view_mode = ViewMode::Normal;
        }

        // Space toggles stopwatch
        let space = ctx.input(|i| i.key_pressed(Key::Space) && !i.modifiers.command);
        if space {
            self.toggle_stopwatch();
        }

        match self.view_mode {
            ViewMode::Normal => self.draw_normal_view(ctx),
            ViewMode::FullScreen => self.draw_fullscreen_view(ctx),
        }

        self.draw_about(ctx);

        // Request repaint for clock updates
        if self.stopwatch_state == StopwatchState::Running {
            ctx.request_repaint_after(Duration::from_millis(33)); // 30fps for stopwatch
        } else {
            ctx.request_repaint_after(Duration::from_millis(500)); // update clock twice a second
        }
    }
}

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([360.0, 420.0])
        .with_title("slowClock");

    if let Some(pos) = slowcore::cascade_position() {
        viewport = viewport.with_position(pos);
    }

    let options = NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "slowClock",
        options,
        Box::new(|cc| {
            slowcore::SlowTheme::default().apply(&cc.egui_ctx);
            Box::new(SlowClockApp::new(cc))
        }),
    )
}
