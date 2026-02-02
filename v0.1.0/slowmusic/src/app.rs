//! SlowMusic - minimal music player with persistent library

use egui::{Context, Key};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use serde::{Deserialize, Serialize};
use slowcore::storage::{config_dir, documents_dir, FileBrowser};
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::status_bar;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TrackInfo {
    name: String,
    path: PathBuf,
}

/// Persistent music library saved to disk
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct Library {
    tracks: Vec<TrackInfo>,
}

impl Library {
    fn config_path() -> PathBuf {
        config_dir("slowmusic").join("library.json")
    }

    fn load() -> Self {
        let path = Self::config_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}

pub struct SlowMusicApp {
    library: Library,
    current_track: Option<usize>,
    _stream: Option<OutputStream>,
    _stream_handle: Option<OutputStreamHandle>,
    sink: Option<Sink>,
    is_playing: bool,
    volume: f32,
    play_start: Option<Instant>,
    elapsed_before_pause: Duration,
    repeat_mode: RepeatMode,
    show_file_browser: bool,
    file_browser: FileBrowser,
    show_about: bool,
    error_msg: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum RepeatMode { None, All, One }

impl SlowMusicApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let (stream, handle) = OutputStream::try_default().ok().unzip();
        let library = Library::load();
        Self {
            library,
            current_track: None,
            _stream: stream,
            _stream_handle: handle,
            sink: None,
            is_playing: false,
            volume: 0.8,
            play_start: None,
            elapsed_before_pause: Duration::ZERO,
            repeat_mode: RepeatMode::None,
            show_file_browser: false,
            file_browser: FileBrowser::new(documents_dir())
                .with_filter(vec!["mp3".into(), "wav".into(), "flac".into(), "ogg".into()]),
            show_about: false,
            error_msg: None,
        }
    }

    fn add_file(&mut self, path: PathBuf) {
        // Don't add duplicates
        if self.library.tracks.iter().any(|t| t.path == path) { return; }
        let name = path.file_stem()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".into());
        self.library.tracks.push(TrackInfo { name, path });
        self.library.save();
    }

    fn remove_track(&mut self, index: usize) {
        if index < self.library.tracks.len() {
            // If removing current track, stop playback
            if self.current_track == Some(index) {
                self.stop();
            } else if let Some(ct) = self.current_track {
                if ct > index { self.current_track = Some(ct - 1); }
            }
            self.library.tracks.remove(index);
            self.library.save();
        }
    }

    fn play_track(&mut self, index: usize) {
        if index >= self.library.tracks.len() { return; }
        if let Some(ref sink) = self.sink { sink.stop(); }

        let path = &self.library.tracks[index].path;
        
        // Check file still exists
        if !path.exists() {
            self.error_msg = Some(format!("file not found: {}", path.display()));
            return;
        }

        match File::open(path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                match Decoder::new(reader) {
                    Ok(source) => {
                        if let Some(ref handle) = self._stream_handle {
                            match Sink::try_new(handle) {
                                Ok(sink) => {
                                    sink.set_volume(self.volume);
                                    sink.append(source);
                                    self.sink = Some(sink);
                                    self.current_track = Some(index);
                                    self.is_playing = true;
                                    self.play_start = Some(Instant::now());
                                    self.elapsed_before_pause = Duration::ZERO;
                                    self.error_msg = None;
                                }
                                Err(e) => self.error_msg = Some(format!("audio error: {}", e)),
                            }
                        }
                    }
                    Err(e) => self.error_msg = Some(format!("decode error: {}", e)),
                }
            }
            Err(e) => self.error_msg = Some(format!("file error: {}", e)),
        }
    }

    fn toggle_play(&mut self) {
        if let Some(ref sink) = self.sink {
            if sink.is_paused() {
                sink.play();
                self.is_playing = true;
                self.play_start = Some(Instant::now());
            } else {
                sink.pause();
                self.is_playing = false;
                if let Some(start) = self.play_start {
                    self.elapsed_before_pause += start.elapsed();
                }
                self.play_start = None;
            }
        } else if !self.library.tracks.is_empty() {
            self.play_track(self.current_track.unwrap_or(0));
        }
    }

    fn stop(&mut self) {
        if let Some(ref sink) = self.sink { sink.stop(); }
        self.sink = None;
        self.is_playing = false;
        self.play_start = None;
        self.elapsed_before_pause = Duration::ZERO;
    }

    fn next_track(&mut self) {
        if self.library.tracks.is_empty() { return; }
        let next = match self.current_track {
            Some(i) => {
                if i + 1 < self.library.tracks.len() { i + 1 }
                else if self.repeat_mode == RepeatMode::All { 0 }
                else { return; }
            }
            None => 0,
        };
        self.play_track(next);
    }

    fn prev_track(&mut self) {
        if self.library.tracks.is_empty() { return; }
        let prev = match self.current_track {
            Some(i) if i > 0 => i - 1,
            _ => if self.repeat_mode == RepeatMode::All { self.library.tracks.len() - 1 } else { 0 },
        };
        self.play_track(prev);
    }

    fn check_track_end(&mut self) {
        if let Some(ref sink) = self.sink {
            if sink.empty() && self.is_playing {
                match self.repeat_mode {
                    RepeatMode::One => { if let Some(idx) = self.current_track { self.play_track(idx); } }
                    _ => self.next_track(),
                }
            }
        }
    }

    fn elapsed(&self) -> Duration {
        let current = self.play_start.map(|s| s.elapsed()).unwrap_or_default();
        self.elapsed_before_pause + current
    }

    fn handle_keys(&mut self, ctx: &Context) {
        // Consume Tab to prevent menu hover
        ctx.input_mut(|i| {
            if i.key_pressed(Key::Tab) {
                i.events.retain(|e| !matches!(e, egui::Event::Key { key: Key::Tab, .. }));
            }
        });
        ctx.input(|i| {
            if i.key_pressed(Key::Space) { self.toggle_play(); }
            if i.key_pressed(Key::N) || i.key_pressed(Key::ArrowRight) { self.next_track(); }
            if i.key_pressed(Key::P) || i.key_pressed(Key::ArrowLeft) { self.prev_track(); }
            if i.modifiers.command && i.key_pressed(Key::O) { self.show_file_browser = true; }
        });
    }

    fn render_controls(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            let track_name = self.current_track
                .and_then(|i| self.library.tracks.get(i))
                .map(|t| t.name.clone())
                .unwrap_or_else(|| "no track".into());
            ui.heading(&track_name);

            let elapsed = self.elapsed();
            ui.label(format!("{}:{:02}", elapsed.as_secs() / 60, elapsed.as_secs() % 60));
            ui.add_space(10.0);

            // Transport
            ui.horizontal(|ui| {
                ui.add_space(ui.available_width() / 2.0 - 100.0);
                if ui.button("prev").on_hover_text("previous track").clicked() { self.prev_track(); }
                let play_label = if self.is_playing { "pause" } else { "play" };
                if ui.button(egui::RichText::new(play_label).size(18.0)).clicked() { self.toggle_play(); }
                if ui.button("stop").clicked() { self.stop(); }
                if ui.button("next").clicked() { self.next_track(); }
            });

            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.label("vol:");
                // Custom volume bar for visibility on e-ink
                let desired = egui::vec2(200.0, 20.0);
                let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());
                if ui.is_rect_visible(rect) {
                    let painter = ui.painter();
                    // Track: white fill, 1px black outline
                    painter.rect_filled(rect, 0.0, SlowColors::WHITE);
                    painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.0, SlowColors::BLACK));
                    // Filled portion: solid black
                    let fill_w = rect.width() * self.volume;
                    let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fill_w, rect.height()));
                    painter.rect_filled(fill_rect, 0.0, SlowColors::BLACK);
                    // Volume text centered
                    let pct = format!("{}%", (self.volume * 100.0) as i32);
                    let text_color = if self.volume > 0.5 { SlowColors::WHITE } else { SlowColors::BLACK };
                    painter.text(rect.center(), egui::Align2::CENTER_CENTER, &pct, egui::FontId::proportional(12.0), text_color);
                }
                // Handle click/drag to set volume
                if response.clicked() || response.dragged() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let rel = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                        self.volume = rel;
                        if let Some(ref sink) = self.sink { sink.set_volume(self.volume); }
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("repeat:");
                if ui.selectable_label(self.repeat_mode == RepeatMode::None, "off").clicked() { self.repeat_mode = RepeatMode::None; }
                if ui.selectable_label(self.repeat_mode == RepeatMode::All, "all").clicked() { self.repeat_mode = RepeatMode::All; }
                if ui.selectable_label(self.repeat_mode == RepeatMode::One, "one").clicked() { self.repeat_mode = RepeatMode::One; }
            });
        });
    }

    fn render_library(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("music").strong());
            if ui.button("add music").clicked() { self.show_file_browser = true; }
            if ui.button("clear all").clicked() { self.library.tracks.clear(); self.library.save(); self.stop(); self.current_track = None; }
        });
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut play_idx = None;
            let mut remove_idx = None;
            let count = self.library.tracks.len();
            for idx in 0..count {
                let track = &self.library.tracks[idx];
                let current = self.current_track == Some(idx);
                let prefix = if current && self.is_playing { "> " } else if current { "| " } else { "  " };
                let label = format!("{}{}", prefix, track.name);

                ui.horizontal(|ui| {
                    let r = ui.selectable_label(current, &label);
                    if r.double_clicked() { play_idx = Some(idx); }
                    // Small remove button
                    if ui.small_button("x").on_hover_text("remove from library").clicked() {
                        remove_idx = Some(idx);
                    }
                });
            }
            if let Some(idx) = play_idx { self.play_track(idx); }
            if let Some(idx) = remove_idx { self.remove_track(idx); }
        });
    }

    fn render_file_browser(&mut self, ctx: &Context) {
        egui::Window::new("add music").collapsible(false).resizable(false).default_width(400.0)
            .show(ctx, |ui| {
                ui.label(self.file_browser.current_dir.to_string_lossy().to_string());
                ui.separator();
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    let entries = self.file_browser.entries.clone();
                    for (idx, entry) in entries.iter().enumerate() {
                        let sel = self.file_browser.selected_index == Some(idx);
                        let r = ui.add(slowcore::widgets::FileListItem::new(&entry.name, entry.is_directory).selected(sel));
                        if r.clicked() { self.file_browser.selected_index = Some(idx); }
                        if r.double_clicked() {
                            if entry.is_directory { self.file_browser.navigate_to(entry.path.clone()); }
                            else { self.add_file(entry.path.clone()); self.show_file_browser = false; }
                        }
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("cancel").clicked() { self.show_file_browser = false; }
                    if ui.button("add selected").clicked() {
                        if let Some(e) = self.file_browser.selected_entry() {
                            if !e.is_directory { let p = e.path.clone(); self.add_file(p); self.show_file_browser = false; }
                        }
                    }
                    if ui.button("add all").clicked() {
                        let files: Vec<PathBuf> = self.file_browser.entries.iter()
                            .filter(|e| !e.is_directory).map(|e| e.path.clone()).collect();
                        for f in files { self.add_file(f); }
                        self.show_file_browser = false;
                    }
                });
            });
    }
}

impl eframe::App for SlowMusicApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Handle drag and drop of audio files
        let dropped_files: Vec<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter()
                .filter_map(|file| file.path.clone())
                .filter(|path| {
                    let ext = path.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase())
                        .unwrap_or_default();
                    matches!(ext.as_str(), "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac")
                })
                .collect()
        });

        // Add dropped files to library
        for path in dropped_files {
            self.add_file(path);
        }

        self.handle_keys(ctx);
        self.check_track_end();
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            menu_bar(ui, |ui| {
                ui.menu_button("file", |ui| {
                    if ui.button("add music...  ⌘o").clicked() { self.show_file_browser = true; ui.close_menu(); }
                });
                ui.menu_button("help", |ui| {
                    if ui.button("about slowMusic").clicked() { self.show_about = true; ui.close_menu(); }
                });
            });
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let err = self.error_msg.as_deref().unwrap_or("");
            status_bar(ui, &format!("{} tracks  |  volume: {}%  {}", self.library.tracks.len(), (self.volume * 100.0) as i32, err));
        });
        egui::TopBottomPanel::top("controls").min_height(140.0).show(ctx, |ui| self.render_controls(ui));
        egui::CentralPanel::default().frame(
            egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(8.0))
        ).show(ctx, |ui| self.render_library(ui));

        if self.show_file_browser { self.render_file_browser(ctx); }
        if self.show_about {
            egui::Window::new("about slowMusic")
                .collapsible(false)
                .resizable(false)
                .default_width(300.0)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("slowMusic");
                        ui.label("version 0.1.0");
                        ui.add_space(8.0);
                        ui.label("music player for slowOS");
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label("supported formats:");
                    ui.label("  MP3, WAV, FLAC, OGG, AAC");
                    ui.add_space(4.0);
                    ui.label("features:");
                    ui.label("  library management, playlists");
                    ui.label("  persistent playback state");
                    ui.add_space(4.0);
                    ui.label("frameworks:");
                    ui.label("  egui/eframe (MIT), rodio (MIT)");
                    ui.label("  symphonia (MPL-2.0)");
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        if ui.button("ok").clicked() { self.show_about = false; }
                    });
                });
        }
    }
}
