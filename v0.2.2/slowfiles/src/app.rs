//! SlowFiles - file explorer

use egui::{ColorImage, Context, Key, Pos2, Rect, TextureHandle, TextureOptions, Vec2};
use slowcore::repaint::RepaintController;
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::status_bar;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{SystemTime, Instant};
use trash::{move_to_trash, restore_from_trash};

/// System folders that cannot be deleted
const SYSTEM_FOLDERS: &[&str] = &[
    "Documents", "documents",
    "Pictures", "pictures",
    "Music", "music",
    "Books", "books",
    "MIDI", "midi",
    "Apps", "apps",
    "Desktop", "desktop",
    "Downloads", "downloads",
    "slowLibrary", "slowlibrary",
    "slowMuseum", "slowmuseum",
    "compositions", "Compositions",
    "Kimiko Ishizaka - J.S. Bach- -Open- Goldberg Variations- BWV 988 (Piano)",
];

struct FileEntry {
    name: String,
    name_lower: String,
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
    view_mode: ViewMode,
    history: Vec<PathBuf>,
    history_idx: usize,
    show_about: bool,
    show_shortcuts: bool,
    error_msg: Option<String>,
    /// Dragging state: paths of files being dragged
    dragging: Option<Vec<PathBuf>>,
    /// Drag preview info: (icon_key, name, count)
    drag_preview: Option<(String, String, usize)>,
    /// Index of folder being hovered during drag
    drag_hover_idx: Option<usize>,
    /// File type icon textures (keyed by category: "folder", "text", "image", etc.)
    file_icons: HashMap<String, TextureHandle>,
    icons_loaded: bool,
    /// Opening animation state: (start_rect, progress 0..1)
    open_anim: Option<(Rect, f32)>,
    /// Last frame time for animation delta
    last_frame: Instant,
    /// Stack of deleted file paths for undo (most recent last)
    deleted_paths: Vec<PathBuf>,
    /// Show new folder dialog
    show_new_folder: bool,
    /// New folder name input
    new_folder_name: String,
    /// Focus text field on next frame
    focus_new_folder_field: bool,
    /// Marquee selection start position (screen coords)
    marquee_start: Option<Pos2>,
    /// Item rects from last render (for marquee hit testing)
    item_rects: Vec<(usize, Rect)>,
    /// Thumbnail cache for image files (keyed by path string)
    thumbnails: HashMap<String, TextureHandle>,
    /// Paths that failed to load as thumbnails (don't retry)
    thumbnail_failed: HashSet<String>,
    repaint: RepaintController,
}

#[derive(Clone, Copy, PartialEq)]
enum SortBy { Name, Size, Modified }

#[derive(Clone, Copy, PartialEq)]
enum ViewMode { Icons, List }

#[allow(dead_code)]
impl SlowFilesApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::new_with_dir(_cc, None)
    }

    pub fn new_with_dir(_cc: &eframe::CreationContext<'_>, start_dir: Option<PathBuf>) -> Self {
        let dir = start_dir
            .filter(|p| p.is_dir())
            .unwrap_or_else(|| dirs_home().unwrap_or_else(|| PathBuf::from("/")));
        let mut app = Self {
            current_dir: dir.clone(),
            entries: Vec::new(),
            selected: HashSet::new(),
            last_clicked: None,
            path_input: dir.to_string_lossy().to_string(),
            show_hidden: false,
            sort_by: SortBy::Name,
            sort_asc: true,
            view_mode: ViewMode::Icons,
            history: vec![dir],
            history_idx: 0,
            show_about: false,
            show_shortcuts: false,
            error_msg: None,
            dragging: None,
            drag_preview: None,
            drag_hover_idx: None,
            file_icons: HashMap::new(),
            icons_loaded: false,
            open_anim: None,
            last_frame: Instant::now(),
            deleted_paths: Vec::new(),
            show_new_folder: false,
            new_folder_name: String::new(),
            focus_new_folder_field: false,
            marquee_start: None,
            item_rects: Vec::new(),
            thumbnails: HashMap::new(),
            thumbnail_failed: HashSet::new(),
            repaint: RepaintController::new(),
        };
        app.refresh();
        app
    }

    /// Generate a 32x32 thumbnail for an image file
    fn get_or_create_thumbnail(&mut self, ctx: &Context, path: &PathBuf) -> Option<TextureHandle> {
        let key = path.to_string_lossy().to_string();

        // Check if already cached
        if let Some(tex) = self.thumbnails.get(&key) {
            return Some(tex.clone());
        }

        // Skip if previously failed
        if self.thumbnail_failed.contains(&key) {
            return None;
        }

        // Evict old thumbnails when cache gets large
        if self.thumbnails.len() >= 64 {
            self.thumbnails.clear();
        }

        // Try to load and create thumbnail
        if let Ok(bytes) = std::fs::read(path) {
            if let Ok(img) = image::load_from_memory(&bytes) {
                // Resize to 32x32 with aspect ratio preservation
                let thumb = img.thumbnail(32, 32);
                let rgba = thumb.to_rgba8();
                let (w, h) = rgba.dimensions();
                let color_image = ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    rgba.as_raw(),
                );
                let texture = ctx.load_texture(
                    format!("thumb_{}", key),
                    color_image,
                    TextureOptions::NEAREST,
                );
                self.thumbnails.insert(key, texture.clone());
                return Some(texture);
            }
        }

        // Mark as failed
        self.thumbnail_failed.insert(key);
        None
    }

    fn create_new_folder(&mut self) {
        let name = self.new_folder_name.trim();
        if name.is_empty() {
            return;
        }
        let new_path = self.current_dir.join(name);
        if new_path.exists() {
            self.error_msg = Some(format!("'{}' already exists", name));
            return;
        }
        match std::fs::create_dir(&new_path) {
            Ok(()) => {
                self.refresh();
                self.new_folder_name.clear();
                self.show_new_folder = false;
            }
            Err(e) => {
                self.error_msg = Some(format!("Failed to create folder: {}", e));
            }
        }
    }

    fn move_files_to_folder(&mut self, paths: &[PathBuf], dest_dir: &PathBuf) {
        let mut blocked_names: Vec<String> = Vec::new();
        for path in paths {
            if path == dest_dir || path.parent() == Some(dest_dir.as_path()) {
                continue; // Skip if already in destination
            }
            // Block moving system folders
            if Self::is_system_folder(path) {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    blocked_names.push(name.to_string());
                }
                continue;
            }
            if let Some(name) = path.file_name() {
                let dest_path = dest_dir.join(name);
                if let Err(e) = std::fs::rename(path, &dest_path) {
                    self.error_msg = Some(format!("Failed to move file: {}", e));
                    return;
                }
            }
        }
        if !blocked_names.is_empty() {
            self.error_msg = Some(format!(
                "Cannot move system folder(s): {}",
                blocked_names.join(", ")
            ));
        }
        self.refresh();
    }

    fn ensure_file_icons(&mut self, ctx: &Context) {
        if self.icons_loaded {
            return;
        }
        self.icons_loaded = true;

        let icon_data: &[(&str, &[u8])] = &[
            ("folder", include_bytes!("../../icons/icons_files.png")),
            ("text",   include_bytes!("../../icons/file_icons/icons_txt_file.png")),
            ("image",  include_bytes!("../../icons/file_icons/icons_imagefile.png")),
            ("midi",   include_bytes!("../../icons/file_icons/icons_midi_file.png")),
            ("audio",  include_bytes!("../../icons/file_icons/icons_mp3_wav.png")),
            ("epub",   include_bytes!("../../icons/file_icons/icons_epub.png")),
            ("sheets", include_bytes!("../../icons/file_icons/icons_sheets_file.png")),
            ("slides", include_bytes!("../../icons/file_icons/icons_slides_file.png")),
            ("latex",  include_bytes!("../../icons/file_icons/icons_latex_file.png")),
        ];

        for (key, bytes) in icon_data {
            if let Ok(img) = image::load_from_memory(bytes) {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let color_image = ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    rgba.as_raw(),
                );
                let texture = ctx.load_texture(
                    format!("file_icon_{}", key),
                    color_image,
                    TextureOptions::NEAREST,
                );
                self.file_icons.insert(key.to_string(), texture);
            }
        }
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
            self.apply_history_nav();
        }
    }

    fn go_forward(&mut self) {
        if self.history_idx < self.history.len() - 1 {
            self.history_idx += 1;
            self.apply_history_nav();
        }
    }

    fn apply_history_nav(&mut self) {
        let path = self.history[self.history_idx].clone();
        self.current_dir = path.clone();
        self.path_input = path.to_string_lossy().to_string();
        self.selected.clear();
        self.last_clicked = None;
        self.refresh();
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

                    // Use file_type() from DirEntry (no extra stat on most platforms)
                    let ft = entry.file_type().ok();
                    let is_dir = ft.as_ref().map(|t| t.is_dir()).unwrap_or(false);

                    // Only stat for size/modified (lazy — skip for directories)
                    let (size, modified) = if is_dir {
                        (0, String::new())
                    } else {
                        let meta = entry.metadata().ok();
                        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                        let modified = meta.as_ref()
                            .and_then(|m| m.modified().ok())
                            .map(format_time)
                            .unwrap_or_default();
                        (size, modified)
                    };

                    let name_lower = name.to_lowercase();
                    self.entries.push(FileEntry {
                        name,
                        name_lower,
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
                    SortBy::Name => a.name_lower.cmp(&b.name_lower),
                    SortBy::Size => a.size.cmp(&b.size),
                    SortBy::Modified => a.modified.cmp(&b.modified),
                };
                if self.sort_asc { cmp } else { cmp.reverse() }
            })
        });
    }

    fn open_selected(&mut self) {
        self.open_selected_with_rect(None);
    }

    fn open_selected_with_rect(&mut self, icon_rect: Option<Rect>) {
        // Open the first selected item (or navigate if it's a directory)
        if let Some(&idx) = self.selected.iter().next() {
            if let Some(entry) = self.entries.get(idx) {
                if entry.is_dir {
                    self.navigate(entry.path.clone());
                } else {
                    if let Some(r) = icon_rect {
                        self.open_anim = Some((r, 0.0));
                    }
                    open_in_slow_app(&entry.path);
                }
            }
        }
    }

    /// Check if a path is a protected system folder
    fn is_system_folder(path: &PathBuf) -> bool {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            // Check if this is a system folder in the home directory
            if let Some(home) = dirs_home() {
                if path.parent() == Some(&home) {
                    return SYSTEM_FOLDERS.contains(&name);
                }
            }
        }
        false
    }

    fn delete_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        // Collect paths to delete (sorted descending so indices don't shift)
        let mut indices: Vec<usize> = self.selected.iter().copied().collect();
        indices.sort_by(|a, b| b.cmp(a));

        let mut deleted_in_batch: Vec<PathBuf> = Vec::new();
        let mut blocked_names: Vec<String> = Vec::new();

        for idx in indices {
            if let Some(entry) = self.entries.get(idx) {
                // Check if this is a protected system folder
                if Self::is_system_folder(&entry.path) {
                    blocked_names.push(entry.name.clone());
                    continue;
                }

                // Track the path before deletion for potential undo
                let path = entry.path.clone();
                if move_to_trash(&path).is_ok() {
                    deleted_in_batch.push(path);
                }
            }
        }

        // Store deleted paths for undo (most recent batch)
        if !deleted_in_batch.is_empty() {
            self.deleted_paths = deleted_in_batch;
        }

        // Show error if system folders were blocked
        if !blocked_names.is_empty() {
            self.error_msg = Some(format!(
                "Cannot delete system folder(s): {}",
                blocked_names.join(", ")
            ));
        }

        self.selected.clear();
        self.last_clicked = None;
        self.refresh();
    }

    /// Undo the last delete operation by restoring from trash
    fn undo_delete(&mut self) {
        if self.deleted_paths.is_empty() {
            return;
        }

        // Try to restore each file from trash
        let mut restored_count = 0;
        for path in self.deleted_paths.drain(..) {
            if restore_from_trash(&path).is_ok() {
                restored_count += 1;
            }
        }

        if restored_count > 0 {
            self.error_msg = Some(format!("Restored {} item(s)", restored_count));
        }

        self.refresh();
    }

    fn handle_keys(&mut self, ctx: &Context) {
        slowcore::theme::consume_special_keys(ctx);
        ctx.input(|i| {
            let cmd = i.modifiers.command;
            if cmd && i.key_pressed(Key::ArrowUp) { self.go_up(); }
            if cmd && i.key_pressed(Key::ArrowLeft) { self.go_back(); }
            if cmd && i.key_pressed(Key::ArrowRight) { self.go_forward(); }
            if cmd && i.modifiers.shift && i.key_pressed(Key::N) {
                self.show_new_folder = true;
                self.focus_new_folder_field = true;
                self.new_folder_name = "untitled folder".to_string();
            }
            if i.key_pressed(Key::Enter) { self.open_selected(); }
            // Delete selected files
            if i.key_pressed(Key::Backspace) || i.key_pressed(Key::Delete) {
                // Will be handled outside input closure
            }
            if !cmd {
                // View mode shortcuts: 1 = icons, 2 = list
                if i.key_pressed(Key::Num1) { self.view_mode = ViewMode::Icons; }
                if i.key_pressed(Key::Num2) { self.view_mode = ViewMode::List; }

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

        // Handle undo (Cmd+Z)
        let should_undo = ctx.input(|i| i.modifiers.command && i.key_pressed(Key::Z));
        if should_undo {
            self.undo_delete();
        }
    }

    fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        let primary_released = ui.input(|i| i.pointer.primary_released());
        let is_dragging = self.dragging.is_some();
        let mut drop_to_back = false;
        let mut drop_to_fwd = false;
        let mut drop_to_up = false;

        ui.horizontal(|ui| {
            // Back button - droppable when dragging and history available
            let back_can_drop = is_dragging && self.history_idx > 0;
            let back_btn = ui.button("◀").on_hover_text(if back_can_drop {
                "drop to move here"
            } else {
                "back"
            });

            // Darken back button when hovered during drag (dither effect)
            if back_can_drop && back_btn.hovered() {
                let painter = ui.painter();
                slowcore::dither::draw_dither_selection(painter, back_btn.rect);
            }

            if back_btn.clicked() { self.go_back(); }
            if back_can_drop && back_btn.hovered() && primary_released {
                drop_to_back = true;
            }

            // Forward button
            let fwd_can_drop = is_dragging && self.history_idx < self.history.len() - 1;
            let fwd_btn = ui.button("▶").on_hover_text(if fwd_can_drop {
                "drop to move here"
            } else {
                "forward"
            });

            if fwd_can_drop && fwd_btn.hovered() {
                let painter = ui.painter();
                slowcore::dither::draw_dither_selection(painter, fwd_btn.rect);
            }

            if fwd_btn.clicked() { self.go_forward(); }
            if fwd_can_drop && fwd_btn.hovered() && primary_released {
                drop_to_fwd = true;
            }

            // Up button - droppable when dragging and parent exists
            let has_parent = self.current_dir.parent().is_some();
            let up_can_drop = is_dragging && has_parent;
            let up_btn = ui.button("▲").on_hover_text(if up_can_drop {
                "drop to move to parent"
            } else {
                "up"
            });

            // Darken up button when hovered during drag (dither effect)
            if up_can_drop && up_btn.hovered() {
                let painter = ui.painter();
                slowcore::dither::draw_dither_selection(painter, up_btn.rect);
            }

            if up_btn.clicked() { self.go_up(); }
            if up_can_drop && up_btn.hovered() && primary_released {
                drop_to_up = true;
            }

            if ui.button("⟳").on_hover_text("refresh").clicked() { self.refresh(); }
            ui.separator();

            let view_label = match self.view_mode {
                ViewMode::Icons => "icons ▾",
                ViewMode::List => "list ▾",
            };
            ui.menu_button(view_label, |ui| {
                if ui.button("icons (1)").clicked() {
                    self.view_mode = ViewMode::Icons;
                    ui.close_menu();
                }
                if ui.button("list (2)").clicked() {
                    self.view_mode = ViewMode::List;
                    ui.close_menu();
                }
            });
            ui.separator();

            let r = ui.text_edit_singleline(&mut self.path_input);
            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                let path = PathBuf::from(&self.path_input);
                if path.is_dir() { self.navigate(path); }
            }
        });

        // Handle drops on nav buttons
        if drop_to_back {
            if let Some(paths) = self.dragging.take() {
                let dest = self.history[self.history_idx - 1].clone();
                self.move_files_to_folder(&paths, &dest);
            }
            self.drag_preview = None;
            self.drag_hover_idx = None;
        }
        if drop_to_fwd {
            if let Some(paths) = self.dragging.take() {
                let dest = self.history[self.history_idx + 1].clone();
                self.move_files_to_folder(&paths, &dest);
            }
            self.drag_preview = None;
            self.drag_hover_idx = None;
        }
        if drop_to_up {
            if let Some(paths) = self.dragging.take() {
                if let Some(parent) = self.current_dir.parent() {
                    let dest = parent.to_path_buf();
                    self.move_files_to_folder(&paths, &dest);
                }
            }
            self.drag_preview = None;
            self.drag_hover_idx = None;
        }
    }

    /// Handle a click action (shift/cmd/normal) to update selection.
    fn handle_click_action(&mut self, idx: usize, shift: bool, cmd: bool) {
        if shift && self.last_clicked.is_some() {
            let start = self.last_clicked.unwrap();
            let (from, to) = if start <= idx { (start, idx) } else { (idx, start) };
            if !cmd {
                self.selected.clear();
            }
            for i in from..=to {
                self.selected.insert(i);
            }
        } else if cmd {
            if self.selected.contains(&idx) {
                self.selected.remove(&idx);
            } else {
                self.selected.insert(idx);
            }
            self.last_clicked = Some(idx);
        } else {
            self.selected.clear();
            self.selected.insert(idx);
            self.last_clicked = Some(idx);
        }
    }

    /// Start a drag operation from the collected drag_start data.
    fn apply_drag_start(&mut self, paths: Vec<PathBuf>, icon_key: String, name: String, count: usize) {
        slowcore::drag::start_drag(&paths);
        self.dragging = Some(paths);
        self.drag_preview = Some((icon_key, name, count));
    }

    /// Handle a drop onto `drop_target` and clear drag state on mouse release.
    fn handle_drop_and_clear_drag(&mut self, drop_target: Option<PathBuf>, primary_released: bool) {
        let did_drop = drop_target.is_some();
        if let Some(dest) = drop_target {
            if let Some(paths) = self.dragging.take() {
                self.move_files_to_folder(&paths, &dest);
            }
            slowcore::drag::end_drag();
            self.drag_hover_idx = None;
            self.drag_preview = None;
        }

        if primary_released && !did_drop {
            slowcore::drag::end_drag();
            self.dragging = None;
            self.drag_preview = None;
            self.drag_hover_idx = None;
        }
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
                let icon_key = if entry.is_dir { "folder".to_string() } else { file_icon_key(&entry.name).to_string() };
                let size_str = if entry.is_dir { "—".into() } else { format_size(entry.size) };
                (idx, entry.name.clone(), icon_key, size_str, entry.modified.clone(), entry.is_dir, entry.path.clone())
            }).collect();

        // Get modifier state for shift/cmd click
        let modifiers = ui.input(|i| i.modifiers);

        // File list
        let mut nav_target: Option<PathBuf> = None;
        let mut open_target: Option<(PathBuf, Rect)> = None;
        let mut click_action: Option<(usize, bool, bool)> = None; // (idx, shift, cmd)
        let mut drag_start: Option<(Vec<PathBuf>, String, String, usize)> = None;
        let mut drop_target: Option<PathBuf> = None;
        let primary_released = ui.input(|i| i.pointer.primary_released());

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (idx, name, icon_key, size_str, modified, is_dir, path) in &display_entries {
                let is_selected = self.selected.contains(idx);
                let is_drag_hover = self.drag_hover_idx == Some(*idx) && *is_dir;
                let row_height = 18.0;
                let total_w = ui.available_width();
                let name_w = total_w - 180.0;

                // Draw the row manually so we control alignment
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(total_w, row_height),
                    egui::Sense::click_and_drag(),
                );

                if ui.is_rect_visible(rect) {
                    let painter = ui.painter();

                    // Selection highlight — dithered (darken folders when dragging over)
                    if is_drag_hover {
                        slowcore::dither::draw_dither_selection(painter, rect);
                    } else if is_selected {
                        slowcore::dither::draw_dither_selection(painter, rect);
                    } else if response.hovered() {
                        slowcore::dither::draw_dither_hover(painter, rect);
                    }

                    let text_color = if is_selected { SlowColors::WHITE } else { SlowColors::BLACK };

                    // Icon (small, 14px) + filename
                    let icon_px = 14.0;
                    let icon_x = rect.min.x + 4.0;
                    let icon_center = egui::pos2(icon_x + icon_px / 2.0, rect.center().y);
                    let icon_rect = Rect::from_center_size(icon_center, Vec2::splat(icon_px));

                    // For image files, try to use a thumbnail
                    let mut drew_thumbnail = false;
                    if icon_key == "image" && !*is_dir {
                        if let Some(thumb) = self.get_or_create_thumbnail(ui.ctx(), path) {
                            let thumb_size = thumb.size_vec2();
                            let scale = icon_px / thumb_size.x.max(thumb_size.y);
                            let display_size = Vec2::new(thumb_size.x * scale, thumb_size.y * scale);
                            let thumb_rect = Rect::from_center_size(icon_center, display_size);
                            painter.image(
                                thumb.id(),
                                thumb_rect,
                                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                egui::Color32::WHITE,
                            );
                            drew_thumbnail = true;
                        }
                    }

                    if !drew_thumbnail {
                        if let Some(tex) = self.file_icons.get(icon_key.as_str()) {
                            painter.image(
                                tex.id(),
                                icon_rect,
                                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                egui::Color32::WHITE,
                            );
                        }
                    }

                    painter.text(
                        egui::pos2(icon_x + icon_px + 4.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        name,
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

                // Start drag - allows dragging unselected items directly
                if response.drag_started() {
                    // If dragging an unselected item, select only that item
                    if !is_selected {
                        self.selected.clear();
                        self.selected.insert(*idx);
                    }
                    // Now drag all selected items
                    let paths: Vec<PathBuf> = self.selected.iter()
                        .filter_map(|&i| self.entries.get(i).map(|e| e.path.clone()))
                        .collect();
                    if !paths.is_empty() {
                        let count = paths.len();
                        drag_start = Some((paths, icon_key.clone(), name.clone(), count));
                    }
                }

                // Track hover target for drop (but not if hovering over a dragged item)
                let is_being_dragged = self.dragging.as_ref()
                    .map(|paths| paths.iter().any(|p| p == path))
                    .unwrap_or(false);
                if self.dragging.is_some() && response.hovered() && *is_dir && !is_being_dragged {
                    self.drag_hover_idx = Some(*idx);
                    // Handle drop on folder when mouse released while hovering
                    if primary_released {
                        drop_target = Some(path.clone());
                    }
                }

                if response.clicked() {
                    click_action = Some((*idx, modifiers.shift, modifiers.command));
                }
                if response.double_clicked() {
                    if *is_dir {
                        nav_target = Some(path.clone());
                    } else {
                        open_target = Some((path.clone(), rect));
                    }
                }
            }
        });

        // Start dragging
        if let Some((paths, icon_key, name, count)) = drag_start {
            self.apply_drag_start(paths, icon_key, name, count);
        }

        // Handle drop and clear drag state
        self.handle_drop_and_clear_drag(drop_target, primary_released);

        // Handle click actions after the loop to avoid borrow issues
        if let Some((idx, shift, cmd)) = click_action {
            self.handle_click_action(idx, shift, cmd);
        }

        if let Some(path) = nav_target { self.navigate(path); }
        if let Some((path, rect)) = open_target {
            self.open_anim = Some((rect, 0.0));
            open_in_slow_app(&path);
        }
    }

    fn render_icon_view(&mut self, ui: &mut egui::Ui) {
        let cell_w = 96.0;
        let cell_h = 96.0;
        let available_w = ui.available_width();
        let cols = ((available_w / cell_w) as usize).max(1);

        // Clear item rects for this frame
        self.item_rects.clear();

        // Collect entry data: (index, name, icon_key, is_dir, path)
        let display_entries: Vec<(usize, String, String, bool, PathBuf)> =
            self.entries.iter().enumerate().map(|(idx, entry)| {
                let icon_key = if entry.is_dir { "folder".to_string() } else { file_icon_key(&entry.name).to_string() };
                (idx, entry.name.clone(), icon_key, entry.is_dir, entry.path.clone())
            }).collect();

        let modifiers = ui.input(|i| i.modifiers);
        let pointer_pos = ui.input(|i| i.pointer.interact_pos());
        let primary_down = ui.input(|i| i.pointer.primary_down());
        let primary_released = ui.input(|i| i.pointer.primary_released());

        let mut nav_target: Option<PathBuf> = None;
        let mut open_target: Option<(PathBuf, Rect)> = None;
        let mut click_action: Option<(usize, bool, bool)> = None;
        let mut drag_start: Option<(Vec<PathBuf>, String, String, usize)> = None;
        let mut drop_target: Option<PathBuf> = None;
        let mut clicked_on_item = false;

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Allocate a background area for detecting clicks on empty space
            let content_rect = ui.available_rect_before_wrap();

            let chunks: Vec<&[(usize, String, String, bool, PathBuf)]> =
                display_entries.chunks(cols).collect();

            for row in chunks {
                ui.horizontal(|ui| {
                    for (idx, name, icon_key, is_dir, path) in row {
                        let is_selected = self.selected.contains(idx);
                        let is_drag_hover = self.drag_hover_idx == Some(*idx) && *is_dir;

                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(cell_w, cell_h),
                            egui::Sense::click_and_drag(),
                        );

                        // Store item rect for marquee hit testing
                        self.item_rects.push((*idx, rect));

                        // Check if this item is inside the marquee selection
                        let in_marquee = if let (Some(start), Some(current)) = (self.marquee_start, pointer_pos) {
                            if primary_down {
                                let marquee_rect = Rect::from_two_pos(start, current);
                                rect.intersects(marquee_rect)
                            } else {
                                false
                            }
                        } else {
                            false
                        };

                        if ui.is_rect_visible(rect) {
                            let painter = ui.painter();

                            // Darken folder when dragging over it (dither effect)
                            if is_drag_hover {
                                slowcore::dither::draw_dither_selection(painter, rect);
                            } else if is_selected || in_marquee {
                                slowcore::dither::draw_dither_selection(painter, rect);
                            } else if response.hovered() {
                                slowcore::dither::draw_dither_hover(painter, rect);
                            }

                            let text_color = if is_selected || in_marquee { SlowColors::WHITE } else { SlowColors::BLACK };

                            // Icon centered in upper area
                            let icon_size = 48.0;
                            let icon_center = egui::pos2(rect.center().x, rect.min.y + 30.0);
                            let icon_rect = Rect::from_center_size(icon_center, Vec2::splat(icon_size));

                            // For image files, try to use a thumbnail
                            let mut drew_thumbnail = false;
                            if icon_key == "image" && !*is_dir {
                                if let Some(thumb) = self.get_or_create_thumbnail(ui.ctx(), path) {
                                    // Center the thumbnail (may be smaller than 48x48)
                                    let thumb_size = thumb.size_vec2();
                                    let scale = (icon_size / thumb_size.x.max(thumb_size.y)).min(1.5);
                                    let display_size = Vec2::new(thumb_size.x * scale, thumb_size.y * scale);
                                    let thumb_rect = Rect::from_center_size(icon_center, display_size);
                                    painter.image(
                                        thumb.id(),
                                        thumb_rect,
                                        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                        egui::Color32::WHITE,
                                    );
                                    drew_thumbnail = true;
                                }
                            }

                            if !drew_thumbnail {
                                if let Some(tex) = self.file_icons.get(icon_key.as_str()) {
                                    painter.image(
                                        tex.id(),
                                        icon_rect,
                                        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                        egui::Color32::WHITE,
                                    );
                                } else {
                                    // Fallback text
                                    painter.text(
                                        icon_center, egui::Align2::CENTER_CENTER,
                                        if *is_dir { "D" } else { "F" },
                                        egui::FontId::proportional(28.0), text_color,
                                    );
                                }
                            }

                            // Filename below icon, truncated
                            let display_name = if name.len() > 12 {
                                format!("{}...", &name[..11])
                            } else {
                                name.clone()
                            };
                            let name_pos = egui::pos2(rect.center().x, rect.min.y + 66.0);
                            painter.text(
                                name_pos,
                                egui::Align2::CENTER_CENTER,
                                &display_name,
                                egui::FontId::proportional(10.0),
                                text_color,
                            );
                        }

                        // Start drag - allows dragging unselected items directly
                        if response.drag_started() && self.marquee_start.is_none() {
                            // If dragging an unselected item, select only that item
                            if !is_selected {
                                self.selected.clear();
                                self.selected.insert(*idx);
                            }
                            // Now drag all selected items
                            let paths: Vec<PathBuf> = self.selected.iter()
                                .filter_map(|&i| self.entries.get(i).map(|e| e.path.clone()))
                                .collect();
                            if !paths.is_empty() {
                                let count = paths.len();
                                drag_start = Some((paths, icon_key.clone(), name.clone(), count));
                            }
                        }

                        // Track hover target for drop (but not if hovering over a dragged item)
                        let is_being_dragged = self.dragging.as_ref()
                            .map(|paths| paths.iter().any(|p| p == path))
                            .unwrap_or(false);
                        if self.dragging.is_some() && response.hovered() && *is_dir && !is_being_dragged {
                            self.drag_hover_idx = Some(*idx);
                            // Handle drop on folder when mouse released while hovering
                            if primary_released {
                                drop_target = Some(path.clone());
                            }
                        }

                        if response.clicked() {
                            click_action = Some((*idx, modifiers.shift, modifiers.command));
                            clicked_on_item = true;
                        }
                        if response.double_clicked() {
                            if *is_dir {
                                nav_target = Some(path.clone());
                            } else {
                                open_target = Some((path.clone(), rect));
                            }
                        }
                    }
                });
            }

            // Detect drag on empty space for marquee selection (not using ui.interact which steals clicks)
            let primary_pressed = ui.input(|i| i.pointer.primary_pressed());
            if primary_pressed && !clicked_on_item {
                if let Some(pos) = pointer_pos {
                    // Check if the click is not on any item
                    let on_item = self.item_rects.iter().any(|(_, r)| r.contains(pos));
                    if !on_item && content_rect.contains(pos) {
                        self.marquee_start = Some(pos);
                        // Clear selection unless shift is held
                        if !modifiers.shift {
                            self.selected.clear();
                        }
                    }
                }
            }
        });

        // Draw marquee rectangle if active
        if let (Some(start), Some(current)) = (self.marquee_start, pointer_pos) {
            if primary_down {
                let painter = ui.painter();
                let marquee_rect = Rect::from_two_pos(start, current);
                // Draw dashed rectangle for marquee
                painter.rect_stroke(
                    marquee_rect,
                    0.0,
                    egui::Stroke::new(1.0, SlowColors::BLACK),
                );
            }
        }

        // Finalize marquee selection on mouse release
        if primary_released && self.marquee_start.is_some() {
            if let (Some(start), Some(end)) = (self.marquee_start, pointer_pos) {
                let marquee_rect = Rect::from_two_pos(start, end);
                // Select all items that intersect with the marquee
                for (idx, item_rect) in &self.item_rects {
                    if item_rect.intersects(marquee_rect) {
                        self.selected.insert(*idx);
                    }
                }
            }
            self.marquee_start = None;
        }

        // Start dragging
        if let Some((paths, icon_key, name, count)) = drag_start {
            self.apply_drag_start(paths, icon_key, name, count);
        }

        // Handle drop and clear drag state
        self.handle_drop_and_clear_drag(drop_target, primary_released);

        // Handle click actions (only if not doing marquee)
        if self.marquee_start.is_none() {
            if let Some((idx, shift, cmd)) = click_action {
                self.handle_click_action(idx, shift, cmd);
            }
        }

        if let Some(path) = nav_target { self.navigate(path); }
        if let Some((path, rect)) = open_target {
            self.open_anim = Some((rect, 0.0));
            open_in_slow_app(&path);
        }
    }
}

impl eframe::App for SlowFilesApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.repaint.begin_frame(ctx);
        self.ensure_file_icons(ctx);
        self.handle_keys(ctx);

        // Update opening animation
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        if let Some((_, ref mut progress)) = self.open_anim {
            *progress += dt * 3.0; // Complete in ~0.33s
            if *progress >= 1.0 {
                self.open_anim = None;
            }
        }
        // Enable continuous repaint during folder open animation
        self.repaint.set_continuous(self.open_anim.is_some());

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
                    if ui.button("new folder...  ⇧⌘N").clicked() {
                        self.show_new_folder = true;
                        self.focus_new_folder_field = true;
                        self.new_folder_name = "untitled folder".to_string();
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
                    if ui.button("keyboard shortcuts").clicked() { self.show_shortcuts = true; ui.close_menu(); }
                    ui.separator();
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
            match self.view_mode {
                ViewMode::Icons => self.render_icon_view(ui),
                ViewMode::List => self.render_file_list(ui),
            }
        });

        if self.show_about {
            // Calculate max height based on available screen space
            let screen_rect = ctx.screen_rect();
            let max_height = (screen_rect.height() - 80.0).max(200.0);

            let resp = egui::Window::new("about files")
                .collapsible(false)
                .resizable(false)
                .default_width(300.0)
                .max_height(max_height)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().max_height(max_height - 60.0).show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.heading("files");
                            ui.label("version 0.2.2");
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
                    });
                    ui.vertical_centered(|ui| {
                        if ui.button("ok").clicked() { self.show_about = false; }
                    });
                });
            if let Some(r) = &resp {
                slowcore::dither::draw_window_shadow(ctx, r.response.rect);
            }
        }

        if self.show_shortcuts {
            // Calculate max height based on available screen space
            let screen_rect = ctx.screen_rect();
            let max_height = (screen_rect.height() - 80.0).max(200.0);

            let resp = egui::Window::new("keyboard shortcuts")
                .collapsible(false)
                .resizable(false)
                .default_width(320.0)
                .max_height(max_height)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().max_height(max_height - 60.0).show(ui, |ui| {
                        ui.heading("files shortcuts");
                        ui.add_space(8.0);

                        ui.label(egui::RichText::new("Navigation").strong());
                        ui.separator();
                        shortcut_row(ui, "Enter", "Open selected item");
                        shortcut_row(ui, "Backspace", "Go to parent folder");
                        shortcut_row(ui, "⌘↑", "Go up one folder");
                        shortcut_row(ui, "⌘←", "Go back");
                        shortcut_row(ui, "⌘→", "Go forward");
                        shortcut_row(ui, "↑/↓", "Navigate between items");
                        ui.add_space(8.0);

                        ui.label(egui::RichText::new("Selection").strong());
                        ui.separator();
                        shortcut_row(ui, "⌘A", "Select all");
                        shortcut_row(ui, "Shift+Click", "Select range");
                        shortcut_row(ui, "⌘+Click", "Toggle item selection");
                        shortcut_row(ui, "Click+Drag", "Marquee select (icon view)");
                        shortcut_row(ui, "Esc", "Deselect all");
                        ui.add_space(8.0);

                        ui.label(egui::RichText::new("File Operations").strong());
                        ui.separator();
                        shortcut_row(ui, "⇧⌘N", "New folder");
                        shortcut_row(ui, "⌫", "Move to trash");
                        shortcut_row(ui, "⌘Z", "Undo delete");
                        ui.add_space(8.0);

                        ui.label(egui::RichText::new("View").strong());
                        ui.separator();
                        shortcut_row(ui, "1", "Icon view");
                        shortcut_row(ui, "2", "List view");
                        ui.add_space(8.0);
                    });
                    ui.vertical_centered(|ui| {
                        if ui.button("ok").clicked() { self.show_shortcuts = false; }
                    });
                });
            if let Some(r) = &resp {
                slowcore::dither::draw_window_shadow(ctx, r.response.rect);
            }
        }

        // New folder dialog
        if self.show_new_folder {
            let should_focus = self.focus_new_folder_field;
            self.focus_new_folder_field = false;

            let resp = egui::Window::new("new folder")
                .collapsible(false)
                .resizable(false)
                .default_width(250.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("name:");
                        let r = ui.text_edit_singleline(&mut self.new_folder_name);
                        if should_focus {
                            r.request_focus();
                        }
                        if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                            self.create_new_folder();
                        }
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button("cancel").clicked() {
                            self.show_new_folder = false;
                            self.new_folder_name.clear();
                        }
                        if ui.button("create").clicked() {
                            self.create_new_folder();
                        }
                    });
                });
            if let Some(r) = &resp {
                slowcore::dither::draw_window_shadow(ctx, r.response.rect);
            }
        }

        // Draw drag preview silhouette following cursor
        if let (Some((icon_key, name, count)), Some(pos)) = (&self.drag_preview, ctx.input(|i| i.pointer.hover_pos())) {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("drag_preview"),
            ));

            // Draw semi-transparent icon near cursor
            let icon_size = 48.0;
            let offset = Vec2::new(16.0, 16.0); // Offset from cursor
            let icon_center = pos + offset + Vec2::new(icon_size / 2.0, icon_size / 2.0);
            let icon_rect = Rect::from_center_size(icon_center, Vec2::splat(icon_size));

            // Draw icon (pure white tint — no alpha on e-ink)
            if let Some(tex) = self.file_icons.get(icon_key.as_str()) {
                painter.image(
                    tex.id(),
                    icon_rect,
                    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }

            // Draw name below icon
            let display_name = if name.len() > 12 {
                format!("{}...", &name[..11])
            } else {
                name.clone()
            };
            let name_pos = egui::pos2(icon_center.x, icon_rect.max.y + 10.0);
            painter.text(
                name_pos,
                egui::Align2::CENTER_CENTER,
                &display_name,
                egui::FontId::proportional(10.0),
                SlowColors::BLACK,
            );

            // If multiple items, show count badge
            if *count > 1 {
                let badge_center = egui::pos2(icon_rect.max.x - 4.0, icon_rect.min.y + 4.0);
                painter.circle_filled(badge_center, 9.0, SlowColors::BLACK);
                painter.text(
                    badge_center,
                    egui::Align2::CENTER_CENTER,
                    &count.to_string(),
                    egui::FontId::proportional(10.0),
                    SlowColors::WHITE,
                );
            }
        }

        // Draw expanding rectangle animation overlay
        if let Some((start_rect, progress)) = self.open_anim {
            let screen = ctx.screen_rect();
            let t = progress.min(1.0);
            // Ease out cubic
            let t = 1.0 - (1.0 - t).powi(3);
            let target = Rect::from_center_size(screen.center(), screen.size() * 0.8);
            let current = Rect::from_min_max(
                egui::pos2(
                    start_rect.min.x + (target.min.x - start_rect.min.x) * t,
                    start_rect.min.y + (target.min.y - start_rect.min.y) * t,
                ),
                egui::pos2(
                    start_rect.max.x + (target.max.x - start_rect.max.x) * t,
                    start_rect.max.y + (target.max.y - start_rect.max.y) * t,
                ),
            );
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("open_anim"),
            ));
            painter.rect_stroke(current, 0.0, egui::Stroke::new(2.0, SlowColors::BLACK));
        }

        self.repaint.end_frame(ctx);
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

/// Map a filename to a file icon category key
fn file_icon_key(name: &str) -> &'static str {
    let lower = name.to_lowercase();
    // Check compound extensions first
    if lower.ends_with(".slides.json") {
        return "slides";
    }
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "txt" | "md" | "rs" | "py" | "js" | "c" | "h" | "css" | "html"
            | "toml" | "yaml" | "yml" | "xml" | "sh" | "pdf" | "json" => "text",
        "png" | "jpg" | "jpeg" | "bmp" | "gif" | "tiff" | "webp" | "svg" => "image",
        "mid" | "midi" => "midi",
        "mp3" | "wav" | "flac" | "ogg" | "aac" | "m4a" => "audio",
        "epub" => "epub",
        "csv" | "tsv" | "sheets" => "sheets",
        "slides" => "slides",
        "tex" | "latex" => "latex",
        _ => "text",
    }
}

/// Map a file extension to the slow app binary that should open it.
fn slow_app_for_ext(ext: &str) -> Option<&'static str> {
    match ext {
        "txt" | "md" | "rtf" => Some("slowwrite"),
        "png" | "jpg" | "jpeg" | "bmp" | "gif" | "tiff" | "webp" | "pdf" => Some("slowview"),
        "epub" => Some("slowreader"),
        "mid" | "midi" => Some("slowmidi"),
        "mp3" | "wav" | "flac" | "ogg" | "aac" | "m4a" => Some("slowmusic"),
        "swd" => Some("slowwrite"),
        _ => None,
    }
}

/// Find a slow app binary by name, searching common binary paths.
fn find_slow_binary(name: &str) -> Option<PathBuf> {
    let mut paths = Vec::new();

    let exe_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf()));
    if let Some(ref dir) = exe_dir {
        paths.push(dir.clone());
    }
    paths.push(PathBuf::from("/usr/bin"));
    if let Some(ref dir) = exe_dir {
        let mut search_dir = Some(dir.clone());
        while let Some(d) = search_dir {
            if d.join("Cargo.toml").exists() {
                paths.push(d.join("target/debug"));
                paths.push(d.join("target/release"));
                break;
            }
            search_dir = d.parent().map(|p| p.to_path_buf());
        }
    }

    for base in &paths {
        let path = base.join(name);
        if path.exists() && path.is_file() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = path.metadata() {
                    if meta.permissions().mode() & 0o111 != 0 {
                        return Some(path);
                    }
                }
            }
            #[cfg(not(unix))]
            return Some(path);
        }
    }
    None
}

/// Open a file in the appropriate slow app, falling back to system default.
fn open_in_slow_app(path: &PathBuf) {
    // Check compound extensions first (e.g., .slides.json)
    let filename = path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_lowercase())
        .unwrap_or_default();

    let ext = if filename.ends_with(".slides.json") {
        "slides.json".to_string()
    } else {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default()
    };

    if let Some(app_name) = slow_app_for_ext(&ext) {
        if let Some(bin_path) = find_slow_binary(app_name) {
            use std::sync::atomic::{AtomicU32, Ordering};
            static CASCADE: AtomicU32 = AtomicU32::new(0);
            let offset = CASCADE.fetch_add(1, Ordering::Relaxed) % 10;
            let _ = std::process::Command::new(bin_path)
                .arg(path.to_string_lossy().as_ref())
                .env("SLOWOS_MANAGED", "1")
                .env("SLOWOS_CASCADE", offset.to_string())
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .spawn();
            return;
        }
    }
    let _ = open::that(path);
}

fn shortcut_row(ui: &mut egui::Ui, shortcut: &str, description: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(shortcut).monospace().strong());
        ui.add_space(20.0);
        ui.label(description);
    });
}
