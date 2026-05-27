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

use crate::input_telemetry_enabled;
use crate::process_manager::{AppInfo, ProcessManager};
use chrono::Local;
use egui::{
    Align2, ColorImage, Context, Event, FontId, Key, Painter, Pos2, Rect, Response, Sense, Stroke,
    TextureHandle, TextureOptions, Ui, Vec2,
};
use slowcore::animation::AnimationManager;
use slowcore::dither;
use slowcore::repaint::RepaintController;
use slowcore::theme::SlowColors;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
const INPUT_TELEMETRY_LOG_FILE: &str = "/tmp/slowdesktop-input-telemetry.log";
const INPUT_TELEMETRY_MAX_EVENTS: usize = 512;
const INPUT_TELEMETRY_FLUSH_BATCH: usize = 64;
const INPUT_TELEMETRY_FLUSH_INTERVAL_MS: u64 = 250;

#[derive(Clone, Copy)]
enum InputTelemetryStage {
    Ingress,
    Logic,
    Paint,
    FocusRequest,
    FocusAccept,
}

impl InputTelemetryStage {
    fn code(self) -> &'static str {
        match self {
            Self::Ingress => "I",
            Self::Logic => "L",
            Self::Paint => "P",
            Self::FocusRequest => "FR",
            Self::FocusAccept => "FA",
        }
    }
}

struct BoundedInputTelemetry {
    sequence: u64,
    events: VecDeque<String>,
    last_flush: Instant,
}

impl BoundedInputTelemetry {
    fn new() -> Self {
        Self {
            sequence: 0,
            events: VecDeque::with_capacity(INPUT_TELEMETRY_MAX_EVENTS),
            last_flush: Instant::now(),
        }
    }

    fn current_micros() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_micros())
            .unwrap_or_default()
    }

    fn append(&mut self, frame: u64, stage: InputTelemetryStage, detail: &str) {
        if self.events.len() >= INPUT_TELEMETRY_MAX_EVENTS {
            self.events.pop_front();
        }

        let line = format!(
            "{},{},f={},ts={},{}",
            self.sequence,
            stage.code(),
            frame,
            Self::current_micros(),
            detail
        );
        self.events.push_back(line);
        self.sequence = self.sequence.wrapping_add(1);
    }

    fn record_input(&mut self, frame: u64, key_down: u32, key_up: u32, text: u32, mouse: u32) {
        if key_down == 0 && key_up == 0 && text == 0 && mouse == 0 {
            return;
        }
        self.append(
            frame,
            InputTelemetryStage::Ingress,
            &format!("kd={key_down},ku={key_up},tx={text},ms={mouse}"),
        );
    }

    fn record_logic(&mut self, frame: u64, duration_micros: u128) {
        self.append(frame, InputTelemetryStage::Logic, &format!("dur_us={duration_micros}"));
    }

    fn record_paint(&mut self, frame: u64, duration_micros: u128) {
        self.append(frame, InputTelemetryStage::Paint, &format!("dur_us={duration_micros}"));
    }

    fn record_focus_request(&mut self, frame: u64, reason: &str) {
        self.append(frame, InputTelemetryStage::FocusRequest, reason);
    }

    fn record_focus_accept(&mut self, frame: u64, latency_frames: u64) {
        self.append(frame, InputTelemetryStage::FocusAccept, &format!("lat_frames={latency_frames}"));
    }

    fn flush(&mut self) {
        if self.events.is_empty() {
            return;
        }

        let mut payload = String::new();
        while let Some(line) = self.events.pop_front() {
            payload.push_str(&line);
            payload.push('\n');
        }

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(INPUT_TELEMETRY_LOG_FILE)
        {
            if let Err(e) = file.write_all(payload.as_bytes()) {
                eprintln!("[slowdesktop] telemetry write failed: {e}");
            }
            self.last_flush = Instant::now();
        }
    }

    fn flush_if_needed(&mut self) {
        let due_to_batch = self.events.len() >= INPUT_TELEMETRY_FLUSH_BATCH;
        let due_to_interval = self
            .last_flush
            .elapsed()
            .as_millis()
            >= INPUT_TELEMETRY_FLUSH_INTERVAL_MS as u128;
        if due_to_batch || due_to_interval {
            self.flush();
        }
    }

    fn flush_all(&mut self) {
        self.flush();
    }
}

/// Double-click timing threshold in milliseconds
const DOUBLE_CLICK_MS: u128 = 500;

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
    /// Optional bounded in-memory input telemetry state
    input_telemetry: Option<BoundedInputTelemetry>,
    /// Search focus is waiting for focus confirmation
    search_focus_pending: bool,
    /// Frame at which search focus request was initiated
    search_focus_request_frame: Option<u64>,
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
    /// Battery percentage (0-100)
    battery_percent: u8,
    /// Whether battery is charging
    battery_charging: bool,
    /// Last time battery was polled
    battery_last_check: Instant,
    /// Cached battery sysfs path (discovered once, reused)
    battery_sysfs_path: Option<Option<PathBuf>>,
    /// Cached filtered app indices (rebuilt only when process list changes)
    cached_app_indices: Option<Vec<usize>>,
    /// Last known number of running processes (to detect changes)
    last_running_count: usize,
    /// Full searchable file listing for spotlight (built once per open-search session when the
    /// user first types a non-empty query; filtered in-memory per keystroke — avoids repeated
    /// `read_dir` on large/NFS HOME trees).
    search_file_snapshot: Option<Vec<(std::path::PathBuf, String)>>,
    /// Cached search app rows: (normalized query, app_state_epoch, matches)
    search_app_matches_cache: Option<(String, u64, Vec<(String, String, bool)>)>,
    /// Repaint controller for partial repainting
    repaint: RepaintController,
}

impl DesktopApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let docs = dirs::document_dir().unwrap_or_else(|| home.join("Documents"));

        // Setup default content (books, pictures) on first launch — run in background
        let home_clone = home.clone();
        std::thread::spawn(move || {
            Self::setup_default_content(&home_clone);
        });

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
            status_message: "welcome to slowOS v0.2.1".to_string(),
            status_time: Instant::now(),
            frame_count: 0,
            animations: AnimationManager::new(),
            icon_rects: Vec::new(),
            last_folder_launch_rect: None,
            folder_icon_rects: Vec::new(),
            // Placeholder until CentralPanel runs; match typical Pi HDMI so early animations center sensibly.
            screen_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(1920.0, 1080.0)),
            last_frame_time: Instant::now(),
            use_24h_time: false,
            date_format: 0,
            show_search: false,
            search_query: String::new(),
            search_opened_frame: 0,
            input_telemetry: if input_telemetry_enabled() { Some(BoundedInputTelemetry::new()) } else { None },
            search_focus_pending: false,
            search_focus_request_frame: None,
            icon_textures: HashMap::new(),
            icons_loaded: false,
            desktop_folders,
            selected_folders: HashSet::new(),
            last_folder_click_time: Instant::now(),
            last_folder_click_index: None,
            hovered_folder: None,
            marquee_start: None,
            battery_percent: 100,
            battery_charging: true,
            battery_last_check: Instant::now(),
            battery_sysfs_path: None,
            cached_app_indices: None,
            last_running_count: 0,
            search_file_snapshot: None,
            search_app_matches_cache: None,
            // ~33 ms continuous tick (not 250 ms `new()` default) when selection/search/animation
            // asks for continuous repaint — avoids starving HDMI updates.
            repaint: RepaintController::with_fast_interval(),
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

        // Setup Music/Goldberg Variations
        let music_dir = home.join("Music");
        let _ = std::fs::create_dir_all(&music_dir);
        let album_name = "Kimiko Ishizaka - J.S. Bach- -Open- Goldberg Variations- BWV 988 (Piano)";
        let album_dest = music_dir.join(album_name);
        if !album_dest.exists() {
            for data_dir in &data_dirs {
                let source = data_dir.join(album_name);
                if source.is_dir() {
                    let _ = Self::copy_dir_recursive(&source, &album_dest);
                    break;
                }
            }
        }

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

    /// Discover the battery sysfs path once, cache it for future reads.
    fn find_battery_sysfs_path(&mut self) -> Option<&PathBuf> {
        if self.battery_sysfs_path.is_none() {
            let base = std::path::Path::new("/sys/class/power_supply");
            let found = std::fs::read_dir(base).ok().and_then(|entries| {
                entries.flatten().find_map(|entry| {
                    let path = entry.path();
                    if path.join("capacity").exists() {
                        Some(path)
                    } else {
                        None
                    }
                })
            });
            self.battery_sysfs_path = Some(found);
        }
        self.battery_sysfs_path.as_ref().unwrap().as_ref()
    }

    /// Poll battery status from cached sysfs path. Returns (percent, charging).
    fn read_battery(&mut self) -> (u8, bool) {
        if let Some(path) = self.find_battery_sysfs_path().cloned() {
            let percent = std::fs::read_to_string(path.join("capacity"))
                .ok()
                .and_then(|s| s.trim().parse::<u8>().ok())
                .unwrap_or(100);
            let charging = std::fs::read_to_string(path.join("status"))
                .map(|s| {
                    let s = s.trim().to_lowercase();
                    s == "charging" || s == "full"
                })
                .unwrap_or(true);
            (percent, charging)
        } else {
            // No battery found — assume plugged in
            (100, true)
        }
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
            ("slowwrite", include_bytes!("../../icons/app_icons/icons_pen.png")),
            ("slowpaint", include_bytes!("../../icons/icons_paint.png")),
            ("slowdesign", include_bytes!("../../icons/app_icons/icons_design.png")),
            ("slowreader", include_bytes!("../../icons/icons_reader.png")),
            ("slowsheets", include_bytes!("../../icons/icons_sheets_1.png")),
            ("slowchess", include_bytes!("../../icons/icons_chess.png")),
            ("slowfiles", include_bytes!("../../icons/icons_files.png")),
            ("slowmusic", include_bytes!("../../icons/icons_music.png")),
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
            ("slowclock", include_bytes!("../../icons/app_icons/icons_clock.png")),
            // Folder-specific icons
            ("folder_documents", include_bytes!("../../icons/folder_icons/icons_docsfolder.png")),
            ("folder_books", include_bytes!("../../icons/folder_icons/icons_bookfolder.png")),
            ("folder_pictures", include_bytes!("../../icons/folder_icons/icons_picturefolder.png")),
            ("folder_music", include_bytes!("../../icons/folder_icons/icons_musicfolder.png")),
            ("folder_midi", include_bytes!("../../icons/folder_icons/icons_midifolder.png")),
            // Battery icons
            ("battery_charging", include_bytes!("../../icons/system_icons/icons_batterycharging.png")),
            ("battery_low", include_bytes!("../../icons/system_icons/icons_batterylow.png")),
            ("battery_empty", include_bytes!("../../icons/system_icons/icons_emptybattery.png")),
            // System icons
            ("hourglass", include_bytes!("../../icons/system_icons/hourglass_16.png")),
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

        // Load the full-size hourglass with LINEAR filtering for the about screen
        {
            let png_bytes = include_bytes!("../../icons/system_icons/hourglass.png");
            if let Ok(img) = image::load_from_memory(png_bytes) {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let color_image = ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    rgba.as_raw(),
                );
                let texture = ctx.load_texture(
                    "icon_hourglass_large",
                    color_image,
                    TextureOptions::NEAREST,
                );
                self.icon_textures.insert("hourglass_large".to_string(), texture);
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
            self.process_manager.focus_app(binary);
            self.set_status(format!("{} is already running", binary));
            return;
        }

        // Get icon position for animation start
        if let Some(icon_rect) = self.get_icon_rect(binary) {
            let window_rect = self.get_window_rect();
            self.animations
                .start_open_to(icon_rect, window_rect, binary.to_string());
            self.set_status(format!("opening {}...", binary));
            self.launch_app_direct(binary);
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
                    ui.menu_button("slowOS", |ui| {
                        if ui.button("about").clicked() {
                            self.show_about = true;
                            ui.close_menu();
                        }
                        if ui.button("credits").clicked() {
                            self.launch_app_animated("credits");
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("shut down...").clicked() {
                            self.show_shutdown = true;
                            ui.close_menu();
                        }
                    });

                    ui.separator();

                    // Apps menu (terminal hidden — use ⌘⌥T)
                    ui.menu_button("apps", |ui| {
                        let apps: Vec<(String, String)> = self
                            .process_manager
                            .apps()
                            .iter()
                            .filter(|a| a.binary != "slowterm")
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
                            let will_show = !self.show_search;
                            self.show_search = will_show;
                            if will_show {
                                self.search_query.clear();
                                self.search_opened_frame = self.frame_count;
                                self.request_search_focus(self.frame_count, "menu-search");
                            }
                        }

                        ui.add_space(8.0);

                        // Battery indicator (text glyph)
                        {
                            // Poll battery every 30 seconds (cached sysfs path)
                            if self.battery_last_check.elapsed() > Duration::from_secs(30) {
                                let (pct, charging) = self.read_battery();
                                self.battery_percent = pct;
                                self.battery_charging = charging;
                                self.battery_last_check = Instant::now();
                            }

                            let label = if self.battery_charging {
                                format!("\u{26A1} {}%", self.battery_percent) // ⚡ + percentage
                            } else {
                                format!("{}%", self.battery_percent)
                            };

                            ui.label(
                                egui::RichText::new(&label)
                                    .font(FontId::proportional(11.0))
                                    .color(SlowColors::BLACK),
                            );
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
        let resp = egui::Window::new("about slowOS")
            .collapsible(false)
            .resizable(false)
            .default_width(320.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    if let Some(tex) = self.icon_textures.get("hourglass_large") {
                        // Source is 149x214; display at half-size for a crisp icon
                        let img_size = Vec2::new(37.0, 53.0);
                        ui.add(egui::Image::new((tex.id(), img_size)));
                        ui.add_space(4.0);
                    }
                    ui.heading("slowOS");
                    ui.add_space(4.0);
                    ui.label("version 0.2.1");
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
        if let Some(r) = &resp { slowcore::dither::draw_window_shadow(ctx, r.response.rect); }
    }

    /// Draw the shutdown confirmation dialog
    fn draw_shutdown(&mut self, ctx: &Context) {
        if !self.show_shutdown {
            return;
        }
        let resp = egui::Window::new("shut down")
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
        if let Some(r) = &resp { slowcore::dither::draw_window_shadow(ctx, r.response.rect); }
    }

    /// App-list matches for spotlight search; cached per (query, running-state epoch).
    fn search_app_matches_for_query(&mut self, query: &str) -> Vec<(String, String, bool)> {
        let epoch = self.process_manager.app_state_epoch();
        if let Some((q, e, cached)) = &self.search_app_matches_cache {
            if q.as_str() == query && *e == epoch {
                return cached.clone();
            }
        }
        let apps: Vec<AppInfo> = self.process_manager.apps().to_vec();
        let v: Vec<(String, String, bool)> = apps
            .iter()
            .filter(|a| {
                a.binary != "slowterm"
                    && self.process_manager.binary_exists(&a.binary)
                    && (a.display_name.to_lowercase().contains(query)
                        || a.description.to_lowercase().contains(query)
                        || a.binary.to_lowercase().contains(query))
            })
            .map(|a| (a.binary.clone(), a.display_name.clone(), a.running))
            .collect();
        self.search_app_matches_cache = Some((query.to_string(), epoch, v.clone()));
        v
    }

    /// Draw the spotlight search overlay
    fn draw_search(&mut self, ctx: &Context) {
        if !self.show_search {
            self.search_focus_pending = false;
            self.search_focus_request_frame = None;
            self.search_app_matches_cache = None;
            self.search_file_snapshot = None;
            return;
        }

        let query = self.search_query.to_lowercase();

        // One synchronous directory scan per search session (first non-empty query), not per key.
        if !query.is_empty() && self.search_file_snapshot.is_none() {
            self.search_file_snapshot = Some(self.build_search_file_snapshot());
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
                let should_request_focus = self.search_focus_pending
                    && self
                        .search_focus_request_frame
                        .is_some_and(|request_frame| self.frame_count.saturating_sub(request_frame) <= 4);
                if should_request_focus {
                    r.request_focus();
                }

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
                        // Search apps (terminal hidden from search — use ⌘⌥T)
                        let app_matches = self.search_app_matches_for_query(&query);

                        let file_matches: Vec<(std::path::PathBuf, String)> = self
                            .search_file_snapshot
                            .as_ref()
                            .map(|snap| Self::filter_file_snapshot_for_query(snap, &query))
                            .unwrap_or_default();

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

                // Handle Enter to launch first match (reuse results already computed above)
                if !query.is_empty() {
                    let enter_pressed = ui.input(|i| i.key_pressed(Key::Enter));
                    if enter_pressed && launch_binary.is_none() && open_file.is_none() {
                        let first_app = self
                            .search_app_matches_for_query(&query)
                            .first()
                            .map(|(b, _, _)| b.clone());
                        if let Some(binary) = first_app {
                            launch_binary = Some(binary);
                        } else if let Some(snap) = &self.search_file_snapshot {
                            let files = Self::filter_file_snapshot_for_query(snap, &query);
                            if let Some((path, _)) = files.first() {
                                open_file = Some(path.clone());
                            }
                        }
                    }
                }

                if let Some(binary) = launch_binary {
                    self.show_search = false;
                    self.search_query.clear();
                    self.search_focus_pending = false;
                    self.search_focus_request_frame = None;
                    self.launch_app_animated(&binary);
                }

                if let Some(path) = open_file {
                    self.show_search = false;
                    self.search_query.clear();
                    self.search_focus_pending = false;
                    self.search_focus_request_frame = None;
                    self.open_file_with_app(&path);
                }
            });

        // Draw dithered shadow
        if let Some(ref inner) = response {
            if self.search_focus_pending && inner.response.has_focus() {
                let latency_frames = self
                    .search_focus_request_frame
                    .and_then(|request_frame| self.frame_count.checked_sub(request_frame))
                    .unwrap_or(0);
                if let Some(telemetry) = self.input_telemetry.as_mut() {
                    telemetry.record_focus_accept(self.frame_count, latency_frames);
                }
                self.search_focus_pending = false;
                self.search_focus_request_frame = None;
            }
            slowcore::dither::draw_window_shadow(ctx, inner.response.rect);
        }

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
                            self.search_focus_pending = false;
                            self.search_focus_request_frame = None;
                        }
                    }
                }
            }
        }
    }

    fn request_search_focus(&mut self, frame_id: u64, reason: &'static str) {
        self.search_focus_pending = true;
        self.search_focus_request_frame = Some(frame_id);
        if let Some(telemetry) = self.input_telemetry.as_mut() {
            telemetry.record_focus_request(frame_id, reason);
        }
    }

    fn log_input_ingress(&mut self, frame_id: u64, ctx: &Context) {
        let telemetry = match self.input_telemetry.as_mut() {
            Some(telemetry) => telemetry,
            None => return,
        };

        let (key_down, key_up, text, mouse) = ctx.input(|i| {
            let mut key_down = 0u32;
            let mut key_up = 0u32;
            let mut text = 0u32;
            let mut mouse = 0u32;

            for event in &i.events {
                match event {
                    Event::Key { pressed: true, .. } => key_down += 1,
                    Event::Key { pressed: false, .. } => key_up += 1,
                    Event::Text(_) => text += 1,
                    _ => {}
                }
            }

            if i.pointer.primary_pressed() {
                mouse += 1;
            }
            if i.pointer.primary_released() {
                mouse += 1;
            }
            if i.pointer.any_click() {
                mouse += 1;
            }

            (key_down, key_up, text, mouse)
        });

        telemetry.record_input(frame_id, key_down, key_up, text, mouse);
    }

    fn log_logic_timing(&mut self, frame_id: u64, logic_start: Instant) {
        if let Some(telemetry) = self.input_telemetry.as_mut() {
            telemetry.record_logic(frame_id, logic_start.elapsed().as_micros());
        }
    }

    fn log_paint_timing(&mut self, frame_id: u64, paint_start: Instant) {
        if let Some(telemetry) = self.input_telemetry.as_mut() {
            telemetry.record_paint(frame_id, paint_start.elapsed().as_micros());
        }
    }

    fn flush_input_telemetry(&mut self) {
        if let Some(telemetry) = self.input_telemetry.as_mut() {
            telemetry.flush_if_needed();
        }
    }

    /// Max entries in the spotlight file index (bounds scan time / memory on huge trees).
    const SEARCH_FILE_SNAPSHOT_CAP: usize = 500;

    fn filter_file_snapshot_for_query(
        snap: &[(PathBuf, String)],
        query: &str,
    ) -> Vec<(PathBuf, String)> {
        snap.iter()
            .filter(|(_, name)| name.to_lowercase().contains(query))
            .take(12)
            .cloned()
            .collect()
    }

    /// Full index of searchable files/dirs (one synchronous scan when spotlight first sees a
    /// non-empty query for this session). Per-keystroke filtering is in-memory only.
    fn build_search_file_snapshot(&self) -> Vec<(PathBuf, String)> {
        let mut results = Vec::new();
        let home = dirs::home_dir().unwrap_or_default();

        let search_dirs = [
            home.join("Books"),
            home.join("Books").join("slowLibrary"),
            home.join("Music"),
            home.join("Documents"),
            home.join("Pictures"),
            home.join("Pictures").join("slowMuseum"),
            home.join("MIDI"),
        ];

        let extensions = [
            "epub", "txt", "rtf", "mp3", "wav", "midi", "mid", "png", "jpg", "jpeg", "gif", "bmp",
            "pdf",
        ];

        for dir in &search_dirs {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();

                    if name.starts_with('.') {
                        continue;
                    }

                    let ft = entry.file_type().ok();
                    if ft.as_ref().map(|t| t.is_dir()).unwrap_or(false) {
                        results.push((path, format!("{}/", name)));
                    } else if ft.as_ref().map(|t| t.is_file()).unwrap_or(false) {
                        let ext = path
                            .extension()
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

        results.sort_by(|a, b| {
            let a_is_dir = a.1.ends_with('/');
            let b_is_dir = b.1.ends_with('/');
            b_is_dir.cmp(&a_is_dir).then(a.1.cmp(&b.1))
        });

        results.truncate(Self::SEARCH_FILE_SNAPSHOT_CAP);
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
        let mut launch_terminal = false;
        let mut terminal_shortcut_miss: Option<(bool, bool)> = None;

        ctx.input(|i| {
            let cmd = i.modifiers.command;
            let alt = i.modifiers.alt;
            let mut saw_t_event = false;
            for event in &i.events {
                if let Event::Key {
                    key: Key::T,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } = event
                {
                    saw_t_event = true;
                    let event_cmd = cmd || modifiers.command;
                    let event_alt = alt || modifiers.alt;
                    if event_cmd && event_alt {
                        launch_terminal = true;
                    } else if event_cmd || event_alt {
                        terminal_shortcut_miss = Some((event_cmd, event_alt));
                    }
                }
            }

            if !saw_t_event && i.key_pressed(Key::T) {
                if cmd && alt {
                    launch_terminal = true;
                } else if cmd || alt {
                    terminal_shortcut_miss = Some((cmd, alt));
                }
            }

            // Cmd+Q: show shutdown dialog
            if cmd && i.key_pressed(Key::Q) {
                self.show_shutdown = true;
            }

            // Cmd+Space: toggle search
            if cmd && i.key_pressed(Key::Space) {
                let will_show = !self.show_search;
                self.show_search = will_show;
                if will_show {
                    self.search_query.clear();
                    self.search_opened_frame = self.frame_count;
                    self.request_search_focus(self.frame_count, "cmd-space");
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

        if launch_terminal {
            eprintln!("[slowdesktop] shortcut: Cmd+Opt+T -> launch terminal");
            self.launch_app_direct("slowterm");
        } else if let Some((cmd_down, alt_down)) = terminal_shortcut_miss {
            eprintln!(
                "[slowdesktop] shortcut miss: T pressed with cmd={cmd_down} alt={alt_down}, expected Cmd+Opt+T"
            );
        }

        // Handle Enter key outside of input closure
        let enter_pressed = ctx.input(|i| i.key_pressed(Key::Enter));

        if enter_pressed {
            // Open all selected folders
            let folder_indices: Vec<usize> = self.selected_folders.iter().copied().collect();
            for index in &folder_indices {
                if *index == self.desktop_folders.len() {
                    self.launch_app_animated("trash");
                } else {
                    self.open_folder(*index);
                }
            }
            // Open all selected apps
            let app_indices: Vec<usize> = self.selected_icons.iter().copied().collect();
            let apps: Vec<String> = self.process_manager.apps().iter().map(|a| a.binary.clone()).collect();
            for index in &app_indices {
                if let Some(binary) = apps.get(*index) {
                    self.launch_app_animated(binary);
                }
            }
            if !folder_indices.is_empty() || !app_indices.is_empty() {
                self.selected_icons.clear();
                self.selected_folders.clear();
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
        self.repaint.begin_frame(ctx);

        // Load icon textures on first frame
        self.load_icon_textures(ctx);
        // Consume Tab key to prevent menu focus issues
        slowcore::theme::consume_special_keys(ctx);

        let frame_id = self.frame_count.saturating_add(1);
        self.log_input_ingress(frame_id, ctx);

        // Calculate delta time
        let logic_start = Instant::now();
        let dt = logic_start.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = logic_start;

        // Update animations (launches are handled immediately)
        self.animations.update(dt);

        // Poll running child processes whenever any app is marked running.
        // Previously gated on frame_count % 30: that tied waitpid(2) cadence to egui repaint
        // rate, so exits and close animations could stall until the next unrelated input event
        // (felt like multi-second / minute-long "OS delay" on HDMI and frozen e-ink mirroring).
        self.frame_count += 1;
        let has_running = self.process_manager.apps().iter().any(|a| a.running);
        if has_running {
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

        // Continuous repaint: animations; also while an icon/folder is selected or spotlight is
        // open so egui+X11 are not left at ~4 Hz (250 ms) or input-only wake — Pi HDMI showed
        // multi‑second stale UI on selection/typing without this.
        self.repaint.set_continuous(
            self.animations.is_animating()
                || self.show_search
                || !self.selected_icons.is_empty()
                || !self.selected_folders.is_empty(),
        );

        self.handle_keys(ctx);
        self.log_logic_timing(frame_id, logic_start);

        let paint_start = Instant::now();
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

                // Build filtered app indices (cached, rebuilt only when app list changes)
                let running_count = self.process_manager.apps().iter().filter(|a| a.running).count();
                if self.cached_app_indices.is_none() || running_count != self.last_running_count {
                    let hidden_from_desktop = ["trash", "credits", "slowterm"];
                    self.cached_app_indices = Some(self.process_manager.apps()
                        .iter().enumerate()
                        .filter(|(_, a)| !hidden_from_desktop.contains(&a.binary.as_str()))
                        .map(|(i, _)| i)
                        .collect());
                    self.last_running_count = running_count;
                }
                let app_indices = self.cached_app_indices.clone().unwrap();

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
                        // If multiple icons selected, open all of them
                        if self.selected_icons.len() > 1 && self.selected_icons.contains(&index) {
                            let all_apps: Vec<String> = self.process_manager.apps().iter().map(|a| a.binary.clone()).collect();
                            let to_launch: Vec<String> = self.selected_icons.iter()
                                .filter_map(|&i| all_apps.get(i).cloned())
                                .collect();
                            self.selected_icons.clear();
                            for b in to_launch { self.launch_app_animated(&b); }
                        } else {
                            self.selected_icons.clear();
                            self.launch_app_animated(binary);
                        }
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
                        // If multiple folders selected, open all of them
                        if self.selected_folders.len() > 1 && self.selected_folders.contains(&index) {
                            let to_open: Vec<usize> = self.selected_folders.iter().copied().collect();
                            self.selected_folders.clear();
                            for i in to_open {
                                if i == self.desktop_folders.len() {
                                    self.launch_app_animated("trash");
                                } else {
                                    self.open_folder(i);
                                }
                            }
                        } else {
                            self.selected_folders.clear();
                            if index == self.desktop_folders.len() {
                                self.launch_app_animated("trash");
                            } else {
                                self.open_folder(index);
                            }
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

        self.log_paint_timing(frame_id, paint_start);
        self.flush_input_telemetry();
        self.repaint.end_frame(ctx);

        // While an X11 child app has the focus, egui may not receive pointer/keyboard events.
        // Schedule a low-rate wake so try_wait() above keeps running and the shell can repaint
        // after exits. Tunable: SLOWDESKTOP_CHILDWATCH_MS (default 100, 0 = disable).
        if self.process_manager.apps().iter().any(|a| a.running) {
            let ms: u64 = std::env::var("SLOWDESKTOP_CHILDWATCH_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100);
            if ms > 0 {
                ctx.request_repaint_after(Duration::from_millis(ms));
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.process_manager.shutdown_all();
        if let Some(telemetry) = self.input_telemetry.as_mut() {
            telemetry.flush_all();
        }
    }
}
