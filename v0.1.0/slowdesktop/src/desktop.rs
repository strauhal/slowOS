//! SlowOS Desktop — System 6-inspired desktop environment
//!
//! Features:
//! - Dithered desktop background
//! - Menu bar with system menu, apps menu, date and clock
//! - Desktop icons for each application (double-click to launch)
//! - Smooth window open/close animations
//! - Running app indicators
//! - Keyboard navigation
//! - About dialog with system info

use crate::process_manager::{AppInfo, ProcessManager};
use chrono::Local;
use egui::{
    Align2, ColorImage, Context, FontId, Key, Painter, Pos2, Rect, Response, Sense, Stroke,
    TextureHandle, TextureOptions, Ui, Vec2,
};
use slowcore::animation::AnimationManager;
use slowcore::dither;
use slowcore::theme::SlowColors;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// A desktop folder shortcut
struct DesktopFolder {
    name: &'static str,
    /// Directory path this folder opens
    path: PathBuf,
}

/// Desktop icon layout
const ICON_SIZE: f32 = 64.0;
const ICON_SPACING: f32 = 80.0;
const ICON_LABEL_HEIGHT: f32 = 16.0;
const ICON_TOTAL_HEIGHT: f32 = 52.0 + ICON_LABEL_HEIGHT;
const DESKTOP_PADDING: f32 = 24.0;
const MENU_BAR_HEIGHT: f32 = 22.0;
const ICONS_PER_COLUMN: usize = 6;

/// Double-click timing threshold in milliseconds
const DOUBLE_CLICK_MS: u128 = 400;

/// Desktop application state
pub struct DesktopApp {
    /// Process manager for launching/tracking apps
    process_manager: ProcessManager,
    /// Currently selected app icon indices
    selected_icons: HashSet<usize>,
    /// Time of last click (for double-click detection)
    last_click_time: Instant,
    /// Index of last clicked icon (for double-click detection)
    last_click_index: Option<usize>,
    /// Currently hovered icon index
    hovered_icon: Option<usize>,
    /// Show about dialog
    show_about: bool,
    /// Show shutdown dialog
    show_shutdown: bool,
    /// Status message (bottom of screen)
    status_message: String,
    /// Status message timestamp
    status_time: Instant,
    /// Frame counter for polling
    frame_count: u64,
    /// Animation manager for window open/close effects
    animations: AnimationManager,
    /// Cached icon positions for animations
    icon_rects: Vec<(String, Rect)>,
    /// Folder icon rect that last launched slowFiles (for close animation)
    last_folder_launch_rect: Option<Rect>,
    /// Cached folder icon rects for animations (populated during draw)
    folder_icon_rects: Vec<Rect>,
    /// Screen dimensions for animation targets
    screen_rect: Rect,
    /// Last frame time for delta calculation
    last_frame_time: Instant,
    /// Use 24-hour (military) time format
    use_24h_time: bool,
    /// Date format: 0 = "Mon Jan 15", 1 = "01/15", 2 = "15/01", 3 = "2024-01-15"
    date_format: u8,
    /// Spotlight search state
    show_search: bool,
    search_query: String,
    /// Frame when search was opened (to prevent immediate close)
    search_opened_frame: u64,
    /// Icon textures loaded from embedded PNGs
    icon_textures: HashMap<String, TextureHandle>,
    /// Whether textures have been initialized
    icons_loaded: bool,
    /// Desktop folder shortcuts
    desktop_folders: Vec<DesktopFolder>,
    /// Selected folder indices
    selected_folders: HashSet<usize>,
    /// Last click time for folder double-click
    last_folder_click_time: Instant,
    /// Last clicked folder index
    last_folder_click_index: Option<usize>,
    /// Hovered folder index
    hovered_folder: Option<usize>,
    /// Marquee selection start position
    marquee_start: Option<Pos2>,
}

impl DesktopApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let docs = dirs::document_dir().unwrap_or_else(|| home.join("Documents"));

        // Setup default content (books, pictures) on first launch
        Self::setup_default_content(&home);

        let desktop_folders = vec![
            DesktopFolder { name: "documents", path: docs.clone() },
            DesktopFolder { name: "books", path: home.join("Books") },
            DesktopFolder { name: "pictures", path: home.join("Pictures") },
            DesktopFolder { name: "music", path: home.join("Music") },
            DesktopFolder { name: "midi", path: home.join("MIDI") },
        ];

        Self {
            process_manager: ProcessManager::new(),
            selected_icons: HashSet::new(),
            last_click_time: Instant::now(),
            last_click_index: None,
            hovered_icon: None,
            show_about: false,
            show_shutdown: false,
            status_message: "welcome to slowOS".to_string(),
            status_time: Instant::now(),
            frame_count: 0,
            animations: AnimationManager::new(),
            icon_rects: Vec::new(),
            last_folder_launch_rect: None,
            folder_icon_rects: Vec::new(),
            screen_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 680.0)),
            last_frame_time: Instant::now(),
            use_24h_time: false,
            date_format: 0,
            show_search: false,
            search_query: String::new(),
            search_opened_frame: 0,
            icon_textures: HashMap::new(),
            icons_loaded: false,
            desktop_folders,
            selected_folders: HashSet::new(),
            last_folder_click_time: Instant::now(),
            last_folder_click_index: None,
            hovered_folder: None,
            marquee_start: None,
        }
    }

    /// Setup default content folders (slowLibrary books, slowMuseum pictures)
    /// This runs on first launch to populate user folders with bundled content.
    fn setup_default_content(home: &PathBuf) {
        // Find the data directory (relative to executable or at standard locations)
        let data_dirs = Self::find_data_dirs();

        // Setup Books/slowLibrary
        let books_dir = home.join("Books");
        let slow_library = books_dir.join("slowLibrary");
        if !slow_library.exists() {
            // Create Books directory if needed
            let _ = std::fs::create_dir_all(&books_dir);

            // Look for slowLibrary source
            for data_dir in &data_dirs {
                let source = data_dir.join("slowLibrary");
                if source.is_dir() {
                    if let Err(_) = Self::copy_dir_recursive(&source, &slow_library) {
                        // Silently fail - not critical
                    }
                    break;
                }
            }
        }

        // Setup Pictures/slowMuseum (if source exists)
        let pictures_dir = home.join("Pictures");
        let slow_museum = pictures_dir.join("slowMuseum");
        if !slow_museum.exists() {
            // Create Pictures directory if needed
            let _ = std::fs::create_dir_all(&pictures_dir);

            // Look for slowMuseum source
            for data_dir in &data_dirs {
                let source = data_dir.join("slowMuseum");
                if source.is_dir() {
                    if let Err(_) = Self::copy_dir_recursive(&source, &slow_museum) {
                        // Silently fail - not critical
                    }
                    break;
                }
            }
        }

        // Setup Pictures subdirectories from default_content
        for folder_name in &["computerdrawing.club", "icons_process"] {
            let dest = pictures_dir.join(folder_name);
            if !dest.exists() {
                for data_dir in &data_dirs {
                    let source = data_dir.join("default_content").join("Pictures").join(folder_name);
                    if source.is_dir() {
                        let _ = Self::copy_dir_recursive(&source, &dest);
                        break;
                    }
                }
            }
        }

        // Ensure other standard folders exist
        let _ = std::fs::create_dir_all(home.join("Music"));
        let midi_dir = home.join("MIDI");
        let _ = std::fs::create_dir_all(&midi_dir);
        let _ = std::fs::create_dir_all(home.join("Documents"));

        // Setup MIDI/compositions (if source exists)
        let compositions_dir = midi_dir.join("compositions");
        if !compositions_dir.exists() {
            // Look for compositions source
            for data_dir in &data_dirs {
                let source = data_dir.join("compositions");
                if source.is_dir() {
                    if let Err(_) = Self::copy_dir_recursive(&source, &compositions_dir) {
                        // Silently fail - not critical
                    }
                    break;
                }
            }
        }
    }

    /// Find directories that might contain bundled content
    fn find_data_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        // 1. Directory of the executable
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                // Check for data dir next to executable
                dirs.push(exe_dir.to_path_buf());
                // Check parent directories (for cargo builds)
                if let Some(parent) = exe_dir.parent() {
                    dirs.push(parent.to_path_buf());
                    if let Some(grandparent) = parent.parent() {
                        dirs.push(grandparent.to_path_buf());
                        // Look for workspace root (where slowLibrary is)
                        if let Some(workspace) = grandparent.parent() {
                            dirs.push(workspace.to_path_buf());
                        }
                    }
                }
            }
        }

        // 2. Standard data locations
        dirs.push(PathBuf::from("/usr/share/slowos"));
        dirs.push(PathBuf::from("/usr/local/share/slowos"));

        dirs
    }

    /// Recursively copy a directory
    fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> std::io::Result<()> {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let path = entry.path();
            let dest_path = dst.join(entry.file_name());
            if path.is_dir() {
                Self::copy_dir_recursive(&path, &dest_path)?;
            } else {
                std::fs::copy(&path, &dest_path)?;
            }
        }
        Ok(())
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = msg.into();
        self.status_time = Instant::now();
    }

    /// Load embedded icon PNGs as egui textures
    fn load_icon_textures(&mut self, ctx: &Context) {
        if self.icons_loaded {
            return;
        }
        self.icons_loaded = true;

        let icons: &[(&str, &[u8])] = &[
            ("slowwrite", include_bytes!("../../icons/icons_write.png")),
            ("slowpaint", include_bytes!("../../icons/icons_paint.png")),
            ("slowreader", include_bytes!("../../icons/icons_reader.png")),
            ("slowsheets", include_bytes!("../../icons/icons_sheets_1.png")),
            ("slowchess", include_bytes!("../../icons/icons_chess.png")),
            ("slowfiles", include_bytes!("../../icons/icons_files.png")),
            ("slowmusic", include_bytes!("../../icons/icons_music.png")),
            ("slowslides", include_bytes!("../../icons/icons_slides.png")),
            ("slowtex", include_bytes!("../../icons/icons_latex.png")),
            ("trash", include_bytes!("../../icons/icons_trash.png")),
            ("slowview", include_bytes!("../../icons/icons_view.png")),
            ("credits", include_bytes!("../../icons/icons_credits.png")),
            ("slowmidi", include_bytes!("../../icons/icons_midi.png")),
            ("slowbreath", include_bytes!("../../icons/icons_breath.png")),
            ("settings", include_bytes!("../../icons/icons_settings.png")),
            ("folder", include_bytes!("../../icons/icons_files.png")),
            ("slowterm", include_bytes!("../../icons/icons_terminal.png")),
            ("slowcalc", include_bytes!("../../icons/icons_calculator.png")),
            ("slownotes", include_bytes!("../../icons/icons_notes.png")),
            ("slowsolitaire", include_bytes!("../../icons/icons_solitaire.png")),
            // Folder-specific icons
            ("folder_documents", include_bytes!("../../icons/folder_icons/icons_docsfolder.png")),
            ("folder_books", include_bytes!("../../icons/folder_icons/icons_bookfolder.png")),
            ("folder_pictures", include_bytes!("../../icons/folder_icons/icons_picturefolder.png")),
            ("folder_music", include_bytes!("../../icons/folder_icons/icons_musicfolder.png")),
            ("folder_midi", include_bytes!("../../icons/folder_icons/icons_midifolder.png")),
        ];

        for (binary, png_bytes) in icons {
            if let Ok(img) = image::load_from_memory(png_bytes) {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let color_image = ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    rgba.as_raw(),
                );
                let texture = ctx.load_texture(
                    format!("icon_{}", binary),
                    color_image,
                    TextureOptions::NEAREST,
                );
                self.icon_textures.insert(binary.to_string(), texture);
            }
        }
    }

    /// Get the icon rect for a given app binary
    fn get_icon_rect(&self, binary: &str) -> Option<Rect> {
        self.icon_rects
            .iter()
            .find(|(b, _)| b == binary)
            .map(|(_, r)| *r)
    }

    /// Calculate the target window rect for animations
    fn get_window_rect(&self) -> Rect {
        // Center of screen, standard app window size
        let center = self.screen_rect.center();
        Rect::from_center_size(center, Vec2::new(720.0, 520.0))
    }

    /// Launch an app with animation
    fn launch_app_animated(&mut self, binary: &str) {
        // Don't launch if already animating or running
        if self.animations.is_app_animating(binary) {
            return;
        }

        if self.process_manager.is_running(binary) {
            self.set_status(format!("{} is already running", binary));
            return;
        }

        // Get icon position for animation start
        if let Some(icon_rect) = self.get_icon_rect(binary) {
            let window_rect = self.get_window_rect();
            self.animations
                .start_open_to(icon_rect, window_rect, binary.to_string());
            self.set_status(format!("opening {}...", binary));
        } else {
            // Fallback: launch immediately without animation
            self.launch_app_direct(binary);
        }
    }

    /// Launch an app directly (after animation or as fallback)
    fn launch_app_direct(&mut self, binary: &str) {
        match self.process_manager.launch(binary) {
            Ok(true) => {
                self.set_status(format!("{} launched", binary));
            }
            Ok(false) => {
                self.set_status(format!("{} is already running", binary));
            }
            Err(e) => {
                self.set_status(format!("error: {}", e));
                eprintln!("[slowdesktop] launch error: {}", e);
            }
        }
    }

    /// Draw the desktop background
    fn draw_background(&self, ui: &mut Ui) {
        let rect = ui.available_rect_before_wrap();
        let painter = ui.painter();

        // Clean white background
        painter.rect_filled(rect, 0.0, SlowColors::WHITE);
    }

    /// Draw an icon label (dithered+white when selected, white bg+black when not)
    fn draw_icon_label(painter: &Painter, pos: Pos2, text: &str, selected: bool) {
        let label_rect = Rect::from_min_size(
            Pos2::new(pos.x - 8.0, pos.y + 52.0),
            Vec2::new(ICON_SIZE + 16.0, ICON_LABEL_HEIGHT),
        );
        let (bg, fg) = if selected {
            (None, SlowColors::WHITE)
        } else {
            (Some(SlowColors::WHITE), SlowColors::BLACK)
        };
        if selected {
            dither::draw_dither_selection(painter, label_rect);
        }
        if let Some(bg) = bg {
            painter.rect_filled(label_rect, 0.0, bg);
        }
        painter.text(
            label_rect.center(), Align2::CENTER_CENTER,
            text, FontId::proportional(11.0), fg,
        );
    }

    /// Draw a single desktop icon
    fn draw_icon(
        &self,
        ui: &mut Ui,
        pos: Pos2,
        app: &AppInfo,
        index: usize,
    ) -> Response {
        // Use a larger clickable area for easier interaction
        let total_rect =
            Rect::from_min_size(
                Pos2::new(pos.x - 8.0, pos.y),
                Vec2::new(ICON_SIZE + 16.0, ICON_TOTAL_HEIGHT + 4.0)
            );

        // Use Sense::click() for reliable click detection
        let response = ui.allocate_rect(total_rect, Sense::click());
        let painter = ui.painter();
        let is_selected = self.selected_icons.contains(&index);
        let is_hovered = self.hovered_icon == Some(index) || response.hovered();
        let is_animating = self.animations.is_app_animating(&app.binary);

        // Icon box
        let icon_rect =
            Rect::from_min_size(Pos2::new(pos.x + (ICON_SIZE - 48.0) / 2.0, pos.y), Vec2::new(48.0, 48.0));

        // Draw icon background (no outline)
        painter.rect_filled(icon_rect, 0.0, SlowColors::WHITE);

        // Hover effect: subtle dither overlay on icon
        if is_hovered && !is_selected && !is_animating {
            dither::draw_dither_hover(painter, icon_rect);
        }

        // Selected effect: dithered overlay on icon
        if is_selected && !is_animating {
            dither::draw_dither_selection(painter, icon_rect);
        }

        // Animating effect: pulsing dither
        if is_animating {
            dither::draw_dither_selection(painter, icon_rect);
        }

        // Running indicator: filled top-right corner
        if app.running {
            let indicator_rect = Rect::from_min_size(
                Pos2::new(icon_rect.max.x - 10.0, icon_rect.min.y),
                Vec2::new(10.0, 10.0),
            );
            painter.rect_filled(indicator_rect, 0.0, SlowColors::BLACK);
        }

        // Icon image or fallback glyph
        if let Some(tex) = self.icon_textures.get(&app.binary) {
            painter.image(
                tex.id(),
                icon_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        } else {
            let glyph_color = if is_selected || is_animating {
                SlowColors::WHITE
            } else {
                SlowColors::BLACK
            };
            painter.text(
                icon_rect.center(),
                Align2::CENTER_CENTER,
                &app.icon_label,
                FontId::proportional(20.0),
                glyph_color,
            );
        }

        Self::draw_icon_label(painter, pos, &app.display_name, is_selected || is_animating);

        response.clone().on_hover_text(&app.description)
    }

    /// Draw a single desktop folder icon
    fn draw_folder_icon(
        &self,
        ui: &mut Ui,
        pos: Pos2,
        name: &str,
        index: usize,
    ) -> Response {
        let total_rect = Rect::from_min_size(
            Pos2::new(pos.x - 8.0, pos.y),
            Vec2::new(ICON_SIZE + 16.0, ICON_TOTAL_HEIGHT + 4.0),
        );
        let response = ui.allocate_rect(total_rect, Sense::click());
        let painter = ui.painter();
        let is_selected = self.selected_folders.contains(&index);
        let is_hovered = self.hovered_folder == Some(index) || response.hovered();

        let icon_rect = Rect::from_min_size(
            Pos2::new(pos.x + (ICON_SIZE - 48.0) / 2.0, pos.y),
            Vec2::new(48.0, 48.0),
        );

        painter.rect_filled(icon_rect, 0.0, SlowColors::WHITE);

        if is_hovered && !is_selected {
            dither::draw_dither_hover(painter, icon_rect);
        }
        if is_selected {
            dither::draw_dither_selection(painter, icon_rect);
        }

        // Map folder name to specific icon key
        let icon_key = match name {
            "documents" => "folder_documents",
            "books" => "folder_books",
            "pictures" => "folder_pictures",
            "music" => "folder_music",
            "midi" => "folder_midi",
            _ => "folder",
        };

        // Use the folder-specific icon texture
        if let Some(tex) = self.icon_textures.get(icon_key) {
            painter.image(
                tex.id(),
                icon_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }

        Self::draw_icon_label(painter, pos, name, is_selected);

        response
    }

    /// Open a desktop folder by launching slowFiles with the folder path
    fn open_folder(&mut self, index: usize) {
        if index >= self.desktop_folders.len() {
            return;
        }
        let path = &self.desktop_folders[index].path;
        let _ = std::fs::create_dir_all(path);
        let path_str = path.to_string_lossy().to_string();
        match self.process_manager.launch_with_args("slowfiles", &[&path_str]) {
            Ok(true) => self.set_status(format!("opening {}...", self.desktop_folders[index].name)),
            Ok(false) => self.set_status("files is already running".to_string()),
            Err(e) => self.set_status(format!("error: {}", e)),
        }
    }

    /// Draw the menu bar
    fn draw_menu_bar(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("menu_bar")
            .exact_height(MENU_BAR_HEIGHT)
            .frame(
                egui::Frame::none()
                    .fill(SlowColors::WHITE)
                    .stroke(Stroke::new(1.0, SlowColors::BLACK))
                    .inner_margin(egui::Margin::symmetric(4.0, 0.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // Hourglass / system menu
                    ui.menu_button("slowOS", |ui| {
                        if ui.button("about").clicked() {
                            self.show_about = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("shut down...").clicked() {
                            self.show_shutdown = true;
                            ui.close_menu();
                        }
                    });

                    ui.separator();

                    // Apps menu
                    ui.menu_button("apps", |ui| {
                        let apps: Vec<(String, String)> = self
                            .process_manager
                            .apps()
                            .iter()
                            .map(|a| (a.binary.clone(), a.display_name.clone()))
                            .collect();
                        for (binary, display_name) in apps {
                            let running = self.process_manager.is_running(&binary);
                            let label = if running {
                                format!("{} (running)", display_name)
                            } else {
                                display_name
                            };
                            if ui.button(label).clicked() {
                                self.launch_app_animated(&binary);
                                ui.close_menu();
                            }
                        }
                    });

                    // Date, clock, and search on the right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Padding from right edge
                        ui.add_space(12.0);

                        // Search button
                        if ui.add(egui::Label::new(
                            egui::RichText::new("🔍")
                                .font(FontId::proportional(12.0))
                                .color(SlowColors::BLACK),
                        ).sense(Sense::click())).clicked() {
                            self.show_search = !self.show_search;
                            if self.show_search {
                                self.search_query.clear();
                                self.search_opened_frame = self.frame_count;
                            }
                        }

                        ui.add_space(8.0);

                        // Separator
                        ui.label(
                            egui::RichText::new("|")
                                .font(FontId::proportional(12.0))
                                .color(SlowColors::BLACK),
                        );

                        ui.add_space(8.0);

                        // Time (click to toggle format)
                        let now = Local::now();
                        let time = if self.use_24h_time {
                            now.format("%H:%M").to_string()
                        } else {
                            now.format("%l:%M %p").to_string().trim_start().to_string()
                        };
                        if ui.add(egui::Label::new(
                            egui::RichText::new(&time)
                                .font(FontId::proportional(12.0))
                                .color(SlowColors::BLACK),
                        ).sense(Sense::click())).clicked() {
                            self.use_24h_time = !self.use_24h_time;
                        }

                        ui.add_space(8.0);

                        // Separator
                        ui.label(
                            egui::RichText::new("|")
                                .font(FontId::proportional(12.0))
                                .color(SlowColors::BLACK),
                        );

                        ui.add_space(8.0);

                        // Date (click to cycle format)
                        let date = match self.date_format {
                            0 => now.format("%a %b %d").to_string(), // Mon Jan 15
                            1 => now.format("%m/%d").to_string(),    // 01/15
                            2 => now.format("%d/%m").to_string(),    // 15/01
                            _ => now.format("%Y-%m-%d").to_string(), // 2024-01-15
                        };
                        if ui.add(egui::Label::new(
                            egui::RichText::new(&date)
                                .font(FontId::proportional(12.0))
                                .color(SlowColors::BLACK),
                        ).sense(Sense::click())).clicked() {
                            self.date_format = (self.date_format + 1) % 4;
                        }
                    });
                });
            });
    }

    /// Draw the status bar at the bottom
    fn draw_status_bar(&self, ctx: &Context) {
        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(20.0)
            .frame(
                egui::Frame::none()
                    .fill(SlowColors::WHITE)
                    .stroke(Stroke::new(1.0, SlowColors::BLACK))
                    .inner_margin(egui::Margin::symmetric(8.0, 2.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Show status message if recent (last 5 seconds)
                    if self.status_time.elapsed().as_secs() < 5 {
                        ui.label(
                            egui::RichText::new(&self.status_message)
                                .font(FontId::proportional(11.0))
                                .color(SlowColors::BLACK),
                        );
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let running = self.process_manager.running_count();
                        let animating = self.animations.animation_count();

                        let text = if animating > 0 {
                            "loading...".to_string()
                        } else if running == 0 {
                            "no apps running".to_string()
                        } else if running == 1 {
                            "1 app running".to_string()
                        } else {
                            format!("{} apps running", running)
                        };
                        ui.label(
                            egui::RichText::new(text)
                                .font(FontId::proportional(11.0))
                                .color(SlowColors::BLACK),
                        );
                    });
                });
            });
    }

    /// Draw the about dialog
    fn draw_about(&mut self, ctx: &Context) {
        if !self.show_about {
            return;
        }
        egui::Window::new("about slowOS")
            .collapsible(false)
            .resizable(false)
            .default_width(320.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    ui.heading("slowOS");
                    ui.add_space(4.0);
                    ui.label("version 0.1.0");
                    ui.add_space(12.0);
                    ui.label("a minimal operating system");
                    ui.label("for focused computing");
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // System info
                    let num_apps = self.process_manager.apps().len();
                    ui.label(format!("{} applications installed", num_apps));

                    let running = self.process_manager.running_count();
                    if running > 0 {
                        ui.label(format!("{} currently running", running));
                    }

                    ui.add_space(4.0);

                    let date = Local::now().format("%A, %B %d, %Y").to_string();
                    ui.label(date);

                    ui.add_space(12.0);
                    ui.label("the slow computer company");

                    ui.add_space(12.0);
                    if ui.button("ok").clicked() {
                        self.show_about = false;
                    }
                    ui.add_space(4.0);
                });
            });
    }

    /// Draw the shutdown confirmation dialog
    fn draw_shutdown(&mut self, ctx: &Context) {
        if !self.show_shutdown {
            return;
        }
        egui::Window::new("shut down")
            .collapsible(false)
            .resizable(false)
            .default_width(320.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    let running = self.process_manager.running_count();
                    if running > 0 {
                        ui.label(format!(
                            "{} app{} still running.",
                            running,
                            if running == 1 { " is" } else { "s are" }
                        ));
                        ui.label("these will be closed.");
                    } else {
                        ui.label("choose an action:");
                    }
                    ui.add_space(12.0);
                });
                ui.horizontal(|ui| {
                    if ui.button("cancel").clicked() {
                        self.show_shutdown = false;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("shut down").clicked() {
                            self.process_manager.shutdown_all();
                            if std::path::Path::new("/sbin/poweroff").exists() {
                                let _ = std::process::Command::new("/sbin/poweroff").spawn();
                            }
                            std::process::exit(0);
                        }
                        if ui.button("restart").clicked() {
                            self.process_manager.shutdown_all();
                            // Try system reboot first (for embedded/buildroot)
                            if std::path::Path::new("/sbin/reboot").exists() {
                                let _ = std::process::Command::new("/sbin/reboot").spawn();
                            } else {
                                // Restart the desktop app itself
                                if let Ok(exe) = std::env::current_exe() {
                                    #[cfg(unix)]
                                    {
                                        use std::os::unix::process::CommandExt;
                                        // Fork a new process that's fully detached
                                        let _ = std::process::Command::new(&exe)
                                            .stdin(std::process::Stdio::null())
                                            .stdout(std::process::Stdio::null())
                                            .stderr(std::process::Stdio::null())
                                            .process_group(0)
                                            .spawn();
                                    }
                                    #[cfg(not(unix))]
                                    {
                                        let _ = std::process::Command::new(&exe).spawn();
                                    }
                                }
                            }
                            std::process::exit(0);
                        }
                    });
                });
                ui.add_space(4.0);
            });
    }

    /// Draw the spotlight search overlay
    fn draw_search(&mut self, ctx: &Context) {
        if !self.show_search {
            return;
        }

        // Pin search window to fixed position near top-right
        let screen = ctx.screen_rect();
        let search_pos = Pos2::new(screen.max.x - 304.0, screen.min.y + 4.0);
        let response = egui::Window::new("search")
            .collapsible(false)
            .resizable(false)
            .movable(false)
            .title_bar(false)
            .fixed_pos(search_pos)
            .fixed_size(Vec2::new(280.0, 300.0))
            .frame(
                egui::Frame::none()
                    .fill(SlowColors::WHITE)
                    .stroke(Stroke::new(1.0, SlowColors::BLACK))
                    .inner_margin(egui::Margin::same(8.0)),
            )
            .show(ctx, |ui| {
                ui.set_min_width(264.0);
                ui.set_max_width(264.0);
                // Search input - always request focus when search is open
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .hint_text("search apps and files...")
                        .desired_width(260.0)
                );
                r.request_focus();

                let query = self.search_query.to_lowercase();

                // Always show results area with fixed height to prevent bounce
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                let mut launch_binary: Option<String> = None;
                let mut open_file: Option<std::path::PathBuf> = None;

                egui::ScrollArea::vertical()
                    .max_height(256.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                    if query.is_empty() {
                        ui.weak("type to search apps and files...");
                    } else {
                        // Search apps
                        let app_matches: Vec<(String, String, bool)> = self.process_manager.apps().iter()
                            .filter(|a| {
                                self.process_manager.binary_exists(&a.binary) && (
                                    a.display_name.to_lowercase().contains(&query) ||
                                    a.description.to_lowercase().contains(&query) ||
                                    a.binary.to_lowercase().contains(&query)
                                )
                            })
                            .map(|a| (a.binary.clone(), a.display_name.clone(), a.running))
                            .collect();

                        let file_matches = self.search_files(&query);

                        let has_results = !app_matches.is_empty() || !file_matches.is_empty();

                        if has_results {
                            if !app_matches.is_empty() {
                                ui.label("apps:");
                                for (binary, display_name, running) in &app_matches {
                                    let label = if *running {
                                        format!("  {} (running)", display_name)
                                    } else {
                                        format!("  {}", display_name)
                                    };
                                    if ui.selectable_label(false, &label).clicked() {
                                        launch_binary = Some(binary.clone());
                                    }
                                }
                            }

                            if !file_matches.is_empty() {
                                if !app_matches.is_empty() {
                                    ui.add_space(4.0);
                                }
                                ui.label("files:");
                                for (path, name) in &file_matches {
                                    if ui.selectable_label(false, &format!("  {}", name)).clicked() {
                                        open_file = Some(path.clone());
                                    }
                                }
                            }
                        } else {
                            ui.label("no results");
                        }
                    }
                });

                // Handle Enter to launch first match
                if !query.is_empty() {
                    let enter_pressed = ui.input(|i| i.key_pressed(Key::Enter));
                    if enter_pressed {
                        let app_matches: Vec<(String, String, bool)> = self.process_manager.apps().iter()
                            .filter(|a| {
                                self.process_manager.binary_exists(&a.binary) && (
                                    a.display_name.to_lowercase().contains(&query) ||
                                    a.description.to_lowercase().contains(&query) ||
                                    a.binary.to_lowercase().contains(&query)
                                )
                            })
                            .map(|a| (a.binary.clone(), a.display_name.clone(), a.running))
                            .collect();
                        if !app_matches.is_empty() {
                            launch_binary = Some(app_matches[0].0.clone());
                        } else {
                            let file_matches = self.search_files(&query);
                            if !file_matches.is_empty() {
                                open_file = Some(file_matches[0].0.clone());
                            }
                        }
                    }
                }

                if let Some(binary) = launch_binary {
                    self.show_search = false;
                    self.search_query.clear();
                    self.launch_app_animated(&binary);
                }

                if let Some(path) = open_file {
                    self.show_search = false;
                    self.search_query.clear();
                    self.open_file_with_app(&path);
                }
            });

        // Close if clicked outside the search window (on mouse release to avoid race conditions)
        // Skip this check for the first 2 frames after opening to prevent immediate close
        let frames_since_opened = self.frame_count.saturating_sub(self.search_opened_frame);
        if frames_since_opened >= 2 {
            if let Some(inner) = response {
                let window_rect = inner.response.rect;
                let primary_released = ctx.input(|i| i.pointer.primary_released());
                let pointer_pos = ctx.input(|i| i.pointer.interact_pos());

                if primary_released {
                    if let Some(pos) = pointer_pos {
                        if !window_rect.contains(pos) {
                            self.show_search = false;
                            self.search_query.clear();
                        }
                    }
                }
            }
        }
    }

    /// Search files and folders in common directories (books, music, documents, pictures)
    fn search_files(&self, query: &str) -> Vec<(std::path::PathBuf, String)> {
        let mut results = Vec::new();
        let home = dirs::home_dir().unwrap_or_default();

        // Directories to search
        let search_dirs = [
            home.join("Books"),
            home.join("Books").join("slowLibrary"),
            home.join("Music"),
            home.join("Documents"),
            home.join("Pictures"),
            home.join("Pictures").join("slowMuseum"),
            home.join("MIDI"),
        ];

        // File extensions to include
        let extensions = ["epub", "txt", "rtf", "mp3", "wav", "midi", "mid",
                          "png", "jpg", "jpeg", "gif", "bmp", "pdf"];

        for dir in &search_dirs {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();

                    // Skip hidden files
                    if name.starts_with('.') {
                        continue;
                    }

                    if name.to_lowercase().contains(query) {
                        if path.is_dir() {
                            // Include folders
                            results.push((path, format!("{}/", name)));
                        } else if path.is_file() {
                            let ext = path.extension()
                                .and_then(|e| e.to_str())
                                .map(|e| e.to_lowercase())
                                .unwrap_or_default();
                            if extensions.contains(&ext.as_str()) {
                                results.push((path, name));
                            }
                        }
                    }
                }
            }
        }

        // Sort results: folders first, then files
        results.sort_by(|a, b| {
            let a_is_dir = a.1.ends_with('/');
            let b_is_dir = b.1.ends_with('/');
            b_is_dir.cmp(&a_is_dir).then(a.1.cmp(&b.1))
        });

        // Limit results to avoid overwhelming the UI
        results.truncate(12);
        results
    }

    /// Open a file or folder with the appropriate application
    fn open_file_with_app(&mut self, path: &std::path::Path) {
        // Handle directories - open in slowfiles
        if path.is_dir() {
            let path_str = path.to_string_lossy().to_string();
            let _ = self.process_manager.launch_with_args("slowfiles", &[&path_str]);
            return;
        }

        let ext = path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        let app = match ext.as_str() {
            "epub" => Some("slowreader"),
            "txt" | "rtf" => Some("slowwrite"),
            "mp3" | "wav" => Some("slowmusic"),
            "midi" | "mid" => Some("slowmidi"),
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "pdf" => Some("slowview"),
            _ => None,
        };

        if let Some(app_name) = app {
            let path_str = path.to_string_lossy().to_string();
            let _ = self.process_manager.launch_with_args(app_name, &[&path_str]);
        }
    }

    /// Handle keyboard shortcuts
    fn handle_keys(&mut self, ctx: &Context) {
        ctx.input(|i| {
            let cmd = i.modifiers.command;

            // Cmd+Q: show shutdown dialog
            if cmd && i.key_pressed(Key::Q) {
                self.show_shutdown = true;
            }

            // Cmd+Space: toggle search
            if cmd && i.key_pressed(Key::Space) {
                self.show_search = !self.show_search;
                if self.show_search {
                    self.search_query.clear();
                    self.search_opened_frame = self.frame_count;
                }
            }

            // Escape: close search, dialogs, deselect, or cancel marquee
            if i.key_pressed(Key::Escape) {
                if self.marquee_start.is_some() {
                    self.marquee_start = None;
                } else if self.show_search {
                    self.show_search = false;
                    self.search_query.clear();
                } else if self.show_about {
                    self.show_about = false;
                } else if self.show_shutdown {
                    self.show_shutdown = false;
                } else {
                    self.selected_icons.clear();
                    self.selected_folders.clear();
                }
            }

            // Arrow keys: navigate whichever side has selection
            if !self.selected_folders.is_empty() {
                // Folders on LEFT side, bottom-aligned, columns going right
                if i.key_pressed(Key::ArrowDown) { self.navigate_folders(1); }
                if i.key_pressed(Key::ArrowUp) { self.navigate_folders(-1); }
                if i.key_pressed(Key::ArrowRight) { self.navigate_folders(ICONS_PER_COLUMN as i32); }
                if i.key_pressed(Key::ArrowLeft) { self.navigate_folders(-(ICONS_PER_COLUMN as i32)); }
            } else {
                // Apps on RIGHT side, top-aligned, columns going left
                if i.key_pressed(Key::ArrowDown) { self.navigate_icons(1); }
                if i.key_pressed(Key::ArrowUp) { self.navigate_icons(-1); }
                if i.key_pressed(Key::ArrowLeft) { self.navigate_icons(ICONS_PER_COLUMN as i32); }
                if i.key_pressed(Key::ArrowRight) { self.navigate_icons(-(ICONS_PER_COLUMN as i32)); }
            }
        });

        // Handle Enter key outside of input closure
        let enter_pressed = ctx.input(|i| i.key_pressed(Key::Enter));

        if enter_pressed {
            // Launch first selected folder
            if let Some(&index) = self.selected_folders.iter().next() {
                if index == self.desktop_folders.len() {
                    // Trash
                    self.launch_app_animated("trash");
                } else {
                    // Animate from folder icon and launch slowFiles
                    if let Some(&rect) = self.folder_icon_rects.get(index) {
                        self.last_folder_launch_rect = Some(rect);
                        let window_rect = self.get_window_rect();
                        self.animations.start_close(rect, window_rect, "slowfiles".to_string());
                    }
                    self.open_folder(index);
                }
            // Launch first selected app
            } else if let Some(&index) = self.selected_icons.iter().next() {
                let apps: Vec<String> = self
                    .process_manager
                    .apps()
                    .iter()
                    .map(|a| a.binary.clone())
                    .collect();
                if let Some(binary) = apps.get(index) {
                    let binary = binary.clone();
                    self.selected_icons.clear();
                    self.launch_app_animated(&binary);
                }
            }
        }
    }

    /// Navigate between icons with arrow keys
    fn navigate_icons(&mut self, delta: i32) {
        let app_count = self.process_manager.apps().len() as i32;
        if app_count == 0 {
            return;
        }

        let current = self.selected_icons.iter().next().copied().unwrap_or(0) as i32;
        let new_index = (current + delta).rem_euclid(app_count);
        self.selected_icons.clear();
        self.selected_icons.insert(new_index as usize);
    }

    /// Navigate between folders with arrow keys (includes trash as last item)
    fn navigate_folders(&mut self, delta: i32) {
        let count = (self.desktop_folders.len() + 1) as i32; // +1 for trash
        if count == 0 {
            return;
        }
        let current = self.selected_folders.iter().next().copied().unwrap_or(0) as i32;
        let new_index = (current + delta).rem_euclid(count);
        self.selected_folders.clear();
        self.selected_folders.insert(new_index as usize);
    }
}

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Load icon textures on first frame
        self.load_icon_textures(ctx);

        // Consume Tab key to prevent menu focus issues
        slowcore::theme::consume_special_keys(ctx);

        // Calculate delta time
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;

        // Update animations and get apps ready to launch
        let apps_to_launch = self.animations.update(dt);
        for binary in apps_to_launch {
            self.launch_app_direct(&binary);
        }

        // Poll running processes periodically (every ~30 frames ~ 0.5s)
        self.frame_count += 1;
        if self.frame_count % 30 == 0 {
            let exited = self.process_manager.poll();
            for binary in &exited {
                self.set_status(format!("{} has quit", binary));

                // For slowFiles launched from a folder, animate back to the folder icon
                let target_rect = if binary == "slowfiles" {
                    self.last_folder_launch_rect.take()
                        .or_else(|| self.get_icon_rect(binary))
                } else {
                    self.get_icon_rect(binary)
                };

                // Start close animation from center of screen to icon
                if let Some(icon_rect) = target_rect {
                    let window_rect = self.get_window_rect();
                    self.animations.start_close(window_rect, icon_rect, binary.clone());
                }
            }
        }

        // Request repaint for animations, clock, and status updates
        if self.animations.is_animating() {
            ctx.request_repaint_after(Duration::from_millis(33)); // 30 FPS for Pi
        } else {
            ctx.request_repaint_after(Duration::from_secs(1));
        }

        self.handle_keys(ctx);
        self.draw_menu_bar(ctx);
        self.draw_status_bar(ctx);

        // Main desktop area
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE))
            .show(ctx, |ui| {
                // Update screen rect
                self.screen_rect = ui.available_rect_before_wrap();

                // Draw dithered background
                self.draw_background(ui);

                let available = ui.available_rect_before_wrap();

                // === RIGHT SIDE: Application icons (top-aligned, columns going left) ===
                let app_start_x = available.max.x - DESKTOP_PADDING - ICON_SIZE;
                let app_start_y = available.min.y + DESKTOP_PADDING;

                // Build filtered app indices once
                let app_indices: Vec<usize> = self.process_manager.apps()
                    .iter().enumerate()
                    .filter(|(_, a)| a.binary != "trash")
                    .map(|(i, _)| i)
                    .collect();

                self.icon_rects.clear();

                let mut clicked_icon: Option<(usize, String)> = None;
                let mut new_hovered_icon: Option<usize> = None;

                for (display_idx, &app_idx) in app_indices.iter().enumerate() {
                    let app = &self.process_manager.apps()[app_idx];
                    let col = display_idx / ICONS_PER_COLUMN;
                    let row = display_idx % ICONS_PER_COLUMN;

                    let x = app_start_x - col as f32 * ICON_SPACING;
                    let y = app_start_y + row as f32 * (ICON_TOTAL_HEIGHT + 8.0);

                    let pos = Pos2::new(x, y);
                    let binary = app.binary.as_str();
                    let response = self.draw_icon(ui, pos, app, display_idx);

                    let icon_rect = Rect::from_min_size(
                        Pos2::new(pos.x + (ICON_SIZE - 48.0) / 2.0, pos.y),
                        Vec2::new(48.0, 48.0),
                    );
                    self.icon_rects.push((binary.to_string(), icon_rect));

                    if response.hovered() {
                        new_hovered_icon = Some(display_idx);
                    }
                    if response.clicked() {
                        clicked_icon = Some((display_idx, binary.to_string()));
                    }
                }

                self.hovered_icon = new_hovered_icon;

                // Handle app icon clicks
                let icon_was_clicked = if let Some((index, ref binary)) = clicked_icon {
                    let now = Instant::now();
                    let is_double_click = self.last_click_index == Some(index)
                        && now.duration_since(self.last_click_time).as_millis() < DOUBLE_CLICK_MS;

                    if is_double_click {
                        self.selected_icons.clear();
                        self.launch_app_animated(binary);
                    } else {
                        self.selected_icons.clear();
                        self.selected_icons.insert(index);
                        self.selected_folders.clear();
                    }

                    self.last_click_time = now;
                    self.last_click_index = Some(index);
                    true
                } else {
                    false
                };

                // === LEFT SIDE: Folder icons + trash (bottom-aligned) ===
                let folder_start_x = available.min.x + DESKTOP_PADDING;
                let folder_bottom_y = available.max.y - DESKTOP_PADDING - ICON_TOTAL_HEIGHT - 8.0;

                let folder_names: Vec<&str> = self.desktop_folders.iter()
                    .map(|f| f.name)
                    .collect();
                let total_folder_items = folder_names.len() + 1; // +1 for trash

                let mut clicked_folder: Option<usize> = None;
                let mut new_hovered_folder: Option<usize> = None;

                // Draw folder icons (index 0 at top, last at bottom)
                self.folder_icon_rects.clear();
                for (index, name) in folder_names.iter().enumerate() {
                    let col = index / ICONS_PER_COLUMN;
                    let row_from_bottom = (total_folder_items - 1 - index) % ICONS_PER_COLUMN;
                    let x = folder_start_x + col as f32 * ICON_SPACING;
                    let y = folder_bottom_y - row_from_bottom as f32 * (ICON_TOTAL_HEIGHT + 8.0);
                    let pos = Pos2::new(x, y);

                    let response = self.draw_folder_icon(ui, pos, name, index);
                    let folder_icon_rect = Rect::from_min_size(
                        Pos2::new(pos.x + (ICON_SIZE - 48.0) / 2.0, pos.y),
                        Vec2::new(48.0, 48.0),
                    );
                    self.folder_icon_rects.push(folder_icon_rect);
                    if response.hovered() {
                        new_hovered_folder = Some(index);
                    }
                    if response.clicked() {
                        clicked_folder = Some(index);
                    }
                }

                // Draw trash icon as last folder item (at the bottom)
                {
                    let trash_index = folder_names.len();
                    let col = trash_index / ICONS_PER_COLUMN;
                    let row_from_bottom = (total_folder_items - 1 - trash_index) % ICONS_PER_COLUMN;
                    let x = folder_start_x + col as f32 * ICON_SPACING;
                    let y = folder_bottom_y - row_from_bottom as f32 * (ICON_TOTAL_HEIGHT + 8.0);
                    let pos = Pos2::new(x, y);

                    let total_rect = Rect::from_min_size(
                        Pos2::new(pos.x - 8.0, pos.y),
                        Vec2::new(ICON_SIZE + 16.0, ICON_TOTAL_HEIGHT + 4.0),
                    );
                    let response = ui.allocate_rect(total_rect, Sense::click());
                    let painter = ui.painter();
                    let is_selected = self.selected_folders.contains(&trash_index);
                    let is_hovered = self.hovered_folder == Some(trash_index) || response.hovered();

                    let icon_rect = Rect::from_min_size(
                        Pos2::new(pos.x + (ICON_SIZE - 48.0) / 2.0, pos.y),
                        Vec2::new(48.0, 48.0),
                    );
                    painter.rect_filled(icon_rect, 0.0, SlowColors::WHITE);
                    if is_hovered && !is_selected {
                        dither::draw_dither_hover(painter, icon_rect);
                    }
                    if is_selected {
                        dither::draw_dither_selection(painter, icon_rect);
                    }
                    if let Some(tex) = self.icon_textures.get("trash") {
                        painter.image(
                            tex.id(),
                            icon_rect,
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }
                    Self::draw_icon_label(painter, pos, "trash", is_selected);
                    if response.hovered() {
                        new_hovered_folder = Some(trash_index);
                    }
                    if response.clicked() {
                        clicked_folder = Some(trash_index);
                    }
                    // Cache trash icon rect for animations
                    self.icon_rects.push(("trash".to_string(), icon_rect));
                }

                self.hovered_folder = new_hovered_folder;

                // Handle folder clicks
                let folder_was_clicked = if let Some(index) = clicked_folder {
                    let now = Instant::now();
                    let is_double_click = self.last_folder_click_index == Some(index)
                        && now.duration_since(self.last_folder_click_time).as_millis() < DOUBLE_CLICK_MS;

                    if is_double_click {
                        self.selected_folders.clear();
                        if index == self.desktop_folders.len() {
                            // Trash icon double-clicked
                            self.launch_app_animated("trash");
                        } else {
                            // Animate from folder icon and launch slowFiles
                            if let Some(&rect) = self.folder_icon_rects.get(index) {
                                self.last_folder_launch_rect = Some(rect);
                                let window_rect = self.get_window_rect();
                                // Visual-only animation (start_close doesn't queue a launch)
                                self.animations.start_close(rect, window_rect, "slowfiles".to_string());
                            }
                            self.open_folder(index);
                        }
                    } else {
                        self.selected_folders.clear();
                        self.selected_folders.insert(index);
                        self.selected_icons.clear();
                    }

                    self.last_folder_click_time = now;
                    self.last_folder_click_index = Some(index);
                    true
                } else {
                    false
                };

                // Get pointer state for marquee
                let pointer_pos = ui.input(|i| i.pointer.interact_pos());
                let primary_down = ui.input(|i| i.pointer.primary_down());
                let primary_pressed = ui.input(|i| i.pointer.primary_pressed());
                let primary_released = ui.input(|i| i.pointer.primary_released());

                // Start marquee when clicking on empty space
                if primary_pressed && !icon_was_clicked && !folder_was_clicked {
                    if let Some(pos) = pointer_pos {
                        // Check if click is on any icon
                        let on_app_icon = self.icon_rects.iter().any(|(_, r)| r.contains(pos));
                        let on_folder_icon = self.folder_icon_rects.iter().any(|r| r.contains(pos));
                        if !on_app_icon && !on_folder_icon {
                            self.marquee_start = Some(pos);
                            self.selected_icons.clear();
                            self.selected_folders.clear();
                        }
                    }
                }

                // Draw marquee rectangle if active
                if let (Some(start), Some(current)) = (self.marquee_start, pointer_pos) {
                    if primary_down {
                        let painter = ui.painter();
                        let marquee_rect = Rect::from_two_pos(start, current);
                        painter.rect_stroke(
                            marquee_rect,
                            0.0,
                            Stroke::new(1.0, SlowColors::BLACK),
                        );

                        // Highlight icons that intersect with marquee
                        for (index, (_, rect)) in self.icon_rects.iter().enumerate() {
                            if rect.intersects(marquee_rect) {
                                self.selected_icons.insert(index);
                            } else {
                                self.selected_icons.remove(&index);
                            }
                        }
                        for (index, rect) in self.folder_icon_rects.iter().enumerate() {
                            if rect.intersects(marquee_rect) {
                                self.selected_folders.insert(index);
                            } else {
                                self.selected_folders.remove(&index);
                            }
                        }
                        // Check trash icon too (it's at folder_rects index = desktop_folders.len())
                        let trash_index = self.desktop_folders.len();
                        if let Some((_, trash_rect)) = self.icon_rects.iter().find(|(name, _)| name == "trash") {
                            if trash_rect.intersects(marquee_rect) {
                                self.selected_folders.insert(trash_index);
                            } else {
                                self.selected_folders.remove(&trash_index);
                            }
                        }

                        ui.ctx().request_repaint_after(Duration::from_millis(33));
                    }
                }

                // Finalize marquee selection on release
                if primary_released && self.marquee_start.is_some() {
                    self.marquee_start = None;
                }

                // Deselect when clicking empty space (only if not marquee)
                if !icon_was_clicked && !folder_was_clicked && self.marquee_start.is_none() {
                    if !self.selected_icons.is_empty() || !self.selected_folders.is_empty() {
                        let pointer_clicked = ui.input(|i| i.pointer.any_click());
                        if pointer_clicked {
                            // Check we're not clicking on any icon
                            if let Some(pos) = pointer_pos {
                                let on_app_icon = self.icon_rects.iter().any(|(_, r)| r.contains(pos));
                                let on_folder_icon = self.folder_icon_rects.iter().any(|r| r.contains(pos));
                                if !on_app_icon && !on_folder_icon {
                                    self.selected_icons.clear();
                                    self.selected_folders.clear();
                                }
                            }
                        }
                    }
                }

                // Draw animations on top of everything
                let painter = ui.painter();
                self.animations.draw(painter);
            });

        // Dialogs
        self.draw_about(ctx);
        self.draw_shutdown(ctx);
        self.draw_search(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.process_manager.shutdown_all();
    }
}
