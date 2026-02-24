//! SlowCalc application

use egui::{Context, Key};
use slowcore::repaint::RepaintController;
use slowcore::theme::{menu_bar, SlowColors};

#[derive(PartialEq, Clone, Copy)]
enum CalcMode {
    Basic,
    Scientific,
}

#[derive(PartialEq, Clone, Copy)]
enum Operation {
    None,
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
}

/// Window height for basic mode
const BASIC_HEIGHT: f32 = 350.0;
/// Window height for scientific mode
const SCIENTIFIC_HEIGHT: f32 = 480.0;

pub struct SlowCalcApp {
    display: String,
    stored_value: f64,
    current_operation: Operation,
    awaiting_operand: bool,
    mode: CalcMode,
    prev_mode: CalcMode,
    memory: f64,
    show_about: bool,
    repaint: RepaintController,
}

impl SlowCalcApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            display: "0".to_string(),
            stored_value: 0.0,
            current_operation: Operation::None,
            awaiting_operand: true,
            mode: CalcMode::Basic,
            prev_mode: CalcMode::Basic,
            memory: 0.0,
            show_about: false,
            repaint: RepaintController::new(),
        }
    }

    fn clear(&mut self) {
        self.display = "0".to_string();
        self.stored_value = 0.0;
        self.current_operation = Operation::None;
        self.awaiting_operand = true;
    }

    fn clear_entry(&mut self) {
        self.display = "0".to_string();
        self.awaiting_operand = true;
    }

    fn append_digit(&mut self, digit: char) {
        if self.awaiting_operand {
            self.display = digit.to_string();
            self.awaiting_operand = false;
        } else if self.display.len() < 15 {
            if self.display == "0" && digit != '.' {
                self.display = digit.to_string();
            } else {
                self.display.push(digit);
            }
        }
    }

    fn append_decimal(&mut self) {
        if self.awaiting_operand {
            self.display = "0.".to_string();
            self.awaiting_operand = false;
        } else if !self.display.contains('.') {
            self.display.push('.');
        }
    }

    fn toggle_sign(&mut self) {
        if let Ok(val) = self.display.parse::<f64>() {
            if val != 0.0 {
                self.display = format_number(-val);
            }
        }
    }

    fn set_operation(&mut self, op: Operation) {
        self.calculate();
        self.stored_value = self.display.parse().unwrap_or(0.0);
        self.current_operation = op;
        self.awaiting_operand = true;
    }

    fn calculate(&mut self) {
        if self.current_operation == Operation::None {
            return;
        }

        let current_value: f64 = self.display.parse().unwrap_or(0.0);
        let result = match self.current_operation {
            Operation::Add => self.stored_value + current_value,
            Operation::Subtract => self.stored_value - current_value,
            Operation::Multiply => self.stored_value * current_value,
            Operation::Divide => {
                if current_value == 0.0 {
                    f64::NAN
                } else {
                    self.stored_value / current_value
                }
            }
            Operation::Power => self.stored_value.powf(current_value),
            Operation::None => current_value,
        };

        self.display = format_number(result);
        self.stored_value = result;
        self.current_operation = Operation::None;
        self.awaiting_operand = true;
    }

    fn percent(&mut self) {
        if let Ok(val) = self.display.parse::<f64>() {
            let result = if self.current_operation == Operation::Add
                || self.current_operation == Operation::Subtract
            {
                self.stored_value * val / 100.0
            } else {
                val / 100.0
            };
            self.display = format_number(result);
        }
    }

    // Scientific functions
    fn apply_unary(&mut self, f: fn(f64) -> f64) {
        if let Ok(val) = self.display.parse::<f64>() {
            self.display = format_number(f(val));
            self.awaiting_operand = true;
        }
    }

    fn handle_keys(&mut self, ctx: &Context) {
        slowcore::theme::consume_special_keys(ctx);

        ctx.input(|i| {
            // Digit keys
            for digit in '0'..='9' {
                if i.key_pressed(digit_to_key(digit)) {
                    self.append_digit(digit);
                }
            }

            // Operations
            if i.key_pressed(Key::Plus) || (i.modifiers.shift && i.key_pressed(Key::Equals)) {
                self.set_operation(Operation::Add);
            }
            if i.key_pressed(Key::Minus) {
                self.set_operation(Operation::Subtract);
            }
            if i.modifiers.shift && i.key_pressed(Key::Num8) {
                self.set_operation(Operation::Multiply);
            }
            if i.key_pressed(Key::Slash) {
                self.set_operation(Operation::Divide);
            }

            // Decimal point
            if i.key_pressed(Key::Period) {
                self.append_decimal();
            }

            // Enter/equals
            if i.key_pressed(Key::Enter) || i.key_pressed(Key::Equals) {
                self.calculate();
            }

            // Clear
            if i.key_pressed(Key::Escape) || i.key_pressed(Key::C) {
                self.clear();
            }

            // Backspace
            if i.key_pressed(Key::Backspace) {
                if !self.awaiting_operand && self.display.len() > 1 {
                    self.display.pop();
                } else {
                    self.display = "0".to_string();
                    self.awaiting_operand = true;
                }
            }
        });
    }

    fn render_button(&self, ui: &mut egui::Ui, label: &str, width: f32, height: f32) -> bool {
        ui.add_sized(
            [width, height],
            egui::Button::new(label),
        ).clicked()
    }

    fn render_display(&self, ui: &mut egui::Ui) {
        let display_height = 48.0;
        egui::Frame::none()
            .fill(SlowColors::WHITE)
            .stroke(egui::Stroke::new(1.0, SlowColors::BLACK))
            .inner_margin(egui::Margin::symmetric(8.0, 4.0))
            .show(ui, |ui| {
                ui.set_min_height(display_height);
                ui.set_max_height(display_height);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(&self.display)
                            .font(egui::FontId::proportional(28.0))
                            .strong(),
                    );
                });
            });
    }

    fn render_basic_buttons(&mut self, ui: &mut egui::Ui) {
        let btn_w = (ui.available_width() - 24.0) / 4.0;
        let btn_h = 38.0;

        // Row 1: C, CE, %, /
        ui.horizontal(|ui| {
            if self.render_button(ui, "C", btn_w, btn_h) { self.clear(); }
            if self.render_button(ui, "CE", btn_w, btn_h) { self.clear_entry(); }
            if self.render_button(ui, "%", btn_w, btn_h) { self.percent(); }
            if self.render_button(ui, "/", btn_w, btn_h) { self.set_operation(Operation::Divide); }
        });

        // Row 2: 7, 8, 9, *
        ui.horizontal(|ui| {
            if self.render_button(ui, "7", btn_w, btn_h) { self.append_digit('7'); }
            if self.render_button(ui, "8", btn_w, btn_h) { self.append_digit('8'); }
            if self.render_button(ui, "9", btn_w, btn_h) { self.append_digit('9'); }
            if self.render_button(ui, "*", btn_w, btn_h) { self.set_operation(Operation::Multiply); }
        });

        // Row 3: 4, 5, 6, -
        ui.horizontal(|ui| {
            if self.render_button(ui, "4", btn_w, btn_h) { self.append_digit('4'); }
            if self.render_button(ui, "5", btn_w, btn_h) { self.append_digit('5'); }
            if self.render_button(ui, "6", btn_w, btn_h) { self.append_digit('6'); }
            if self.render_button(ui, "-", btn_w, btn_h) { self.set_operation(Operation::Subtract); }
        });

        // Row 4: 1, 2, 3, +
        ui.horizontal(|ui| {
            if self.render_button(ui, "1", btn_w, btn_h) { self.append_digit('1'); }
            if self.render_button(ui, "2", btn_w, btn_h) { self.append_digit('2'); }
            if self.render_button(ui, "3", btn_w, btn_h) { self.append_digit('3'); }
            if self.render_button(ui, "+", btn_w, btn_h) { self.set_operation(Operation::Add); }
        });

        // Row 5: +/-, 0, ., =
        ui.horizontal(|ui| {
            if self.render_button(ui, "+/-", btn_w, btn_h) { self.toggle_sign(); }
            if self.render_button(ui, "0", btn_w, btn_h) { self.append_digit('0'); }
            if self.render_button(ui, ".", btn_w, btn_h) { self.append_decimal(); }
            if self.render_button(ui, "=", btn_w, btn_h) { self.calculate(); }
        });
    }

    fn render_scientific_buttons(&mut self, ui: &mut egui::Ui) {
        let btn_w = (ui.available_width() - 24.0) / 4.0;
        let btn_h = 28.0;

        // Scientific row 1: sin, cos, tan, ln
        ui.horizontal(|ui| {
            if self.render_button(ui, "sin", btn_w, btn_h) { self.apply_unary(|x| x.to_radians().sin()); }
            if self.render_button(ui, "cos", btn_w, btn_h) { self.apply_unary(|x| x.to_radians().cos()); }
            if self.render_button(ui, "tan", btn_w, btn_h) { self.apply_unary(|x| x.to_radians().tan()); }
            if self.render_button(ui, "ln", btn_w, btn_h) { self.apply_unary(f64::ln); }
        });

        // Scientific row 2: asin, acos, atan, log
        ui.horizontal(|ui| {
            if self.render_button(ui, "asin", btn_w, btn_h) { self.apply_unary(|x| x.asin().to_degrees()); }
            if self.render_button(ui, "acos", btn_w, btn_h) { self.apply_unary(|x| x.acos().to_degrees()); }
            if self.render_button(ui, "atan", btn_w, btn_h) { self.apply_unary(|x| x.atan().to_degrees()); }
            if self.render_button(ui, "log", btn_w, btn_h) { self.apply_unary(f64::log10); }
        });

        // Scientific row 3: x^2, sqrt, x^y, e^x
        ui.horizontal(|ui| {
            if self.render_button(ui, "x^2", btn_w, btn_h) { self.apply_unary(|x| x * x); }
            if self.render_button(ui, "sqrt", btn_w, btn_h) { self.apply_unary(f64::sqrt); }
            if self.render_button(ui, "x^y", btn_w, btn_h) { self.set_operation(Operation::Power); }
            if self.render_button(ui, "e^x", btn_w, btn_h) { self.apply_unary(f64::exp); }
        });

        // Scientific row 4: 1/x, |x|, pi, e
        ui.horizontal(|ui| {
            if self.render_button(ui, "1/x", btn_w, btn_h) { self.apply_unary(|x| 1.0 / x); }
            if self.render_button(ui, "|x|", btn_w, btn_h) { self.apply_unary(f64::abs); }
            if self.render_button(ui, "pi", btn_w, btn_h) {
                self.display = format_number(std::f64::consts::PI);
                self.awaiting_operand = true;
            }
            if self.render_button(ui, "e", btn_w, btn_h) {
                self.display = format_number(std::f64::consts::E);
                self.awaiting_operand = true;
            }
        });

        ui.separator();

        // Basic buttons (smaller in scientific mode)
        let btn_w = (ui.available_width() - 24.0) / 4.0;
        let btn_h = 32.0;

        // Row 1: C, CE, %, /
        ui.horizontal(|ui| {
            if self.render_button(ui, "C", btn_w, btn_h) { self.clear(); }
            if self.render_button(ui, "CE", btn_w, btn_h) { self.clear_entry(); }
            if self.render_button(ui, "%", btn_w, btn_h) { self.percent(); }
            if self.render_button(ui, "/", btn_w, btn_h) { self.set_operation(Operation::Divide); }
        });

        // Row 2: 7, 8, 9, *
        ui.horizontal(|ui| {
            if self.render_button(ui, "7", btn_w, btn_h) { self.append_digit('7'); }
            if self.render_button(ui, "8", btn_w, btn_h) { self.append_digit('8'); }
            if self.render_button(ui, "9", btn_w, btn_h) { self.append_digit('9'); }
            if self.render_button(ui, "*", btn_w, btn_h) { self.set_operation(Operation::Multiply); }
        });

        // Row 3: 4, 5, 6, -
        ui.horizontal(|ui| {
            if self.render_button(ui, "4", btn_w, btn_h) { self.append_digit('4'); }
            if self.render_button(ui, "5", btn_w, btn_h) { self.append_digit('5'); }
            if self.render_button(ui, "6", btn_w, btn_h) { self.append_digit('6'); }
            if self.render_button(ui, "-", btn_w, btn_h) { self.set_operation(Operation::Subtract); }
        });

        // Row 4: 1, 2, 3, +
        ui.horizontal(|ui| {
            if self.render_button(ui, "1", btn_w, btn_h) { self.append_digit('1'); }
            if self.render_button(ui, "2", btn_w, btn_h) { self.append_digit('2'); }
            if self.render_button(ui, "3", btn_w, btn_h) { self.append_digit('3'); }
            if self.render_button(ui, "+", btn_w, btn_h) { self.set_operation(Operation::Add); }
        });

        // Row 5: +/-, 0, ., =
        ui.horizontal(|ui| {
            if self.render_button(ui, "+/-", btn_w, btn_h) { self.toggle_sign(); }
            if self.render_button(ui, "0", btn_w, btn_h) { self.append_digit('0'); }
            if self.render_button(ui, ".", btn_w, btn_h) { self.append_decimal(); }
            if self.render_button(ui, "=", btn_w, btn_h) { self.calculate(); }
        });
    }
}

impl eframe::App for SlowCalcApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.repaint.begin_frame(ctx);
        self.handle_keys(ctx);

        // Dynamically resize window when switching modes
        if self.mode != self.prev_mode {
            let new_height = match self.mode {
                CalcMode::Basic => BASIC_HEIGHT,
                CalcMode::Scientific => SCIENTIFIC_HEIGHT,
            };
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(
                egui::vec2(260.0, new_height),
            ));
            self.prev_mode = self.mode;
        }

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            menu_bar(ui, |ui| {
                ui.menu_button("mode", |ui| {
                    if ui.selectable_label(self.mode == CalcMode::Basic, "basic").clicked() {
                        self.mode = CalcMode::Basic;
                        ui.close_menu();
                    }
                    if ui.selectable_label(self.mode == CalcMode::Scientific, "scientific").clicked() {
                        self.mode = CalcMode::Scientific;
                        ui.close_menu();
                    }
                });
                ui.menu_button("memory", |ui| {
                    if ui.button("MC (clear)").clicked() {
                        self.memory = 0.0;
                        ui.close_menu();
                    }
                    if ui.button("MR (recall)").clicked() {
                        self.display = format_number(self.memory);
                        self.awaiting_operand = true;
                        ui.close_menu();
                    }
                    if ui.button("M+ (add)").clicked() {
                        if let Ok(val) = self.display.parse::<f64>() {
                            self.memory += val;
                        }
                        ui.close_menu();
                    }
                    if ui.button("M- (subtract)").clicked() {
                        if let Ok(val) = self.display.parse::<f64>() {
                            self.memory -= val;
                        }
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

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(8.0)))
            .show(ctx, |ui| {
                self.render_display(ui);
                ui.add_space(8.0);

                match self.mode {
                    CalcMode::Basic => self.render_basic_buttons(ui),
                    CalcMode::Scientific => self.render_scientific_buttons(ui),
                }
            });

        if self.show_about {
            let screen_rect = ctx.screen_rect();
            let max_h = (screen_rect.height() - 40.0).max(120.0);
            let resp = egui::Window::new("about calculator")
                .collapsible(false)
                .resizable(false)
                .default_width(240.0)
                .max_height(max_h)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().max_height(max_h - 50.0).show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.heading("calculator");
                            ui.label("version 0.2.2");
                            ui.add_space(4.0);
                            ui.label("calculator for slowOS");
                        });
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(2.0);
                        ui.label("modes:");
                        ui.label("  basic / scientific");
                        ui.add_space(2.0);
                        ui.label("keys: 0-9 +-*/ Enter Esc");
                    });
                    ui.vertical_centered(|ui| {
                        if ui.button("ok").clicked() {
                            self.show_about = false;
                        }
                    });
                });
            if let Some(r) = &resp { slowcore::dither::draw_window_shadow(ctx, r.response.rect); }
        }
        self.repaint.end_frame(ctx);
    }
}

fn format_number(n: f64) -> String {
    if n.is_nan() {
        return "Error".to_string();
    }
    if n.is_infinite() {
        return if n > 0.0 { "Inf" } else { "-Inf" }.to_string();
    }

    // Avoid floating point display issues
    if n == n.floor() && n.abs() < 1e12 {
        format!("{}", n as i64)
    } else if n.abs() >= 1e12 || (n.abs() < 1e-6 && n != 0.0) {
        format!("{:.6e}", n)
    } else {
        let s = format!("{:.10}", n);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn digit_to_key(digit: char) -> Key {
    match digit {
        '0' => Key::Num0,
        '1' => Key::Num1,
        '2' => Key::Num2,
        '3' => Key::Num3,
        '4' => Key::Num4,
        '5' => Key::Num5,
        '6' => Key::Num6,
        '7' => Key::Num7,
        '8' => Key::Num8,
        '9' => Key::Num9,
        _ => Key::Num0,
    }
}
