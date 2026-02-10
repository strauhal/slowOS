//! SlowNote - simple note-taking with a sidebar list

use chrono::{Local, NaiveDateTime};
use egui::{Context, Key, Stroke, Vec2};
use serde::{Deserialize, Serialize};
use slowcore::storage::config_dir;
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::status_bar;

/// Move note data to the slow computer trash as a .txt file
fn trash_note(note: &Note) {
    let tmp_dir = std::env::temp_dir().join("slownote_trash");
    let _ = std::fs::create_dir_all(&tmp_dir);
    let safe_title: String = note.title.chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let filename = format!("{}_{}.txt", safe_title, note.id);
    let tmp_path = tmp_dir.join(&filename);
    let content = format!("title: {}\ncreated: {}\nmodified: {}\n\n{}", note.title, note.created, note.modified, note.body);
    if std::fs::write(&tmp_path, &content).is_ok() {
        let _ = trash::move_to_trash(&tmp_path);
    }
}

/// Check for notes that have been restored from trash and re-import them
fn check_restored_notes(store: &mut NoteStore) {
    let tmp_dir = std::env::temp_dir().join("slownote_trash");
    if !tmp_dir.exists() {
        return;
    }

    if let Ok(entries) = std::fs::read_dir(&tmp_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("txt") {
                continue;
            }

            // Try to parse the note file
            if let Ok(content) = std::fs::read_to_string(&path) {
                let mut title = String::new();
                let mut created = String::new();
                let mut modified = String::new();
                let mut body = String::new();
                let mut in_body = false;

                for line in content.lines() {
                    if in_body {
                        if !body.is_empty() {
                            body.push('\n');
                        }
                        body.push_str(line);
                    } else if line.is_empty() {
                        in_body = true;
                    } else if let Some(rest) = line.strip_prefix("title: ") {
                        title = rest.to_string();
                    } else if let Some(rest) = line.strip_prefix("created: ") {
                        created = rest.to_string();
                    } else if let Some(rest) = line.strip_prefix("modified: ") {
                        modified = rest.to_string();
                    }
                }

                if !title.is_empty() {
                    // Check if note with this title already exists
                    let exists = store.notes.iter().any(|n| n.title == title);
                    if !exists {
                        // Generate new ID
                        let id = Local::now().timestamp_millis() as u64;
                        store.notes.insert(0, Note {
                            id,
                            title,
                            body,
                            created: if created.is_empty() {
                                Local::now().format("%Y-%m-%d %H:%M").to_string()
                            } else {
                                created
                            },
                            modified: if modified.is_empty() {
                                Local::now().format("%Y-%m-%d %H:%M").to_string()
                            } else {
                                modified
                            },
                            pinned: false,
                        });
                        store.save();
                    }
                    // Remove the file after importing (or if it already exists)
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Note {
    pub id: u64,
    pub title: String,
    pub body: String,
    pub created: String,
    pub modified: String,
    pub pinned: bool,
}

impl Note {
    fn new() -> Self {
        let now = Local::now().format("%Y-%m-%d %H:%M").to_string();
        Self {
            id: Local::now().timestamp_millis() as u64,
            title: "new note".into(),
            body: String::new(),
            created: now.clone(),
            modified: now,
            pinned: false,
        }
    }

    fn preview(&self) -> String {
        let first_line = self.body.lines().next().unwrap_or("");
        if first_line.len() > 60 {
            format!("{}...", &first_line[..60])
        } else if first_line.is_empty() {
            "empty note".into()
        } else {
            first_line.to_string()
        }
    }

    fn touch(&mut self) {
        self.modified = Local::now().format("%Y-%m-%d %H:%M").to_string();
    }
}

#[derive(Serialize, Deserialize, Default)]
struct NoteStore {
    notes: Vec<Note>,
}

impl NoteStore {
    fn load() -> Self {
        let path = config_dir("slownote").join("notes.json");
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        let path = config_dir("slownote").join("notes.json");
        if let Some(p) = path.parent() { let _ = std::fs::create_dir_all(p); }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }
}

pub struct SlowNoteApp {
    store: NoteStore,
    selected: Option<usize>,
    search_query: String,
    editing_title: bool,
    show_about: bool,
}

impl SlowNoteApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut store = NoteStore::load();
        // Check for notes restored from trash
        check_restored_notes(&mut store);
        let selected = if store.notes.is_empty() { None } else { Some(0) };
        Self { store, selected, search_query: String::new(), editing_title: false, show_about: false }
    }

    fn new_note(&mut self) {
        let note = Note::new();
        self.store.notes.insert(0, note);
        self.selected = Some(0);
        self.editing_title = true;
        self.store.save();
    }

    fn delete_note(&mut self) {
        if let Some(idx) = self.selected {
            if idx < self.store.notes.len() {
                let note = &self.store.notes[idx];
                trash_note(note);
                self.store.notes.remove(idx);
                if self.store.notes.is_empty() {
                    self.selected = None;
                } else {
                    self.selected = Some(idx.min(self.store.notes.len() - 1));
                }
                self.store.save();
            }
        }
    }

    fn filtered_indices(&self) -> Vec<usize> {
        let q = self.search_query.to_lowercase();
        self.store.notes.iter().enumerate()
            .filter(|(_, n)| {
                q.is_empty() ||
                n.title.to_lowercase().contains(&q) ||
                n.body.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn sorted_indices(&self) -> Vec<usize> {
        let mut indices = self.filtered_indices();
        indices.sort_by(|&a, &b| {
            let na = &self.store.notes[a];
            let nb = &self.store.notes[b];
            nb.pinned.cmp(&na.pinned).then(nb.modified.cmp(&na.modified))
        });
        indices
    }

    fn handle_keys(&mut self, ctx: &Context) {
        slowcore::theme::consume_special_keys(ctx);
        ctx.input(|i| {
            let cmd = i.modifiers.command;
            if cmd && i.key_pressed(Key::N) { self.new_note(); }
            if cmd && i.key_pressed(Key::Backspace) { self.delete_note(); }
        });
    }

    fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("🔍");
            ui.text_edit_singleline(&mut self.search_query);
        });
        ui.separator();

        if ui.button("+ New Note").clicked() { self.new_note(); }
        ui.add_space(4.0);

        let indices = self.sorted_indices();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for &idx in &indices {
                let note = &self.store.notes[idx];
                let is_selected = self.selected == Some(idx);
                let pin_mark = if note.pinned { "📌 " } else { "" };
                let label = format!("{}{}", pin_mark, note.title);

                let response = ui.selectable_label(is_selected, &label);
                if response.clicked() {
                    self.selected = Some(idx);
                    self.editing_title = false;
                }

                // Show preview under title
                ui.label(egui::RichText::new(note.preview()).small().color(SlowColors::BLACK));
                ui.label(egui::RichText::new(&note.modified).small().color(SlowColors::BLACK));
                ui.add_space(6.0);
            }
        });
    }

    fn render_editor(&mut self, ui: &mut egui::Ui) {
        let idx = match self.selected {
            Some(i) if i < self.store.notes.len() => i,
            _ => {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label("no note selected");
                    if ui.button("create note").clicked() { self.new_note(); }
                });
                return;
            }
        };

        let note = &mut self.store.notes[idx];

        // Title
        ui.horizontal(|ui| {
            let r = ui.text_edit_singleline(&mut note.title);
            if r.changed() { note.touch(); }

            let pin_text = if note.pinned { "unpin" } else { "pin" };
            if ui.button(pin_text).clicked() {
                note.pinned = !note.pinned;
                note.touch();
            }
        });

        ui.separator();

        // Body
        let available = ui.available_size();
        let response = ui.add_sized(
            available,
            egui::TextEdit::multiline(&mut note.body)
                .font(egui::FontId::proportional(14.0))
                .desired_width(available.x)
        );
        if response.changed() {
            note.touch();
            self.store.save();
        }
    }
}

impl eframe::App for SlowNoteApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.handle_keys(ctx);

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            menu_bar(ui, |ui| {
                ui.menu_button("file", |ui| {
                    if ui.button("New Note   ⌘N").clicked() { self.new_note(); ui.close_menu(); }
                    if ui.button("Delete     ⌘⌫").clicked() { self.delete_note(); ui.close_menu(); }
                });
                ui.menu_button("help", |ui| {
                    if ui.button("about").clicked() { self.show_about = true; ui.close_menu(); }
                });
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let count = self.store.notes.len();
            let chars = self.selected
                .and_then(|i| self.store.notes.get(i))
                .map(|n| n.body.len())
                .unwrap_or(0);
            status_bar(ui, &format!("{} notes  |  {} characters", count, chars));
        });

        egui::SidePanel::left("sidebar").default_width(200.0).show(ctx, |ui| {
            self.render_sidebar(ui);
        });

        egui::CentralPanel::default().frame(
            egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(8.0))
        ).show(ctx, |ui| {
            self.render_editor(ui);
        });

        if self.show_about {
            egui::Window::new("about slowNotes")
                .collapsible(false)
                .resizable(false)
                .default_width(300.0)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("slowNotes");
                        ui.label("version 0.1.0");
                        ui.add_space(8.0);
                        ui.label("simple note-taking app");
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label("features:");
                    ui.label("  create, search, pin notes");
                    ui.label("  deleted notes go to trash");
                    ui.add_space(4.0);
                    ui.label("storage: JSON in config directory");
                    ui.add_space(4.0);
                    ui.label("frameworks:");
                    ui.label("  egui/eframe (MIT), chrono (MIT)");
                    ui.label("  serde (MIT/Apache-2.0)");
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        if ui.button("ok").clicked() { self.show_about = false; }
                    });
                });
        }
    }
}
