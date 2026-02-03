//! SlowFiles - file explorer

use egui::{Context, Key, Sense};
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::status_bar;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{SystemTime, Instant};
use trash::move_to_trash;

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
    selected: HashSet<usize>,
    /// Last clicked index for shift+click range selection
    last_clicked: Option<usize>,
    path_input: String,
    show_hidden: bool,
    sort_by: SortBy,
    sort_asc: bool,
    history: Vec<PathBuf>,
    history_idx: usize,
    show_about: bool,
    error_msg: Option<String>,
    /// Dragging state: paths of files being dragged
    dragging: Option<Vec<PathBuf>>,
    /// Index of folder being hovered during drag
    drag_hover_idx: Option<usize>,
    /// Time started hovering over back/up button during drag
    drag_button_hover_start: Option<(ButtonType, Instant)>,
    /// Whether button is flashing (ready to accept drop)
    drag_button_flash: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum ButtonType { Back, Up }

#[derive(Clone, Copy, PartialEq)]
enum SortBy { Name, Size, Modified }

impl SlowFilesApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let home = dirs_home().unwrap_or_else(|| PathBuf::from("/"));
        let mut app = Self {
            current_dir: home.clone(),
            entries: Vec::new(),
            selected: HashSet::new(),
            last_clicked: None,
            path_input: home.to_string_lossy().to_string(),
            show_hidden: false,
            sort_by: SortBy::Name,
            sort_asc: true,
            history: vec![home],
            history_idx: 0,
            show_about: false,
            error_msg: None,
            dragging: None,
            drag_hover_idx: None,
            drag_button_hover_start: None,
            drag_button_flash: false,
        };
        app.refresh();
        app
    }

    fn move_files_to(&mut self, files: &[PathBuf], dest_dir: &PathBuf) {
        for file in files {
            if let Some(name) = file.file_name() {
                let dest = dest_dir.join(name);
                if let Err(e) = std::fs::rename(file, &dest) {
                    self.error_msg = Some(format!("failed to move: {}", e));
                    return;
                }
            }
        }
        self.refresh();
    }

    fn navigate(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.current_dir = path.clone();
            self.path_input = path.to_string_lossy().to_string();
            self.selected.clear();
            self.last_clicked = None;
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
            self.selected.clear();
            self.last_clicked = None;
            self.refresh();
        }
    }

    fn go_forward(&mut self) {
        if self.history_idx < self.history.len() - 1 {
            self.history_idx += 1;
            let path = self.history[self.history_idx].clone();
            self.current_dir = path.clone();
            self.path_input = path.to_string_lossy().to_string();
            self.selected.clear();
            self.last_clicked = None;
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
        // Open the first selected item (or navigate if it's a directory)
        if let Some(&idx) = self.selected.iter().next() {
            if let Some(entry) = self.entries.get(idx) {
                if entry.is_dir {
                    self.navigate(entry.path.clone());
                } else {
                    let _ = open::that(&entry.path);
                }
            }
        }
    }

    fn delete_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        // Collect paths to delete (sorted descending so indices don't shift)
        let mut indices: Vec<usize> = self.selected.iter().copied().collect();
        indices.sort_by(|a, b| b.cmp(a));

        for idx in indices {
            if let Some(entry) = self.entries.get(idx) {
                let _ = move_to_trash(&entry.path);
            }
        }
        self.selected.clear();
        self.last_clicked = None;
        self.refresh();
    }

    fn handle_keys(&mut self, ctx: &Context) {
        slowcore::theme::consume_special_keys(ctx);
        ctx.input(|i| {
            let cmd = i.modifiers.command;
            if cmd && i.key_pressed(Key::ArrowUp) { self.go_up(); }
            if cmd && i.key_pressed(Key::ArrowLeft) { self.go_back(); }
            if cmd && i.key_pressed(Key::ArrowRight) { self.go_forward(); }
            if i.key_pressed(Key::Enter) { self.open_selected(); }
            // Delete selected files
            if i.key_pressed(Key::Backspace) || i.key_pressed(Key::Delete) {
                // Will be handled outside input closure
            }
            if !cmd {
                if i.key_pressed(Key::ArrowUp) {
                    // Move selection up - select item before first selected, or first item
                    let min_selected = self.selected.iter().min().copied();
                    if let Some(idx) = min_selected {
                        if idx > 0 {
                            self.selected.clear();
                            self.selected.insert(idx - 1);
                            self.last_clicked = Some(idx - 1);
                        }
                    }
                }
                if i.key_pressed(Key::ArrowDown) {
                    let max = self.entries.len().saturating_sub(1);
                    let max_selected = self.selected.iter().max().copied();
                    if let Some(idx) = max_selected {
                        if idx < max {
                            self.selected.clear();
                            self.selected.insert(idx + 1);
                            self.last_clicked = Some(idx + 1);
                        }
                    } else if !self.entries.is_empty() {
                        self.selected.clear();
                        self.selected.insert(0);
                        self.last_clicked = Some(0);
                    }
                }
            }
        });

        // Handle delete key outside input closure
        let should_delete = ctx.input(|i| {
            (i.key_pressed(Key::Backspace) || i.key_pressed(Key::Delete)) && !self.selected.is_empty()
        });
        if should_delete {
            self.delete_selected();
        }
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
        // Column headers - render manually to align with data rows
        let total_w = ui.available_width();
        let name_w = total_w - 180.0;
        let header_height = 20.0;

        let (header_rect, header_response) = ui.allocate_exact_size(
            egui::vec2(total_w, header_height),
            egui::Sense::click(),
        );

        if ui.is_rect_visible(header_rect) {
            let painter = ui.painter();

            // Background
            painter.rect_filled(header_rect, 0.0, SlowColors::WHITE);
            painter.rect_stroke(header_rect, 0.0, egui::Stroke::new(1.0, SlowColors::BLACK));

            // Name header
            let name_rect = egui::Rect::from_min_size(
                header_rect.min,
                egui::vec2(name_w, header_height),
            );
            painter.text(
                egui::pos2(name_rect.min.x + 4.0, name_rect.center().y),
                egui::Align2::LEFT_CENTER,
                "name",
                egui::FontId::proportional(12.0),
                SlowColors::BLACK,
            );

            // Size header
            let size_rect = egui::Rect::from_min_size(
                egui::pos2(header_rect.min.x + name_w, header_rect.min.y),
                egui::vec2(80.0, header_height),
            );
            painter.rect_stroke(size_rect, 0.0, egui::Stroke::new(1.0, SlowColors::BLACK));
            painter.text(
                size_rect.center(),
                egui::Align2::CENTER_CENTER,
                "size",
                egui::FontId::proportional(12.0),
                SlowColors::BLACK,
            );

            // Modified header
            let mod_rect = egui::Rect::from_min_size(
                egui::pos2(header_rect.min.x + name_w + 80.0, header_rect.min.y),
                egui::vec2(100.0, header_height),
            );
            painter.rect_stroke(mod_rect, 0.0, egui::Stroke::new(1.0, SlowColors::BLACK));
            painter.text(
                mod_rect.center(),
                egui::Align2::CENTER_CENTER,
                "modified",
                egui::FontId::proportional(12.0),
                SlowColors::BLACK,
            );
        }

        // Handle clicks on headers for sorting
        if header_response.clicked() {
            let mouse_pos = ui.input(|i| i.pointer.interact_pos()).unwrap_or(header_rect.center());
            let click_x = mouse_pos.x - header_rect.min.x;

            if click_x < name_w {
                if self.sort_by == SortBy::Name { self.sort_asc = !self.sort_asc; }
                else { self.sort_by = SortBy::Name; self.sort_asc = true; }
                self.sort_entries();
            } else if click_x < name_w + 80.0 {
                if self.sort_by == SortBy::Size { self.sort_asc = !self.sort_asc; }
                else { self.sort_by = SortBy::Size; self.sort_asc = true; }
                self.sort_entries();
            } else {
                if self.sort_by == SortBy::Modified { self.sort_asc = !self.sort_asc; }
                else { self.sort_by = SortBy::Modified; self.sort_asc = true; }
                self.sort_entries();
            }
        }

        ui.add_space(2.0);

        // Collect entry data to avoid borrow conflict
        let display_entries: Vec<(usize, String, String, String, String, bool, PathBuf)> =
            self.entries.iter().enumerate().map(|(idx, entry)| {
                let icon = if entry.is_dir { "📁".into() } else { file_icon(&entry.name).to_string() };
                let size_str = if entry.is_dir { "—".into() } else { format_size(entry.size) };
                (idx, entry.name.clone(), icon, size_str, entry.modified.clone(), entry.is_dir, entry.path.clone())
            }).collect();

        // Get modifier state for shift/cmd click
        let modifiers = ui.input(|i| i.modifiers);

        // File list
        let mut nav_target: Option<PathBuf> = None;
        let mut open_target: Option<PathBuf> = None;
        let mut click_action: Option<(usize, bool, bool)> = None; // (idx, shift, cmd)

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (idx, name, icon, size_str, modified, is_dir, path) in &display_entries {
                let is_selected = self.selected.contains(idx);
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
                    if is_selected {
                        slowcore::dither::draw_dither_selection(painter, rect);
                    } else if response.hovered() {
                        slowcore::dither::draw_dither_hover(painter, rect);
                    }

                    let text_color = if is_selected { SlowColors::WHITE } else { SlowColors::BLACK };

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

                if response.clicked() {
                    click_action = Some((*idx, modifiers.shift, modifiers.command));
                }
                if response.double_clicked() {
                    if *is_dir {
                        nav_target = Some(path.clone());
                    } else {
                        open_target = Some(path.clone());
                    }
                }
            }
        });

        // Handle click actions after the loop to avoid borrow issues
        if let Some((idx, shift, cmd)) = click_action {
            if shift && self.last_clicked.is_some() {
                // Shift+click: select range from last clicked to current
                let start = self.last_clicked.unwrap();
                let end = idx;
                let (from, to) = if start <= end { (start, end) } else { (end, start) };
                if !cmd {
                    self.selected.clear();
                }
                for i in from..=to {
                    self.selected.insert(i);
                }
            } else if cmd {
                // Cmd+click: toggle selection
                if self.selected.contains(&idx) {
                    self.selected.remove(&idx);
                } else {
                    self.selected.insert(idx);
                }
                self.last_clicked = Some(idx);
            } else {
                // Normal click: select only this item
                self.selected.clear();
                self.selected.insert(idx);
                self.last_clicked = Some(idx);
            }
        }

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
                    if ui.button("new window").clicked() {
                        // Launch a new instance of slowfiles
                        if let Ok(exe) = std::env::current_exe() {
                            let _ = std::process::Command::new(exe)
                                .spawn();
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add_enabled(!self.selected.is_empty(), egui::Button::new("move to trash  ⌫")).clicked() {
                        self.delete_selected();
                        ui.close_menu();
                    }
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
            let info = if self.selected.is_empty() {
                format!("{} items", self.entries.len())
            } else if self.selected.len() == 1 {
                let idx = *self.selected.iter().next().unwrap();
                if let Some(e) = self.entries.get(idx) {
                    format!("{}  —  {}", e.name, if e.is_dir { "folder".into() } else { format_size(e.size) })
                } else { String::new() }
            } else {
                format!("{} items selected", self.selected.len())
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
            egui::Window::new("about slowFiles")
                .collapsible(false)
                .resizable(false)
                .default_width(300.0)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("slowFiles");
                        ui.label("version 0.1.0");
                        ui.add_space(8.0);
                        ui.label("file manager for slowOS");
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label("features:");
                    ui.label("  browse, sort, multi-select files");
                    ui.label("  navigate with ⌘+arrows");
                    ui.add_space(4.0);
                    ui.label("frameworks:");
                    ui.label("  egui/eframe (MIT), chrono (MIT)");
                    ui.label("  open (MIT)");
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
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
