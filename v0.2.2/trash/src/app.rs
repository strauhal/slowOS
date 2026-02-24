//! slowTrash — trash bin for the slow computer
//!
//! Files deleted from other slow computer apps land here.
//! Users can restore files to their original location or permanently delete them.

use chrono::Local;
use egui::{Context, Key};
use serde::{Deserialize, Serialize};
use slowcore::repaint::RepaintController;
use slowcore::storage::config_dir;
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::status_bar;
use std::path::PathBuf;

/// Metadata for a trashed file
#[derive(Clone, Debug, Serialize, Deserialize)]
struct TrashEntry {
    /// Original filename
    original_name: String,
    /// Original full path (for restore)
    original_path: PathBuf,
    /// Path inside trash directory
    trash_path: PathBuf,
    /// When the file was trashed
    trashed_at: String,
    /// File size in bytes
    size: u64,
}

/// Manifest tracking all trashed files
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct TrashManifest {
    entries: Vec<TrashEntry>,
}

impl TrashManifest {
    fn load(path: &std::path::Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self, path: &std::path::Path) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

#[allow(dead_code)]
pub struct TrashApp {
    manifest: TrashManifest,
    manifest_path: PathBuf,
    trash_dir: PathBuf,
    selected: Option<usize>,
    show_about: bool,
    show_confirm_empty: bool,
    show_confirm_delete: bool,
    message: Option<String>,
    repaint: RepaintController,
}

#[allow(dead_code)]
impl TrashApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let trash_dir = trash_dir();
        let _ = std::fs::create_dir_all(&trash_dir);
        let manifest_path = trash_dir.join("manifest.json");
        let mut manifest = TrashManifest::load(&manifest_path);

        // Prune entries whose trash files no longer exist
        manifest.entries.retain(|e| e.trash_path.exists());

        let app = Self {
            manifest,
            manifest_path,
            trash_dir,
            selected: None,
            show_about: false,
            show_confirm_empty: false,
            show_confirm_delete: false,
            message: None,
            repaint: RepaintController::new(),
        };
        app.save_manifest();
        app
    }

    fn save_manifest(&self) {
        self.manifest.save(&self.manifest_path);
    }

    fn refresh(&mut self) {
        self.manifest.entries.retain(|e| e.trash_path.exists());
        self.save_manifest();
        if let Some(sel) = self.selected {
            if sel >= self.manifest.entries.len() {
                self.selected = None;
            }
        }
    }

    fn restore_selected(&mut self) {
        if let Some(idx) = self.selected {
            if idx < self.manifest.entries.len() {
                let entry = &self.manifest.entries[idx];
                let dest = &entry.original_path;

                // Ensure parent directory exists
                if let Some(parent) = dest.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }

                match std::fs::rename(&entry.trash_path, dest) {
                    Ok(()) => {
                        self.message = Some(format!("restored: {}", entry.original_name));
                        self.manifest.entries.remove(idx);
                        self.selected = None;
                        self.save_manifest();
                    }
                    Err(e) => {
                        // rename fails across filesystems; fall back to copy+delete
                        match std::fs::copy(&entry.trash_path, dest) {
                            Ok(_) => {
                                let _ = std::fs::remove_file(&entry.trash_path);
                                self.message = Some(format!("restored: {}", entry.original_name));
                                self.manifest.entries.remove(idx);
                                self.selected = None;
                                self.save_manifest();
                            }
                            Err(_) => {
                                self.message = Some(format!("restore failed: {}", e));
                            }
                        }
                    }
                }
            }
        }
    }

    fn delete_selected_permanently(&mut self) {
        if let Some(idx) = self.selected {
            if idx < self.manifest.entries.len() {
                let entry = &self.manifest.entries[idx];
                if entry.trash_path.is_dir() {
                    let _ = std::fs::remove_dir_all(&entry.trash_path);
                } else {
                    let _ = std::fs::remove_file(&entry.trash_path);
                }
                let name = entry.original_name.clone();
                self.manifest.entries.remove(idx);
                self.selected = None;
                self.save_manifest();
                self.message = Some(format!("permanently deleted: {}", name));
            }
        }
    }

    fn empty_trash(&mut self) {
        for entry in &self.manifest.entries {
            if entry.trash_path.is_dir() {
                let _ = std::fs::remove_dir_all(&entry.trash_path);
            } else {
                let _ = std::fs::remove_file(&entry.trash_path);
            }
        }
        self.manifest.entries.clear();
        self.selected = None;
        self.save_manifest();
        self.message = Some("trash emptied".to_string());
    }

    fn total_size(&self) -> u64 {
        self.manifest.entries.iter().map(|e| e.size).sum()
    }

    fn format_size(bytes: u64) -> String {
        if bytes < 1024 {
            format!("{} B", bytes)
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
}

impl eframe::App for TrashApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.repaint.begin_frame(ctx);
        slowcore::theme::consume_special_keys(ctx);
        // Keyboard shortcuts
        ctx.input(|i| {
            if i.modifiers.command && i.key_pressed(Key::R) {
                self.restore_selected();
            }
            if i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace) {
                if self.selected.is_some() {
                    self.show_confirm_delete = true;
                }
            }
            if i.key_pressed(Key::ArrowDown) {
                let count = self.manifest.entries.len();
                if count > 0 {
                    self.selected = Some(self.selected.map(|s| (s + 1).min(count - 1)).unwrap_or(0));
                }
            }
            if i.key_pressed(Key::ArrowUp) {
                if let Some(s) = self.selected {
                    self.selected = Some(s.saturating_sub(1));
                }
            }
        });

        // Menu bar
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            menu_bar(ui, |ui| {
                ui.menu_button("file", |ui| {
                    if ui.button("restore selected  ⌘r").clicked() {
                        self.restore_selected();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("empty trash").clicked() {
                        self.show_confirm_empty = true;
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

        // Toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let has_sel = self.selected.is_some();
                if ui.add_enabled(has_sel, egui::Button::new("restore")).clicked() {
                    self.restore_selected();
                }
                if ui.add_enabled(has_sel, egui::Button::new("delete permanently")).clicked() {
                    self.show_confirm_delete = true;
                }
                ui.separator();
                if ui.add_enabled(!self.manifest.entries.is_empty(), egui::Button::new("empty trash")).clicked() {
                    self.show_confirm_empty = true;
                }
                if ui.button("refresh").clicked() {
                    self.refresh();
                }
            });
        });

        // Status bar
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let count = self.manifest.entries.len();
            let size = Self::format_size(self.total_size());
            let msg = self.message.as_deref().unwrap_or("");
            status_bar(ui, &format!("{} items  |  {}  {}", count, size, msg));
        });

        // Main content: file list
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(4.0)))
            .show(ctx, |ui| {
                if self.manifest.entries.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(60.0);
                        ui.label("trash is empty");
                    });
                } else {
                    // Header: name, date, size (no folder path)
                    ui.horizontal(|ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), 20.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                let w = ui.available_width();
                                ui.allocate_ui(egui::vec2(w * 0.50, 20.0), |ui| {
                                    ui.label(egui::RichText::new("name").strong());
                                });
                                ui.allocate_ui(egui::vec2(w * 0.30, 20.0), |ui| {
                                    ui.label(egui::RichText::new("date trashed").strong());
                                });
                                ui.allocate_ui(egui::vec2(w * 0.20, 20.0), |ui| {
                                    ui.label(egui::RichText::new("size").strong());
                                });
                            },
                        );
                    });
                    ui.separator();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let mut clicked_idx = None;
                        let mut restore_idx = None;
                        for (idx, entry) in self.manifest.entries.iter().enumerate() {
                            let is_selected = self.selected == Some(idx);
                            let bg = if is_selected { SlowColors::BLACK } else { SlowColors::WHITE };
                            let fg = if is_selected { SlowColors::WHITE } else { SlowColors::BLACK };

                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 22.0),
                                egui::Sense::click(),
                            );

                            if response.clicked() {
                                clicked_idx = Some(idx);
                            }
                            if response.double_clicked() {
                                restore_idx = Some(idx);
                            }

                            let painter = ui.painter();
                            painter.rect_filled(rect, 0.0, bg);

                            let w = rect.width();
                            let y = rect.center().y;

                            // Name
                            painter.text(
                                egui::Pos2::new(rect.min.x + 4.0, y),
                                egui::Align2::LEFT_CENTER,
                                &entry.original_name,
                                egui::FontId::proportional(13.0),
                                fg,
                            );
                            // Date
                            painter.text(
                                egui::Pos2::new(rect.min.x + w * 0.50, y),
                                egui::Align2::LEFT_CENTER,
                                &entry.trashed_at,
                                egui::FontId::proportional(12.0),
                                fg,
                            );
                            // Size
                            painter.text(
                                egui::Pos2::new(rect.min.x + w * 0.80, y),
                                egui::Align2::LEFT_CENTER,
                                &Self::format_size(entry.size),
                                egui::FontId::proportional(12.0),
                                fg,
                            );
                        }
                        if let Some(idx) = clicked_idx { self.selected = Some(idx); }
                        if let Some(idx) = restore_idx {
                            self.selected = Some(idx);
                            self.restore_selected();
                        }
                    });
                }
            });

        // Confirm empty dialog
        if self.show_confirm_empty {
            let resp = egui::Window::new("empty trash")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("permanently delete all items in trash?");
                    ui.label("this cannot be undone.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("cancel").clicked() {
                            self.show_confirm_empty = false;
                        }
                        if ui.button("empty trash").clicked() {
                            self.empty_trash();
                            self.show_confirm_empty = false;
                        }
                    });
                });
            if let Some(r) = &resp { slowcore::dither::draw_window_shadow(ctx, r.response.rect); }
        }

        // Confirm single delete dialog
        if self.show_confirm_delete {
            let name = self.selected
                .and_then(|i| self.manifest.entries.get(i))
                .map(|e| e.original_name.clone())
                .unwrap_or_default();
            let resp = egui::Window::new("delete permanently")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label(format!("permanently delete \"{}\"?", name));
                    ui.label("this cannot be undone.");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("cancel").clicked() {
                            self.show_confirm_delete = false;
                        }
                        if ui.button("delete").clicked() {
                            self.delete_selected_permanently();
                            self.show_confirm_delete = false;
                        }
                    });
                });
            if let Some(r) = &resp { slowcore::dither::draw_window_shadow(ctx, r.response.rect); }
        }

        // About dialog
        if self.show_about {
            let screen = ctx.screen_rect();
            let max_h = (screen.height() - 60.0).max(120.0);
            let resp = egui::Window::new("about trash")
                .collapsible(false)
                .resizable(false)
                .default_width(300.0)
                .max_height(max_h)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().max_height(max_h - 50.0).show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.heading("trash");
                            ui.label("version 0.2.2");
                            ui.add_space(8.0);
                            ui.label("trash bin for slowOS");
                        });
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(4.0);
                        ui.label("features:");
                        ui.label("  view deleted items");
                        ui.label("  restore or permanently delete");
                        ui.label("  empty all trash");
                        ui.add_space(4.0);
                        ui.label("location: ~/.local/share/Trash");
                        ui.add_space(4.0);
                        ui.label("frameworks:");
                        ui.label("  egui/eframe (MIT), chrono (MIT)");
                        ui.add_space(8.0);
                        ui.vertical_centered(|ui| {
                            if ui.button("ok").clicked() { self.show_about = false; }
                        });
                    });
                });
            if let Some(r) = &resp { slowcore::dither::draw_window_shadow(ctx, r.response.rect); }
        }
        self.repaint.end_frame(ctx);
    }
}

/// Get the trash directory
pub fn trash_dir() -> PathBuf {
    config_dir("trash").join("files")
}

/// Move a file to the slow computer trash.
/// Called by other apps to trash files instead of deleting them.
/// Returns Ok(()) on success.
#[allow(dead_code)]
pub fn move_to_trash(source: &std::path::Path) -> Result<(), std::io::Error> {
    let trash = trash_dir();
    std::fs::create_dir_all(&trash)?;

    let filename = source.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".into());

    // Generate unique name if collision
    let mut dest = trash.join(&filename);
    let mut counter = 1u32;
    while dest.exists() {
        let stem = source.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".into());
        let ext = source.extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        dest = trash.join(format!("{} ({}){}", stem, counter, ext));
        counter += 1;
    }

    // Get file size
    let size = std::fs::metadata(source).map(|m| m.len()).unwrap_or(0);

    // Move file
    std::fs::rename(source, &dest).or_else(|_| {
        // Cross-filesystem: copy then delete
        if source.is_dir() {
            // For directories, just note we can't easily copy
            Err(std::io::Error::new(std::io::ErrorKind::Other, "cannot trash directory across filesystems"))
        } else {
            std::fs::copy(source, &dest)?;
            std::fs::remove_file(source)
        }
    })?;

    // Update manifest
    let manifest_path = config_dir("trash").join("files").join("manifest.json");
    let mut manifest = TrashManifest::load(&manifest_path);
    manifest.entries.push(TrashEntry {
        original_name: filename,
        original_path: source.to_path_buf(),
        trash_path: dest,
        trashed_at: Local::now().format("%Y-%m-%d %H:%M").to_string(),
        size,
    });
    manifest.save(&manifest_path);

    Ok(())
}

/// Restore a file from trash to its original location.
/// Searches the manifest for a file with the given original path.
#[allow(dead_code)]
pub fn restore_from_trash(original_path: &std::path::Path) -> Result<(), std::io::Error> {
    let manifest_path = config_dir("trash").join("files").join("manifest.json");
    let mut manifest = TrashManifest::load(&manifest_path);

    // Find the entry with matching original path
    let idx = manifest.entries.iter().position(|e| e.original_path == original_path);

    if let Some(idx) = idx {
        let entry = manifest.entries.remove(idx);

        // Ensure parent directory exists
        if let Some(parent) = entry.original_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Try to move the file back
        std::fs::rename(&entry.trash_path, &entry.original_path).or_else(|_| {
            // Cross-filesystem: copy then delete
            if entry.trash_path.is_dir() {
                Err(std::io::Error::new(std::io::ErrorKind::Other, "cannot restore directory across filesystems"))
            } else {
                std::fs::copy(&entry.trash_path, &entry.original_path)?;
                std::fs::remove_file(&entry.trash_path)
            }
        })?;

        manifest.save(&manifest_path);
        Ok(())
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::NotFound, "file not found in trash"))
    }
}
