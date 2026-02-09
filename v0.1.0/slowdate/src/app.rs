//! SlowDate - a minimal calendar application for slowOS

use chrono::{Datelike, Local, NaiveDate};
use egui::{Context, Key, Rect, Vec2};
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::status_bar;
use std::collections::HashMap;
use std::path::PathBuf;

/// A calendar event
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct Event {
    date: String, // YYYY-MM-DD format
    title: String,
    notes: String,
}

pub struct SlowDateApp {
    /// Currently displayed year/month
    year: i32,
    month: u32,
    /// Selected day (1-31)
    selected_day: Option<u32>,
    /// Events keyed by date string (YYYY-MM-DD)
    events: HashMap<String, Vec<Event>>,
    /// Path to the events CSV file
    events_path: PathBuf,
    /// Show about dialog
    show_about: bool,
    /// Show event dialog
    show_event_dialog: bool,
    /// Event being edited
    event_title: String,
    event_notes: String,
    /// Index of event being edited (None = new event)
    editing_event_idx: Option<usize>,
    /// Focus text field on next frame
    focus_title_field: bool,
    /// Show import dialog
    show_import_dialog: bool,
    /// Show export dialog
    show_export_dialog: bool,
    /// Import file path input
    import_path: String,
    /// Export file path input
    export_path: String,
    /// Status message for import/export
    status_message: Option<String>,
}

impl SlowDateApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let today = Local::now().date_naive();
        let events_path = slowcore::storage::documents_dir().join("calendar.csv");

        let export_default = slowcore::storage::documents_dir().join("calendar_export.csv");
        let mut app = Self {
            year: today.year(),
            month: today.month(),
            selected_day: Some(today.day()),
            events: HashMap::new(),
            events_path,
            show_about: false,
            show_event_dialog: false,
            event_title: String::new(),
            event_notes: String::new(),
            editing_event_idx: None,
            focus_title_field: false,
            show_import_dialog: false,
            show_export_dialog: false,
            import_path: String::new(),
            export_path: export_default.to_string_lossy().to_string(),
            status_message: None,
        };
        app.load_events();
        app
    }

    fn load_events(&mut self) {
        self.events.clear();
        if let Ok(content) = std::fs::read_to_string(&self.events_path) {
            for line in content.lines().skip(1) { // Skip header
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() >= 3 {
                    let date = parts[0].to_string();
                    let title = unescape_csv(parts[1]);
                    let notes = unescape_csv(parts[2..].join(",").as_str());
                    let event = Event { date: date.clone(), title, notes };
                    self.events.entry(date).or_default().push(event);
                }
            }
        }
    }

    fn save_events(&self) {
        let mut lines = vec!["date,title,notes".to_string()];
        let mut all_events: Vec<&Event> = self.events.values().flatten().collect();
        all_events.sort_by(|a, b| a.date.cmp(&b.date));

        for event in all_events {
            lines.push(format!(
                "{},{},{}",
                event.date,
                escape_csv(&event.title),
                escape_csv(&event.notes)
            ));
        }
        let _ = std::fs::write(&self.events_path, lines.join("\n"));
    }

    fn do_export(&mut self) {
        let csv_content = export_to_csv(&self.events);
        match std::fs::write(&self.export_path, csv_content) {
            Ok(_) => {
                self.status_message = Some(format!("Exported to {}", self.export_path));
            }
            Err(e) => {
                self.status_message = Some(format!("Export failed: {}", e));
            }
        }
        self.show_export_dialog = false;
    }

    fn do_import(&mut self) {
        match std::fs::read_to_string(&self.import_path) {
            Ok(content) => {
                let imported = import_from_csv(&content);
                let count = imported.len();
                for event in imported {
                    self.events.entry(event.date.clone()).or_default().push(event);
                }
                self.save_events();
                self.status_message = Some(format!("Imported {} events", count));
            }
            Err(e) => {
                self.status_message = Some(format!("Import failed: {}", e));
            }
        }
        self.show_import_dialog = false;
    }

    fn selected_date_string(&self) -> Option<String> {
        self.selected_day.map(|day| {
            format!("{:04}-{:02}-{:02}", self.year, self.month, day)
        })
    }

    fn days_in_month(&self) -> u32 {
        let next_month = if self.month == 12 {
            NaiveDate::from_ymd_opt(self.year + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(self.year, self.month + 1, 1)
        };
        next_month
            .and_then(|d| d.pred_opt())
            .map(|d| d.day())
            .unwrap_or(30)
    }

    fn first_weekday(&self) -> u32 {
        NaiveDate::from_ymd_opt(self.year, self.month, 1)
            .map(|d| d.weekday().num_days_from_sunday())
            .unwrap_or(0)
    }

    fn prev_month(&mut self) {
        if self.month == 1 {
            self.month = 12;
            self.year -= 1;
        } else {
            self.month -= 1;
        }
        // Clamp selected day to valid range
        let max_day = self.days_in_month();
        if let Some(day) = self.selected_day {
            if day > max_day {
                self.selected_day = Some(max_day);
            }
        }
    }

    fn next_month(&mut self) {
        if self.month == 12 {
            self.month = 1;
            self.year += 1;
        } else {
            self.month += 1;
        }
        // Clamp selected day to valid range
        let max_day = self.days_in_month();
        if let Some(day) = self.selected_day {
            if day > max_day {
                self.selected_day = Some(max_day);
            }
        }
    }

    fn go_today(&mut self) {
        let today = Local::now().date_naive();
        self.year = today.year();
        self.month = today.month();
        self.selected_day = Some(today.day());
    }

    fn add_event(&mut self) {
        if self.event_title.trim().is_empty() {
            return;
        }
        if let Some(date_str) = self.selected_date_string() {
            let event = Event {
                date: date_str.clone(),
                title: self.event_title.trim().to_string(),
                notes: self.event_notes.clone(),
            };

            if let Some(idx) = self.editing_event_idx {
                // Update existing event
                if let Some(events) = self.events.get_mut(&date_str) {
                    if idx < events.len() {
                        events[idx] = event;
                    }
                }
            } else {
                // Add new event
                self.events.entry(date_str).or_default().push(event);
            }

            self.save_events();
            self.event_title.clear();
            self.event_notes.clear();
            self.editing_event_idx = None;
            self.show_event_dialog = false;
        }
    }

    fn delete_event(&mut self, idx: usize) {
        if let Some(date_str) = self.selected_date_string() {
            if let Some(events) = self.events.get_mut(&date_str) {
                if idx < events.len() {
                    events.remove(idx);
                    if events.is_empty() {
                        self.events.remove(&date_str);
                    }
                    self.save_events();
                }
            }
        }
    }

    fn handle_keys(&mut self, ctx: &Context) {
        slowcore::theme::consume_special_keys(ctx);
        ctx.input(|i| {
            if i.key_pressed(Key::ArrowLeft) {
                self.prev_month();
            }
            if i.key_pressed(Key::ArrowRight) {
                self.next_month();
            }
            if i.modifiers.command && i.key_pressed(Key::T) {
                self.go_today();
            }
        });
    }

    fn render_calendar(&mut self, ui: &mut egui::Ui) {
        let today = Local::now().date_naive();
        let is_current_month = self.year == today.year() && self.month == today.month();

        // Month/year header with navigation
        ui.horizontal(|ui| {
            if ui.button("◀").clicked() {
                self.prev_month();
            }

            let month_name = month_name(self.month);
            ui.heading(format!("{} {}", month_name, self.year));

            if ui.button("▶").clicked() {
                self.next_month();
            }

            ui.add_space(8.0);
            if ui.button("today").clicked() {
                self.go_today();
            }
        });

        ui.add_space(8.0);

        // Day headers
        let cell_size = 40.0;
        let days = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];

        ui.horizontal(|ui| {
            for day in &days {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(cell_size, 20.0), egui::Sense::hover());
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    *day,
                    egui::FontId::proportional(12.0),
                    SlowColors::BLACK,
                );
            }
        });

        ui.add_space(4.0);

        // Calendar grid
        let days_in_month = self.days_in_month();
        let first_weekday = self.first_weekday();
        let mut day = 1u32;

        for week in 0..6 {
            if day > days_in_month {
                break;
            }

            ui.horizontal(|ui| {
                for weekday in 0..7 {
                    let cell_idx = week * 7 + weekday;

                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(cell_size, cell_size),
                        egui::Sense::click(),
                    );

                    if cell_idx >= first_weekday && day <= days_in_month {
                        let is_selected = self.selected_day == Some(day);
                        let is_today = is_current_month && day == today.day();
                        let date_str = format!("{:04}-{:02}-{:02}", self.year, self.month, day);
                        let has_events = self.events.contains_key(&date_str);

                        // Draw cell background
                        if is_selected {
                            slowcore::dither::draw_dither_selection(ui.painter(), rect);
                        } else if response.hovered() {
                            slowcore::dither::draw_dither_hover(ui.painter(), rect);
                        }

                        // Day number
                        let text_color = if is_selected { SlowColors::WHITE } else { SlowColors::BLACK };
                        let font = if is_today {
                            egui::FontId::new(14.0, egui::FontFamily::Proportional)
                        } else {
                            egui::FontId::proportional(12.0)
                        };

                        ui.painter().text(
                            rect.center() - Vec2::new(0.0, 6.0),
                            egui::Align2::CENTER_CENTER,
                            format!("{}", day),
                            font,
                            text_color,
                        );

                        // Today indicator (circle)
                        if is_today {
                            ui.painter().circle_stroke(
                                rect.center() - Vec2::new(0.0, 6.0),
                                12.0,
                                egui::Stroke::new(1.5, text_color),
                            );
                        }

                        // Event indicator (dot)
                        if has_events {
                            let dot_color = if is_selected { SlowColors::WHITE } else { SlowColors::BLACK };
                            ui.painter().circle_filled(
                                rect.center() + Vec2::new(0.0, 10.0),
                                3.0,
                                dot_color,
                            );
                        }

                        if response.clicked() {
                            self.selected_day = Some(day);
                        }

                        if response.double_clicked() {
                            self.selected_day = Some(day);
                            self.show_event_dialog = true;
                            self.focus_title_field = true;
                            self.event_title.clear();
                            self.event_notes.clear();
                            self.editing_event_idx = None;
                        }

                        day += 1;
                    }
                }
            });
        }
    }

    fn render_events(&mut self, ui: &mut egui::Ui) {
        if let Some(date_str) = self.selected_date_string() {
            ui.horizontal(|ui| {
                ui.heading(format_date_display(&date_str));
                if ui.button("+").on_hover_text("add event").clicked() {
                    self.show_event_dialog = true;
                    self.focus_title_field = true;
                    self.event_title.clear();
                    self.event_notes.clear();
                    self.editing_event_idx = None;
                }
            });

            ui.add_space(4.0);

            let events = self.events.get(&date_str).cloned().unwrap_or_default();
            let mut action: Option<(usize, bool)> = None; // (idx, is_delete)

            if events.is_empty() {
                ui.label("no events");
            } else {
                for (idx, event) in events.iter().enumerate() {
                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), 24.0),
                        egui::Sense::click(),
                    );

                    if response.hovered() {
                        slowcore::dither::draw_dither_hover(ui.painter(), rect);
                    }

                    // Event title
                    ui.painter().text(
                        rect.left_center() + Vec2::new(4.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        &event.title,
                        egui::FontId::proportional(12.0),
                        SlowColors::BLACK,
                    );

                    // Delete button on right
                    let delete_rect = Rect::from_min_size(
                        rect.right_top() - Vec2::new(20.0, 0.0),
                        Vec2::new(20.0, 24.0),
                    );

                    if response.hovered() {
                        ui.painter().text(
                            delete_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "×",
                            egui::FontId::proportional(14.0),
                            SlowColors::BLACK,
                        );

                        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                            if delete_rect.contains(pos) && response.clicked() {
                                action = Some((idx, true));
                            }
                        }
                    }

                    // Click to edit
                    if response.clicked() && action.is_none() {
                        action = Some((idx, false));
                    }
                }
            }

            // Handle actions after loop
            if let Some((idx, is_delete)) = action {
                if is_delete {
                    self.delete_event(idx);
                } else {
                    // Edit event
                    if let Some(events) = self.events.get(&date_str) {
                        if let Some(event) = events.get(idx) {
                            self.event_title = event.title.clone();
                            self.event_notes = event.notes.clone();
                            self.editing_event_idx = Some(idx);
                            self.show_event_dialog = true;
                            self.focus_title_field = true;
                        }
                    }
                }
            }
        }
    }
}

impl eframe::App for SlowDateApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.handle_keys(ctx);

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            menu_bar(ui, |ui| {
                ui.menu_button("file", |ui| {
                    if ui.button("new event...  ⌘N").clicked() {
                        self.show_event_dialog = true;
                        self.focus_title_field = true;
                        self.event_title.clear();
                        self.event_notes.clear();
                        self.editing_event_idx = None;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("import CSV...").clicked() {
                        self.show_import_dialog = true;
                        ui.close_menu();
                    }
                    if ui.button("export CSV...").clicked() {
                        self.show_export_dialog = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("reload").clicked() {
                        self.load_events();
                        ui.close_menu();
                    }
                });
                ui.menu_button("view", |ui| {
                    if ui.button("today  ⌘T").clicked() {
                        self.go_today();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("previous month  ←").clicked() {
                        self.prev_month();
                        ui.close_menu();
                    }
                    if ui.button("next month  →").clicked() {
                        self.next_month();
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

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let event_count: usize = self.events.values().map(|v| v.len()).sum();
            status_bar(ui, &format!("{} events", event_count));
        });

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(8.0)))
            .show(ctx, |ui| {
                self.render_calendar(ui);
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                self.render_events(ui);
            });

        // About dialog
        if self.show_about {
            egui::Window::new("about slowDate")
                .collapsible(false)
                .resizable(false)
                .default_width(280.0)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("slowDate");
                        ui.label("version 0.1.0");
                        ui.add_space(8.0);
                        ui.label("calendar for slowOS");
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label("features:");
                    ui.label("  monthly calendar view");
                    ui.label("  simple event management");
                    ui.label("  CSV file storage");
                    ui.add_space(4.0);
                    ui.label("frameworks:");
                    ui.label("  egui/eframe (MIT), chrono (MIT)");
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        if ui.button("ok").clicked() {
                            self.show_about = false;
                        }
                    });
                });
        }

        // Event dialog
        if self.show_event_dialog {
            let title = if self.editing_event_idx.is_some() { "edit event" } else { "new event" };
            let should_focus = self.focus_title_field;
            self.focus_title_field = false;

            egui::Window::new(title)
                .collapsible(false)
                .resizable(false)
                .default_width(280.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("title:");
                        let r = ui.text_edit_singleline(&mut self.event_title);
                        if should_focus {
                            r.request_focus();
                        }
                    });
                    ui.add_space(4.0);
                    ui.label("notes:");
                    ui.add(egui::TextEdit::multiline(&mut self.event_notes)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("cancel").clicked() {
                            self.show_event_dialog = false;
                            self.event_title.clear();
                            self.event_notes.clear();
                            self.editing_event_idx = None;
                        }
                        let button_text = if self.editing_event_idx.is_some() { "save" } else { "add" };
                        if ui.button(button_text).clicked() {
                            self.add_event();
                        }
                    });
                });
        }

        // Import dialog
        if self.show_import_dialog {
            egui::Window::new("import calendar")
                .collapsible(false)
                .resizable(false)
                .default_width(400.0)
                .show(ctx, |ui| {
                    ui.label("Import events from a CSV file.");
                    ui.label("Supports Google Calendar and Apple Calendar exports.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("file path:");
                        ui.text_edit_singleline(&mut self.import_path);
                    });
                    ui.add_space(4.0);
                    ui.label("(events will be added to existing calendar)");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("cancel").clicked() {
                            self.show_import_dialog = false;
                        }
                        if ui.button("import").clicked() {
                            self.do_import();
                        }
                    });
                });
        }

        // Export dialog
        if self.show_export_dialog {
            egui::Window::new("export calendar")
                .collapsible(false)
                .resizable(false)
                .default_width(400.0)
                .show(ctx, |ui| {
                    ui.label("Export calendar to CSV format.");
                    ui.label("Compatible with Google Calendar and Apple Calendar.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("save to:");
                        ui.text_edit_singleline(&mut self.export_path);
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("cancel").clicked() {
                            self.show_export_dialog = false;
                        }
                        if ui.button("export").clicked() {
                            self.do_export();
                        }
                    });
                });
        }

        // Show status message if any
        if let Some(ref msg) = self.status_message.clone() {
            egui::Window::new("status")
                .collapsible(false)
                .resizable(false)
                .default_width(300.0)
                .show(ctx, |ui| {
                    ui.label(msg);
                    ui.add_space(8.0);
                    if ui.button("ok").clicked() {
                        self.status_message = None;
                    }
                });
        }
    }
}

fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}

/// Export calendar to standard CSV format compatible with Google/Apple Calendar
/// Format: Subject,Start Date,Start Time,End Date,End Time,Description
fn export_to_csv(events: &HashMap<String, Vec<Event>>) -> String {
    let mut lines = vec!["Subject,Start Date,Start Time,End Date,End Time,Description".to_string()];

    let mut all_events: Vec<&Event> = events.values().flatten().collect();
    all_events.sort_by(|a, b| a.date.cmp(&b.date));

    for event in all_events {
        // Convert YYYY-MM-DD to MM/DD/YYYY for better compatibility
        let date_parts: Vec<&str> = event.date.split('-').collect();
        let formatted_date = if date_parts.len() == 3 {
            format!("{}/{}/{}", date_parts[1], date_parts[2], date_parts[0])
        } else {
            event.date.clone()
        };

        lines.push(format!(
            "{},{},,,{}",
            escape_csv(&event.title),
            formatted_date,
            escape_csv(&event.notes)
        ));
    }
    lines.join("\n")
}

/// Import calendar from CSV (supports Google/Apple Calendar formats)
fn import_from_csv(content: &str) -> Vec<Event> {
    let mut events = Vec::new();

    for (i, line) in content.lines().enumerate() {
        if i == 0 { continue; } // Skip header

        let parts: Vec<&str> = line.split(',').collect();
        if parts.is_empty() { continue; }

        // Try to detect format and extract data
        // Google/Apple: Subject,Start Date,Start Time,End Date,End Time,Description
        // slowDate: date,title,notes

        let (date, title, notes) = if parts.len() >= 2 {
            // Check if first field looks like a date (has /)
            if parts[0].contains('/') || parts[0].contains('-') {
                // Format: date,title,notes (slowDate format)
                let date = parse_date_to_internal(parts[0]);
                let title = if parts.len() > 1 { unescape_csv(parts[1]) } else { String::new() };
                let notes = if parts.len() > 2 { unescape_csv(&parts[2..].join(",")) } else { String::new() };
                (date, title, notes)
            } else {
                // Format: Subject,Start Date,... (Google/Apple format)
                let title = unescape_csv(parts[0]);
                let date = if parts.len() > 1 { parse_date_to_internal(parts[1]) } else { String::new() };
                let notes = if parts.len() > 5 { unescape_csv(parts[5]) } else { String::new() };
                (date, title, notes)
            }
        } else {
            continue;
        };

        if !date.is_empty() && !title.is_empty() {
            events.push(Event { date, title, notes });
        }
    }
    events
}

/// Parse various date formats to internal YYYY-MM-DD format
fn parse_date_to_internal(s: &str) -> String {
    let s = s.trim();

    // Try MM/DD/YYYY (Google Calendar format)
    if let Ok(date) = NaiveDate::parse_from_str(s, "%m/%d/%Y") {
        return format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day());
    }

    // Try DD/MM/YYYY (European format)
    if let Ok(date) = NaiveDate::parse_from_str(s, "%d/%m/%Y") {
        return format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day());
    }

    // Try YYYY-MM-DD (ISO format / slowDate internal)
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day());
    }

    // Try M/D/YYYY (short Google format)
    if s.contains('/') {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() == 3 {
            if let (Ok(m), Ok(d), Ok(y)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>(), parts[2].parse::<i32>()) {
                if let Some(date) = NaiveDate::from_ymd_opt(y, m, d) {
                    return format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day());
                }
            }
        }
    }

    String::new()
}

fn format_date_display(date_str: &str) -> String {
    // Convert YYYY-MM-DD to readable format
    if let Some(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok() {
        let month = month_name(date.month());
        format!("{} {}, {}", month, date.day(), date.year())
    } else {
        date_str.to_string()
    }
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn unescape_csv(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        s[1..s.len()-1].replace("\"\"", "\"")
    } else {
        s.to_string()
    }
}
