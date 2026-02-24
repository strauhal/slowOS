//! slowClock — a dedicated clock application for slowOS
//!
//! Features an analog clock face and a full-screen view.

use chrono::Local;
use eframe::NativeOptions;
use egui::{Align2, CentralPanel, Context, FontId, Key, Pos2, Sense, Stroke, TopBottomPanel, Vec2};
use slowcore::repaint::RepaintController;
use slowcore::theme::{consume_special_keys, menu_bar, SlowColors};
use slowcore::widgets::{status_bar, window_control_buttons, WindowAction};
use std::time::{Duration, Instant};

/// Clock view mode
#[derive(Clone, Copy, PartialEq)]
enum ViewMode {
    Analog,
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
    view_mode: ViewMode,
    use_24h: bool,
    show_seconds: bool,
    date_format: u8,
    stopwatch_state: StopwatchState,
    stopwatch_start: Instant,
    stopwatch_accumulated: Duration,
    show_about: bool,
    /// Cached formatted time string and the second it was computed for
    cached_time: (i64, String),
    /// Cached formatted date string and the day it was computed for
    cached_date: (u32, String),
    repaint: RepaintController,
}

impl SlowClockApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            view_mode: ViewMode::Analog,
            use_24h: false,
            show_seconds: true,
            date_format: 0,
            stopwatch_state: StopwatchState::Stopped,
            stopwatch_start: Instant::now(),
            stopwatch_accumulated: Duration::ZERO,
            show_about: false,
            cached_time: (-1, String::new()),
            cached_date: (0, String::new()),
            repaint: RepaintController::with_fast_interval(),
        }
    }

    fn format_time(&mut self) -> String {
        let now = Local::now();
        let sec = now.timestamp();
        if sec != self.cached_time.0 {
            self.cached_time.0 = sec;
            self.cached_time.1 = match (self.use_24h, self.show_seconds) {
                (true, true) => now.format("%H:%M:%S").to_string(),
                (true, false) => now.format("%H:%M").to_string(),
                (false, true) => now.format("%l:%M:%S %p").to_string().trim_start().to_string(),
                (false, false) => now.format("%l:%M %p").to_string().trim_start().to_string(),
            };
        }
        self.cached_time.1.clone()
    }

    fn format_date(&mut self) -> String {
        let now = Local::now();
        let day = now.format("%j").to_string().parse::<u32>().unwrap_or(0);
        if day != self.cached_date.0 {
            self.cached_date.0 = day;
            self.cached_date.1 = match self.date_format {
                0 => now.format("%A, %B %d, %Y").to_string(),
                1 => now.format("%a %b %d, %Y").to_string(),
                _ => now.format("%Y-%m-%d").to_string(),
            };
        }
        self.cached_date.1.clone()
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

    #[allow(dead_code)]
    fn reset_stopwatch(&mut self) {
        self.stopwatch_state = StopwatchState::Stopped;
        self.stopwatch_accumulated = Duration::ZERO;
    }

    /// Draw an analog clock face
    fn draw_analog_clock(&self, painter: &egui::Painter, center: Pos2, radius: f32) {
        let now = Local::now();
        let hour = now.format("%I").to_string().parse::<f32>().unwrap_or(12.0);
        let minute = now.format("%M").to_string().parse::<f32>().unwrap_or(0.0);
        let second = now.format("%S").to_string().parse::<f32>().unwrap_or(0.0);

        // Clock face outline
        painter.circle_stroke(center, radius, Stroke::new(2.0, SlowColors::BLACK));
        // Inner fill
        painter.circle_filled(center, radius - 1.0, SlowColors::WHITE);
        painter.circle_stroke(center, radius, Stroke::new(2.0, SlowColors::BLACK));

        // Hour markers
        for i in 0..12 {
            let angle = (i as f32) * std::f32::consts::TAU / 12.0 - std::f32::consts::FRAC_PI_2;
            let outer = Pos2::new(
                center.x + angle.cos() * (radius - 4.0),
                center.y + angle.sin() * (radius - 4.0),
            );
            let inner_len = if i % 3 == 0 { 14.0 } else { 8.0 };
            let inner = Pos2::new(
                center.x + angle.cos() * (radius - 4.0 - inner_len),
                center.y + angle.sin() * (radius - 4.0 - inner_len),
            );
            let thickness = if i % 3 == 0 { 2.0 } else { 1.0 };
            painter.line_segment([inner, outer], Stroke::new(thickness, SlowColors::BLACK));
        }

        // Hour numbers
        for i in 1..=12 {
            let angle = (i as f32) * std::f32::consts::TAU / 12.0 - std::f32::consts::FRAC_PI_2;
            let num_pos = Pos2::new(
                center.x + angle.cos() * (radius - 26.0),
                center.y + angle.sin() * (radius - 26.0),
            );
            painter.text(
                num_pos,
                Align2::CENTER_CENTER,
                format!("{}", i),
                FontId::proportional(14.0),
                SlowColors::BLACK,
            );
        }

        // Hour hand
        let hour_angle = (hour + minute / 60.0) * std::f32::consts::TAU / 12.0 - std::f32::consts::FRAC_PI_2;
        let hour_len = radius * 0.5;
        let hour_end = Pos2::new(
            center.x + hour_angle.cos() * hour_len,
            center.y + hour_angle.sin() * hour_len,
        );
        painter.line_segment([center, hour_end], Stroke::new(3.0, SlowColors::BLACK));

        // Minute hand
        let min_angle = (minute + second / 60.0) * std::f32::consts::TAU / 60.0 - std::f32::consts::FRAC_PI_2;
        let min_len = radius * 0.72;
        let min_end = Pos2::new(
            center.x + min_angle.cos() * min_len,
            center.y + min_angle.sin() * min_len,
        );
        painter.line_segment([center, min_end], Stroke::new(2.0, SlowColors::BLACK));

        // Second hand
        if self.show_seconds {
            let sec_angle = second * std::f32::consts::TAU / 60.0 - std::f32::consts::FRAC_PI_2;
            let sec_len = radius * 0.8;
            let sec_end = Pos2::new(
                center.x + sec_angle.cos() * sec_len,
                center.y + sec_angle.sin() * sec_len,
            );
            painter.line_segment([center, sec_end], Stroke::new(1.0, SlowColors::BLACK));
        }

        // Center dot
        painter.circle_filled(center, 4.0, SlowColors::BLACK);
    }

    fn draw_analog_view(&mut self, ctx: &Context) {
        let win_action = TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            menu_bar(ui, |ui| {
                let action = window_control_buttons(ui);
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
                action
            }).inner
        }).inner;

        match win_action {
            WindowAction::Close => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            WindowAction::Minimize => {
                slowcore::minimize::write_minimized("slowclock", "slowClock");
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            WindowAction::None => {}
        }

        TopBottomPanel::top("title_bar").show(ctx, |ui| {
            slowcore::theme::SlowTheme::title_bar_frame().show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label("slowClock");
                });
            });
        });

        TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            let status = if self.stopwatch_state == StopwatchState::Running {
                "stopwatch running  |  ⌘F full screen"
            } else {
                "⌘F full screen  |  space stopwatch"
            };
            status_bar(ui, status);
        });

        CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(8.0)))
            .show(ctx, |ui| {
                let available = ui.available_rect_before_wrap();
                let painter = ui.painter();

                // Clock face centered in available space
                let clock_radius = (available.width().min(available.height()) * 0.42).min(140.0);
                let clock_center = Pos2::new(
                    available.center().x,
                    available.min.y + clock_radius + 12.0,
                );

                self.draw_analog_clock(painter, clock_center, clock_radius);

                // Digital time below the clock face
                let time_str = self.format_time();
                let time_pos = Pos2::new(available.center().x, clock_center.y + clock_radius + 16.0);
                painter.text(
                    time_pos,
                    Align2::CENTER_TOP,
                    &time_str,
                    FontId::proportional(20.0),
                    SlowColors::BLACK,
                );

                // Date below digital time
                let date_str = self.format_date();
                let date_pos = Pos2::new(available.center().x, time_pos.y + 28.0);
                painter.text(
                    date_pos,
                    Align2::CENTER_TOP,
                    &date_str,
                    FontId::proportional(14.0),
                    SlowColors::BLACK,
                );

                // Stopwatch below date
                let sw_y = date_pos.y + 32.0;
                if self.stopwatch_state != StopwatchState::Stopped {
                    let sw_str = self.format_stopwatch();
                    painter.text(
                        Pos2::new(available.center().x, sw_y),
                        Align2::CENTER_TOP,
                        &sw_str,
                        FontId::monospace(24.0),
                        SlowColors::BLACK,
                    );
                }
            });
    }

    fn draw_fullscreen_view(&mut self, ctx: &Context) {
        CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE))
            .show(ctx, |ui| {
                let available = ui.available_rect_before_wrap();

                let response = ui.allocate_rect(available, Sense::click());

                let painter = ui.painter();
                painter.rect_filled(available, 0.0, SlowColors::WHITE);

                // Large analog clock centered on screen
                let clock_radius = (available.width().min(available.height()) * 0.40).min(300.0);
                let clock_center = available.center();

                self.draw_analog_clock(painter, clock_center, clock_radius);

                // Date below the clock
                let date_str = self.format_date();
                let date_pos = Pos2::new(
                    available.center().x,
                    clock_center.y + clock_radius + 20.0,
                );
                painter.text(
                    date_pos,
                    Align2::CENTER_TOP,
                    &date_str,
                    FontId::proportional(18.0),
                    SlowColors::BLACK,
                );

                // Stopwatch below date
                if self.stopwatch_state != StopwatchState::Stopped {
                    let sw_str = self.format_stopwatch();
                    let sw_pos = Pos2::new(
                        available.center().x,
                        date_pos.y + 28.0,
                    );
                    painter.text(
                        sw_pos,
                        Align2::CENTER_TOP,
                        &sw_str,
                        FontId::monospace(24.0),
                        SlowColors::BLACK,
                    );
                }

                let hint_pos = Pos2::new(available.center().x, available.max.y - 24.0);
                painter.text(
                    hint_pos,
                    Align2::CENTER_BOTTOM,
                    "escape to exit full screen",
                    FontId::proportional(11.0),
                    SlowColors::BLACK,
                );

                if response.clicked() {
                    self.date_format = (self.date_format + 1) % 3;
                }
            });
    }

    fn draw_about(&mut self, ctx: &Context) {
        if !self.show_about {
            return;
        }
        let screen = ctx.screen_rect();
        let max_h = (screen.height() - 40.0).max(120.0);
        let resp = egui::Window::new("about slowClock")
            .collapsible(false)
            .resizable(false)
            .default_width(280.0)
            .max_height(max_h)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    ui.heading("slowClock");
                    ui.label("version 0.2.2");
                    ui.add_space(8.0);
                    ui.label("clock for slowOS");
                    ui.add_space(4.0);
                    ui.label("features:");
                    ui.label("  analog clock face");
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
        if let Some(r) = &resp { slowcore::dither::draw_window_shadow_large(ctx, r.response.rect); }
    }
}

impl eframe::App for SlowClockApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.repaint.begin_frame(ctx);
        consume_special_keys(ctx);

        // Keyboard shortcuts
        let toggle_fullscreen = ctx.input(|i| {
            i.modifiers.command && i.key_pressed(Key::F)
        });
        let escape = ctx.input(|i| i.key_pressed(Key::Escape));

        if toggle_fullscreen {
            self.view_mode = match self.view_mode {
                ViewMode::FullScreen => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                    ViewMode::Analog
                }
                _ => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
                    ViewMode::FullScreen
                }
            };
        }
        if escape && self.view_mode == ViewMode::FullScreen {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            self.view_mode = ViewMode::Analog;
        }

        let space = ctx.input(|i| i.key_pressed(Key::Space) && !i.modifiers.command);
        if space {
            self.toggle_stopwatch();
        }

        match self.view_mode {
            ViewMode::Analog => self.draw_analog_view(ctx),
            ViewMode::FullScreen => self.draw_fullscreen_view(ctx),
        }

        self.draw_about(ctx);

        // Enable continuous repaint only for the running stopwatch.
        // Idle clock/analog face updates on next input event.
        self.repaint.set_continuous(self.stopwatch_state == StopwatchState::Running);
        self.repaint.end_frame(ctx);
    }
}

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([360.0, 500.0])
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
