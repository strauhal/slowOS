//! slowBreath - Mindful breathing timer
//!
//! A simple app to guide slow, deep breathing for relaxation and focus.

use egui::{Context, Key, Pos2, Stroke};
use slowcore::repaint::RepaintController;
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::{status_bar, window_control_buttons, WindowAction};
use std::time::Instant;

/// Breathing phase
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Inhale,
    Hold,
    Exhale,
    Rest,
}

impl Phase {
    fn name(&self) -> &'static str {
        match self {
            Phase::Inhale => "breathe in",
            Phase::Hold => "hold",
            Phase::Exhale => "breathe out",
            Phase::Rest => "rest",
        }
    }

    fn next(&self) -> Phase {
        match self {
            Phase::Inhale => Phase::Hold,
            Phase::Hold => Phase::Exhale,
            Phase::Exhale => Phase::Rest,
            Phase::Rest => Phase::Inhale,
        }
    }
}

/// Breathing pattern (durations in seconds)
#[derive(Clone)]
struct BreathPattern {
    name: String,
    inhale: f32,
    hold: f32,
    exhale: f32,
    rest: f32,
}

impl BreathPattern {
    fn get_duration(&self, phase: Phase) -> f32 {
        match phase {
            Phase::Inhale => self.inhale,
            Phase::Hold => self.hold,
            Phase::Exhale => self.exhale,
            Phase::Rest => self.rest,
        }
    }

    fn total_cycle(&self) -> f32 {
        self.inhale + self.hold + self.exhale + self.rest
    }
}

fn default_patterns() -> Vec<BreathPattern> {
    vec![
        BreathPattern {
            name: "relaxing 4-7-8".into(),
            inhale: 4.0,
            hold: 7.0,
            exhale: 8.0,
            rest: 0.0,
        },
        BreathPattern {
            name: "box breathing".into(),
            inhale: 4.0,
            hold: 4.0,
            exhale: 4.0,
            rest: 4.0,
        },
        BreathPattern {
            name: "slow deep".into(),
            inhale: 5.0,
            hold: 2.0,
            exhale: 6.0,
            rest: 1.0,
        },
        BreathPattern {
            name: "calming".into(),
            inhale: 4.0,
            hold: 0.0,
            exhale: 6.0,
            rest: 2.0,
        },
    ]
}

pub struct SlowBreathApp {
    patterns: Vec<BreathPattern>,
    selected_pattern: usize,
    running: bool,
    phase: Phase,
    phase_elapsed: f32,
    total_breaths: u32,
    session_start: Option<Instant>,
    last_update: Instant,
    show_about: bool,
    repaint: RepaintController,
}

impl SlowBreathApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            patterns: default_patterns(),
            selected_pattern: 0,
            running: false,
            phase: Phase::Inhale,
            phase_elapsed: 0.0,
            total_breaths: 0,
            session_start: None,
            last_update: Instant::now(),
            show_about: false,
            repaint: RepaintController::with_fast_interval(),
        }
    }

    fn current_pattern(&self) -> &BreathPattern {
        &self.patterns[self.selected_pattern]
    }

    fn phase_duration(&self) -> f32 {
        self.current_pattern().get_duration(self.phase)
    }

    fn phase_progress(&self) -> f32 {
        let duration = self.phase_duration();
        if duration <= 0.0 {
            1.0
        } else {
            (self.phase_elapsed / duration).min(1.0)
        }
    }

    fn start(&mut self) {
        self.running = true;
        self.phase = Phase::Inhale;
        self.phase_elapsed = 0.0;
        self.total_breaths = 0;
        self.session_start = Some(Instant::now());
    }

    fn stop(&mut self) {
        self.running = false;
        self.session_start = None;
    }

    fn toggle(&mut self) {
        if self.running {
            self.stop();
        } else {
            self.start();
        }
    }

    fn update_breathing(&mut self, dt: f32) {
        if !self.running {
            return;
        }

        self.phase_elapsed += dt;

        // Check if phase is complete
        let duration = self.phase_duration();
        if self.phase_elapsed >= duration {
            self.phase_elapsed = 0.0;
            let old_phase = self.phase;
            self.phase = self.phase.next();

            // Skip zero-duration phases
            while self.phase_duration() <= 0.0 && self.phase != Phase::Inhale {
                self.phase = self.phase.next();
            }

            // Count completed breath cycles
            if old_phase == Phase::Exhale || old_phase == Phase::Rest {
                if self.phase == Phase::Inhale {
                    self.total_breaths += 1;
                }
            }
        }
    }

    fn session_duration(&self) -> f32 {
        self.session_start
            .map(|start| start.elapsed().as_secs_f32())
            .unwrap_or(0.0)
    }

}

impl eframe::App for SlowBreathApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.repaint.begin_frame(ctx);
        if slowcore::minimize::check_restore_signal("slowbreath") {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        // Consume special keys
        slowcore::theme::consume_special_keys(ctx);

        // Calculate delta time
        let now = Instant::now();
        let dt = now.duration_since(self.last_update).as_secs_f32();
        self.last_update = now;

        // Update breathing
        self.update_breathing(dt);

        // Enable continuous repaint while breathing animation is running
        self.repaint.set_continuous(self.running);

        // Handle keyboard and mouse
        ctx.input(|i| {
            if i.key_pressed(Key::Space) {
                self.toggle();
            }
            if i.key_pressed(Key::Escape) && self.running {
                self.stop();
            }
        });
        // Mouse click anywhere in the central area to start/stop
        let clicked = ctx.input(|i| i.pointer.any_click());
        if clicked {
            // Only toggle if no menu/dialog is consuming clicks
            let pointer_pos = ctx.input(|i| i.pointer.interact_pos());
            if let Some(pos) = pointer_pos {
                // Check if click is in the main content area (below menu, above status)
                let screen = ctx.screen_rect();
                let content_top = screen.min.y + 30.0;
                let content_bottom = screen.max.y - 25.0;
                if pos.y > content_top && pos.y < content_bottom && !self.show_about {
                    self.toggle();
                }
            }
        }

        // Menu bar
        let mut win_action = WindowAction::None;
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            menu_bar(ui, |ui| {
                win_action = window_control_buttons(ui);
                ui.menu_button("file", |ui| {
                    if ui.button("start      space").clicked() {
                        self.start();
                        ui.close_menu();
                    }
                    if ui.button("stop       esc").clicked() {
                        self.stop();
                        ui.close_menu();
                    }
                });

                ui.menu_button("pattern", |ui| {
                    let pattern_names: Vec<_> = self.patterns.iter()
                        .map(|p| p.name.clone())
                        .collect();
                    let mut new_selection = None;
                    for (idx, name) in pattern_names.iter().enumerate() {
                        let selected = idx == self.selected_pattern;
                        let label = if selected {
                            format!("* {}", name)
                        } else {
                            format!("  {}", name)
                        };
                        if ui.button(&label).clicked() {
                            new_selection = Some(idx);
                            ui.close_menu();
                        }
                    }
                    if let Some(idx) = new_selection {
                        self.selected_pattern = idx;
                        if self.running {
                            self.start();
                        }
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
        match win_action {
            WindowAction::Close => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            WindowAction::Minimize => {
                slowcore::minimize::write_minimized("slowbreath", "slowBreath");
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            WindowAction::None => {}
        }

        // Status bar
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let pattern = self.current_pattern();
            let cycle_time = pattern.total_cycle();
            let status = if self.running {
                let session = self.session_duration();
                let mins = (session / 60.0) as u32;
                let secs = (session % 60.0) as u32;
                format!(
                    "{}  |  {} breaths  |  {}:{:02}",
                    pattern.name, self.total_breaths, mins, secs
                )
            } else {
                format!("{}  |  {:.0}s cycle", pattern.name, cycle_time)
            };
            status_bar(ui, &status);
        });

        // Main content
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE))
            .show(ctx, |ui| {
                let full_rect = ui.available_rect_before_wrap();
                ui.allocate_rect(full_rect, egui::Sense::hover());

                let painter = ui.painter();
                let center_x = full_rect.center().x;

                // Calculate circle dimensions first
                let circle_area_size = full_rect.width().min(full_rect.height() - 160.0);
                let max_radius = circle_area_size * 0.30;
                let min_radius = max_radius * 0.5;

                // Pattern info at top (with proper spacing)
                let pattern = self.current_pattern();
                painter.text(
                    Pos2::new(center_x, full_rect.min.y + 30.0),
                    egui::Align2::CENTER_CENTER,
                    &pattern.name,
                    egui::FontId::proportional(18.0),
                    SlowColors::BLACK,
                );

                let info = format!(
                    "inhale {}s • hold {}s • exhale {}s • rest {}s",
                    pattern.inhale as u32,
                    pattern.hold as u32,
                    pattern.exhale as u32,
                    pattern.rest as u32
                );
                painter.text(
                    Pos2::new(center_x, full_rect.min.y + 55.0),
                    egui::Align2::CENTER_CENTER,
                    info,
                    egui::FontId::proportional(12.0),
                    SlowColors::BLACK,
                );

                // Breathing circle - centered in remaining space
                let circle_center = Pos2::new(center_x, full_rect.min.y + 80.0 + circle_area_size / 2.0);

                // Calculate current radius based on phase
                let progress = self.phase_progress();
                let radius = match self.phase {
                    Phase::Inhale => min_radius + (max_radius - min_radius) * progress,
                    Phase::Hold => max_radius,
                    Phase::Exhale => max_radius - (max_radius - min_radius) * progress,
                    Phase::Rest => min_radius,
                };

                // Draw outer guide circle
                painter.circle_stroke(
                    circle_center,
                    max_radius + 8.0,
                    Stroke::new(1.0, SlowColors::BLACK),
                );

                // Draw inner guide circle
                painter.circle_stroke(
                    circle_center,
                    min_radius - 4.0,
                    Stroke::new(1.0, SlowColors::BLACK),
                );

                // Draw breathing circle
                if self.running {
                    painter.circle_filled(circle_center, radius, SlowColors::BLACK);
                } else {
                    painter.circle_stroke(circle_center, radius, Stroke::new(2.0, SlowColors::BLACK));
                }

                // Phase text below circle
                let text_y = circle_center.y + max_radius + 30.0;
                let phase_text = if self.running {
                    self.phase.name()
                } else {
                    "press space to start"
                };
                painter.text(
                    Pos2::new(center_x, text_y),
                    egui::Align2::CENTER_CENTER,
                    phase_text,
                    egui::FontId::proportional(16.0),
                    SlowColors::BLACK,
                );

                // Countdown below phase text
                if self.running {
                    let remaining = (self.phase_duration() - self.phase_elapsed).max(0.0);
                    painter.text(
                        Pos2::new(center_x, text_y + 25.0),
                        egui::Align2::CENTER_CENTER,
                        format!("{:.0}s", remaining.ceil()),
                        egui::FontId::proportional(22.0),
                        SlowColors::BLACK,
                    );
                }
            });

        // About dialog
        if self.show_about {
            let screen = ctx.screen_rect();
            let max_h = (screen.height() - 60.0).max(120.0);
            let resp = egui::Window::new("about slowBreath")
                .collapsible(false)
                .resizable(false)
                .default_width(300.0)
                .max_height(max_h)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().max_height(max_h - 50.0).show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.heading("slowBreath");
                            ui.label("version 0.2.2");
                            ui.add_space(8.0);
                            ui.label("mindful breathing timer for slowOS");
                        });
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(4.0);
                        ui.label("breathing patterns:");
                        ui.label("  4-7-8: relaxation technique");
                        ui.label("  box: focus and calm");
                        ui.label("  slow deep: general wellness");
                        ui.add_space(4.0);
                        ui.label("controls:");
                        ui.label("  click or space: start/stop");
                        ui.label("  esc: stop session");
                        ui.add_space(8.0);
                        ui.vertical_centered(|ui| {
                            if ui.button("ok").clicked() {
                                self.show_about = false;
                            }
                        });
                    });
                });
            if let Some(r) = &resp { slowcore::dither::draw_window_shadow_large(ctx, r.response.rect); }
        }

        self.repaint.end_frame(ctx);
    }
}
