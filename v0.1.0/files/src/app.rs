//! SlowFiles - file explorer

use egui::{Context, Key};
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::status_bar;
use std::path::PathBuf;
use std::time::SystemTime;

struct FileEntry {
    name: String,
    path: PathBuf,
    is_dir: bool,
    size: u64,
    modified: String,
}

pub struct SlowFilesApp {
    current_dir: PathBuf,
    entries: Vec<FileEntry>,
    selected: Option<usize>,
    path_input: String,
    show_hidden: bool,
    sort_by: SortBy,
    sort_asc: bool,
    history: Vec<PathBuf>,
    history_idx: usize,
    show_about: bool,
    error_msg: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum SortBy { Name, Size, Modified }

impl SlowFilesApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let home = dirs_home().unwrap_or_else(|| PathBuf::from("/"));
        let mut app = Self {
            current_dir: home.clone(),
            entries: Vec::new(),
            selected: None,
            path_input: home.to_string_lossy().to_string(),
            show_hidden: false,
            sort_by: SortBy::Name,
            sort_asc: true,
            history: vec![home],
            history_idx: 0,
            show_about: false,
            error_msg: None,
        };
        app.refresh();
        app
    }

    fn navigate(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.current_dir = path.clone();
            self.path_input = path.to_string_lossy().to_string();
            self.selected = None;
            self.error_msg = None;

            // Update history
            self.history.truncate(self.history_idx + 1);
            self.history.push(path);
            self.history_idx = self.history.len() - 1;

            self.refresh();
        }
    }

    fn go_back(&mut self) {
        if self.history_idx > 0 {
            self.history_idx -= 1;
            let path = self.history[self.history_idx].clone();
            self.current_dir = path.clone();
            self.path_input = path.to_string_lossy().to_string();
            self.selected = None;
            self.refresh();
        }
    }

    fn go_forward(&mut self) {
        if self.history_idx < self.history.len() - 1 {
            self.history_idx += 1;
            let path = self.history[self.history_idx].clone();
            self.current_dir = path.clone();
            self.path_input = path.to_string_lossy().to_string();
            self.selected = None;
            self.refresh();
        }
    }

    fn go_up(&mut self) {
        if let Some(parent) = self.current_dir.parent() {
            self.navigate(parent.to_path_buf());
        }
    }

    fn refresh(&mut self) {
        self.entries.clear();
        match std::fs::read_dir(&self.current_dir) {
            Ok(rd) => {
                for entry in rd.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !self.show_hidden && name.starts_with('.') { continue; }

                    let meta = entry.metadata().ok();
                    let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                    let modified = meta.as_ref()
                        .and_then(|m| m.modified().ok())
                        .map(format_time)
                        .unwrap_or_default();

                    self.entries.push(FileEntry {
                        name,
                        path: entry.path(),
                        is_dir,
                        size,
                        modified,
                    });
                }
                self.sort_entries();
            }
            Err(e) => { self.error_msg = Some(e.to_string()); }
        }
    }

    fn sort_entries(&mut self) {
        // Directories first, then sort
        self.entries.sort_by(|a, b| {
            b.is_dir.cmp(&a.is_dir).then_with(|| {
                let cmp = match self.sort_by {
                    SortBy::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                    SortBy::Size => a.size.cmp(&b.size),
                    SortBy::Modified => a.modified.cmp(&b.modified),
                };
                if self.sort_asc { cmp } else { cmp.reverse() }
            })
        });
    }

    fn open_selected(&mut self) {
        if let Some(idx) = self.selected {
            if let Some(entry) = self.entries.get(idx) {
                if entry.is_dir {
                    self.navigate(entry.path.clone());
                } else {
                    let _ = open::that(&entry.path);
                }
            }
        }
    }

    fn handle_keys(&mut self, ctx: &Context) {
        // Consume Tab to prevent menu hover
        ctx.input_mut(|i| {
            if i.key_pressed(egui::Key::Tab) {
                i.events.retain(|e| !matches!(e, egui::Event::Key { key: egui::Key::Tab, .. }));
            }
        });
        ctx.input(|i| {
            let cmd = i.modifiers.command;
            if cmd && i.key_pressed(Key::ArrowUp) { self.go_up(); }
            if cmd && i.key_pressed(Key::ArrowLeft) { self.go_back(); }
            if cmd && i.key_pressed(Key::ArrowRight) { self.go_forward(); }
            if i.key_pressed(Key::Enter) { self.open_selected(); }
            if !cmd {
                if i.key_pressed(Key::ArrowUp) {
                    if let Some(idx) = self.selected {
                        if idx > 0 { self.selected = Some(idx - 1); }
                    }
                }
                if i.key_pressed(Key::ArrowDown) {
                    let max = self.entries.len().saturating_sub(1);
                    if let Some(idx) = self.selected {
                        if idx < max { self.selected = Some(idx + 1); }
                    } else if !self.entries.is_empty() {
                        self.selected = Some(0);
                    }
                }
            }
        });
    }

    fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("◀").on_hover_text("back").clicked() { self.go_back(); }
            if ui.button("▶").on_hover_text("forward").clicked() { self.go_forward(); }
            if ui.button("▲").on_hover_text("up").clicked() { self.go_up(); }
            if ui.button("⟳").on_hover_text("refresh").clicked() { self.refresh(); }
            ui.separator();

            let r = ui.text_edit_singleline(&mut self.path_input);
            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                let path = PathBuf::from(&self.path_input);
                if path.is_dir() { self.navigate(path); }
            }
        });
    }

    fn render_file_list(&mut self, ui: &mut egui::Ui) {
        // Column headers
        ui.horizontal(|ui| {
            let name_w = ui.available_width() - 180.0;
            if ui.add_sized([name_w, 20.0], egui::Button::new("name")).clicked() {
                if self.sort_by == SortBy::Name { self.sort_asc = !self.sort_asc; }
                else { self.sort_by = SortBy::Name; self.sort_asc = true; }
                self.sort_entries();
            }
            if ui.add_sized([80.0, 20.0], egui::Button::new("size")).clicked() {
                if self.sort_by == SortBy::Size { self.sort_asc = !self.sort_asc; }
                else { self.sort_by = SortBy::Size; self.sort_asc = true; }
                self.sort_entries();
            }
            if ui.add_sized([100.0, 20.0], egui::Button::new("modified")).clicked() {
                if self.sort_by == SortBy::Modified { self.sort_asc = !self.sort_asc; }
                else { self.sort_by = SortBy::Modified; self.sort_asc = true; }
                self.sort_entries();
            }
        });
        ui.separator();

        // Collect entry data to avoid borrow conflict
        let display_entries: Vec<(usize, String, String, String, String, bool, PathBuf)> =
            self.entries.iter().enumerate().map(|(idx, entry)| {
                let icon = if entry.is_dir { "📁".into() } else { file_icon(&entry.name).to_string() };
                let size_str = if entry.is_dir { "—".into() } else { format_size(entry.size) };
                (idx, entry.name.clone(), icon, size_str, entry.modified.clone(), entry.is_dir, entry.path.clone())
            }).collect();

        // File list
        let mut nav_target: Option<PathBuf> = None;
        let mut open_target: Option<PathBuf> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (idx, name, icon, size_str, modified, is_dir, path) in &display_entries {
                let selected = self.selected == Some(*idx);
                let row_height = 18.0;
                let total_w = ui.available_width();
                let name_w = total_w - 180.0;

                // Draw the row manually so we control alignment
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(total_w, row_height),
                    egui::Sense::click(),
                );

                if ui.is_rect_visible(rect) {
                    let painter = ui.painter();

                    // Selection highlight — dithered
                    if selected {
                        slowcore::dither::draw_dither_selection(painter, rect);
                    } else if response.hovered() {
                        slowcore::dither::draw_dither_hover(painter, rect);
                    }

                    let text_color = if selected { SlowColors::WHITE } else { SlowColors::BLACK };

                    // Left-aligned name (icon + filename)
                    let label = format!("{} {}", icon, name);
                    painter.text(
                        egui::pos2(rect.min.x + 4.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &label,
                        egui::FontId::proportional(12.0),
                        text_color,
                    );

                    // Size column — right side
                    let size_x = rect.min.x + name_w + 4.0;
                    painter.text(
                        egui::pos2(size_x, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        size_str,
                        egui::FontId::proportional(11.0),
                        text_color,
                    );

                    // Modified column
                    let mod_x = rect.min.x + name_w + 84.0;
                    painter.text(
                        egui::pos2(mod_x, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        modified,
                        egui::FontId::proportional(11.0),
                        text_color,
                    );
                }

                if response.clicked() { self.selected = Some(*idx); }
                if response.double_clicked() {
                    if *is_dir {
                        nav_target = Some(path.clone());
                    } else {
                        open_target = Some(path.clone());
                    }
                }
            }
        });
        if let Some(path) = nav_target { self.navigate(path); }
        if let Some(path) = open_target { let _ = open::that(&path); }
    }
}

impl eframe::App for SlowFilesApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.handle_keys(ctx);

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            menu_bar(ui, |ui| {
                ui.menu_button("file", |ui| {
                    if ui.button("new window").clicked() { ui.close_menu(); }
                });
                ui.menu_button("view", |ui| {
                    if ui.button(format!("{} show hidden", if self.show_hidden { "✓" } else { " " })).clicked() {
                        self.show_hidden = !self.show_hidden;
                        self.refresh();
                        ui.close_menu();
                    }
                    if ui.button("refresh ⌘r").clicked() { self.refresh(); ui.close_menu(); }
                });
                ui.menu_button("go", |ui| {
                    if ui.button("Back    ⌘←").clicked() { self.go_back(); ui.close_menu(); }
                    if ui.button("Forward ⌘→").clicked() { self.go_forward(); ui.close_menu(); }
                    if ui.button("up      ⌘↑").clicked() { self.go_up(); ui.close_menu(); }
                    ui.separator();
                    if ui.button("home").clicked() {
                        if let Some(h) = dirs_home() { self.navigate(h); }
                        ui.close_menu();
                    }
                    if ui.button("documents").clicked() {
                        self.navigate(slowcore::storage::documents_dir());
                        ui.close_menu();
                    }
                });
                ui.menu_button("help", |ui| {
                    if ui.button("about").clicked() { self.show_about = true; ui.close_menu(); }
                });
            });
        });
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| self.render_toolbar(ui));
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let info = if let Some(idx) = self.selected {
                if let Some(e) = self.entries.get(idx) {
                    format!("{}  —  {}", e.name, if e.is_dir { "folder".into() } else { format_size(e.size) })
                } else { String::new() }
            } else {
                format!("{} items", self.entries.len())
            };
            status_bar(ui, &info);
        });

        egui::CentralPanel::default().frame(
            egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(4.0))
        ).show(ctx, |ui| {
            if let Some(ref err) = self.error_msg {
                ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
                ui.separator();
            }
            self.render_file_list(ui);
        });

        if self.show_about {
            egui::Window::new("about files").collapsible(false).show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("files");
                    ui.label("version 0.1.0");
                    ui.add_space(5.0);
                    ui.label("a file browser by the slow computer company");
                    ui.add_space(5.0);
                    if ui.button("ok").clicked() { self.show_about = false; }
                });
            });
        }
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 { format!("{} B", bytes) }
    else if bytes < 1024 * 1024 { format!("{:.1} KB", bytes as f64 / 1024.0) }
    else if bytes < 1024 * 1024 * 1024 { format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0)) }
    else { format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0)) }
}

fn format_time(time: SystemTime) -> String {
    let duration = time.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs() as i64;
    let dt = chrono::DateTime::from_timestamp(secs, 0)
        .unwrap_or_default();
    dt.format("%Y-%m-%d %H:%M").to_string()
}

fn file_icon(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "txt" | "md" => "📝",
        "png" | "jpg" | "jpeg" | "bmp" | "gif" => "🖼",
        "pdf" => "📕",
        "epub" => "📖",
        "mp3" | "wav" | "flac" | "ogg" => "🎵",
        "csv" | "json" => "📊",
        "rs" | "py" | "js" | "c" | "h" => "📜",
        "zip" | "tar" | "gz" => "📦",
        _ => "📄",
    }
}
