//! SlowMusic - minimal music player with persistent library

use egui::{ColorImage, Context, Key, TextureHandle, TextureOptions};
use id3::TagLike;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use serde::{Deserialize, Serialize};
use slowcore::storage::{config_dir, documents_dir, FileBrowser};
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::status_bar;
use std::io::Cursor;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Metadata extracted from an audio file's ID3 tags
struct TrackMeta {
    artist: Option<String>,
    album: Option<String>,
    year: Option<String>,
    title: Option<String>,
}

impl Default for TrackMeta {
    fn default() -> Self {
        Self { artist: None, album: None, year: None, title: None }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TrackInfo {
    name: String,
    path: PathBuf,
    #[serde(default)]
    album: Option<String>,
    #[serde(default)]
    artist: Option<String>,
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
    track_duration: Option<Duration>,
    repeat_mode: RepeatMode,
    show_file_browser: bool,
    file_browser: FileBrowser,
    show_about: bool,
    error_msg: Option<String>,
    /// Metadata for the currently playing track
    current_meta: TrackMeta,
    /// Album art texture (dithered B&W)
    art_texture: Option<TextureHandle>,
    /// Path for which metadata was loaded (avoid reloading)
    meta_loaded_for: Option<PathBuf>,
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
            track_duration: None,
            repeat_mode: RepeatMode::None,
            show_file_browser: false,
            file_browser: FileBrowser::new(documents_dir())
                .with_filter(vec!["mp3".into(), "wav".into(), "flac".into(), "ogg".into(), "m4a".into(), "aac".into()]),
            show_about: false,
            error_msg: None,
            current_meta: TrackMeta::default(),
            art_texture: None,
            meta_loaded_for: None,
        }
    }

    /// Load ID3 metadata and album art for the given track path
    fn load_metadata(&mut self, ctx: &Context, path: &PathBuf) {
        if self.meta_loaded_for.as_ref() == Some(path) {
            return;
        }
        self.meta_loaded_for = Some(path.clone());
        self.current_meta = TrackMeta::default();
        self.art_texture = None;

        if let Ok(tag) = id3::Tag::read_from_path(path) {
            self.current_meta.title = tag.title().map(|s| s.to_string());
            self.current_meta.artist = tag.artist().map(|s| s.to_string());
            self.current_meta.album = tag.album().map(|s| s.to_string());
            self.current_meta.year = tag.year().map(|y| y.to_string())
                .or_else(|| tag.date_released().map(|d| d.year.to_string()));

            // Extract album art (first picture)
            if let Some(pic) = tag.pictures().next() {
                if let Ok(img) = image::load_from_memory(&pic.data) {
                    // Resize to fit display and convert to greyscale
                    let resized = img.resize(140, 140, image::imageops::FilterType::Triangle);
                    let grey = resized.grayscale();
                    let rgba = grey.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    let color_image = ColorImage::from_rgba_unmultiplied(
                        [w as usize, h as usize],
                        rgba.as_raw(),
                    );
                    let texture = ctx.load_texture(
                        "album_art",
                        color_image,
                        TextureOptions::NEAREST,
                    );
                    self.art_texture = Some(texture);
                }
            }
        }
    }

    pub fn add_file(&mut self, path: PathBuf) {
        // Don't add duplicates
        if self.library.tracks.iter().any(|t| t.path == path) { return; }
        let name = path.file_stem()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".into());
        // Read album/artist metadata from ID3 tags
        let (album, artist) = id3::Tag::read_from_path(&path)
            .map(|tag| {
                (tag.album().map(|s| s.to_string()), tag.artist().map(|s| s.to_string()))
            })
            .unwrap_or((None, None));
        self.library.tracks.push(TrackInfo { name, path, album, artist });
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

    pub fn play_track(&mut self, index: usize) {
        if index >= self.library.tracks.len() { return; }
        if let Some(ref sink) = self.sink { sink.stop(); }

        let path = &self.library.tracks[index].path;

        // Check file still exists
        if !path.exists() {
            self.error_msg = Some(format!("file not found: {}", path.display()));
            return;
        }

        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => { self.error_msg = Some(format!("file error: {}", e)); return; }
        };

        // Try rodio's Decoder first (works for wav, mp3, flac, ogg)
        let rodio_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Decoder::new(Cursor::new(data.clone()))
        }));

        match rodio_result {
            Ok(Ok(source)) => {
                self.start_playback(source.convert_samples::<f32>(), index);
                return;
            }
            _ => {} // Fall through to symphonia direct decoding
        }

        // Fallback: decode with symphonia directly (for m4a/aac that rodio can't handle)
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match decode_with_symphonia(data, ext) {
            Ok(source) => {
                self.start_playback(source, index);
            }
            Err(e) => {
                self.error_msg = Some(format!("decode error: {}", e));
            }
        }
    }

    fn start_playback<S: Source<Item = f32> + Send + 'static>(&mut self, source: S, index: usize) {
        self.track_duration = source.total_duration();
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
        self.track_duration = None;
        self.current_meta = TrackMeta::default();
        self.art_texture = None;
        self.meta_loaded_for = None;
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
        slowcore::theme::consume_special_keys(ctx);
        ctx.input(|i| {
            if i.key_pressed(Key::Space) { self.toggle_play(); }
            if i.key_pressed(Key::N) || i.key_pressed(Key::ArrowRight) { self.next_track(); }
            if i.key_pressed(Key::P) || i.key_pressed(Key::ArrowLeft) { self.prev_track(); }
            if i.modifiers.command && i.key_pressed(Key::O) { self.show_file_browser = true; }
        });
    }

    fn render_controls(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            // Show album art and metadata side by side if we have art
            let has_art = self.art_texture.is_some();
            let track_name = self.current_track
                .and_then(|i| self.library.tracks.get(i))
                .map(|t| t.name.clone())
                .unwrap_or_else(|| "no track".into());

            if has_art {
                ui.horizontal(|ui| {
                    // Album art
                    if let Some(ref tex) = self.art_texture {
                        let size = tex.size_vec2();
                        let max_side = 100.0;
                        let scale = (max_side / size.x.max(size.y)).min(1.0);
                        let display_size = egui::vec2(size.x * scale, size.y * scale);
                        ui.image(egui::load::SizedTexture::new(tex.id(), display_size));
                    }
                    // Metadata text
                    ui.vertical(|ui| {
                        let title = self.current_meta.title.as_deref()
                            .unwrap_or(&track_name);
                        ui.label(egui::RichText::new(title).strong().size(14.0));
                        if let Some(ref artist) = self.current_meta.artist {
                            ui.label(artist.as_str());
                        }
                        if let Some(ref album) = self.current_meta.album {
                            ui.label(egui::RichText::new(album.as_str()).italics());
                        }
                        if let Some(ref year) = self.current_meta.year {
                            ui.label(year.as_str());
                        }
                    });
                });
            } else {
                // No art: show title and metadata stacked
                let title = self.current_meta.title.as_deref()
                    .unwrap_or(&track_name);
                ui.heading(title);
                let mut meta_parts: Vec<&str> = Vec::new();
                if let Some(ref a) = self.current_meta.artist { meta_parts.push(a); }
                if let Some(ref a) = self.current_meta.album { meta_parts.push(a); }
                if let Some(ref y) = self.current_meta.year { meta_parts.push(y); }
                if !meta_parts.is_empty() {
                    ui.label(meta_parts.join("  ·  "));
                }
            }

            let elapsed = self.elapsed();
            let elapsed_secs = elapsed.as_secs();
            let elapsed_str = format!("{}:{:02}", elapsed_secs / 60, elapsed_secs % 60);

            // Position scrubber
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.label(&elapsed_str);

                // Scrubber bar (shows elapsed progress, click to seek)
                let desired = egui::vec2(200.0, 16.0);
                let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());

                // Get track duration in seconds (fallback to 3 minutes if unknown)
                let duration_secs = self.track_duration
                    .map(|d| d.as_secs_f32())
                    .unwrap_or(180.0)
                    .max(1.0); // Avoid division by zero

                if ui.is_rect_visible(rect) {
                    let painter = ui.painter();
                    // Track: white fill, 1px black outline
                    painter.rect_filled(rect, 0.0, SlowColors::WHITE);
                    painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.0, SlowColors::BLACK));

                    // Calculate fill based on elapsed time vs actual track duration
                    let progress = (elapsed_secs as f32 / duration_secs).min(1.0);
                    let fill_w = rect.width() * progress;
                    let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fill_w, rect.height()));
                    painter.rect_filled(fill_rect, 0.0, SlowColors::BLACK);

                    // Position marker (small vertical line)
                    let marker_x = rect.min.x + fill_w;
                    if marker_x < rect.max.x {
                        painter.vline(marker_x, rect.y_range(), egui::Stroke::new(2.0, SlowColors::BLACK));
                    }
                }

                // Show duration
                let duration_display = self.track_duration
                    .map(|d| format!("{}:{:02}", d.as_secs() / 60, d.as_secs() % 60))
                    .unwrap_or_else(|| "--:--".to_string());
                ui.label(&duration_display);

                // Handle click to seek
                if response.clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let rel = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                        let seek_secs = (rel * duration_secs) as u64;
                        if let Some(ref sink) = self.sink {
                            let _ = sink.try_seek(Duration::from_secs(seek_secs));
                            self.elapsed_before_pause = Duration::from_secs(seek_secs);
                            if self.is_playing {
                                self.play_start = Some(Instant::now());
                            }
                        }
                    }
                }
            });
            ui.add_space(5.0);

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
            if self.library.tracks.is_empty() {
                ui.add_space(40.0);
                ui.vertical_centered(|ui| {
                    ui.label("want to grow your music collection?");
                    ui.add_space(4.0);
                    ui.label("mp3 and wave files can be bought at bandcamp.com");
                });
                return;
            }

            // Group tracks: albums first, then ungrouped
            let mut albums: Vec<(String, Vec<usize>)> = Vec::new();
            let mut ungrouped: Vec<usize> = Vec::new();

            for idx in 0..self.library.tracks.len() {
                if let Some(ref album) = self.library.tracks[idx].album {
                    if let Some(entry) = albums.iter_mut().find(|(a, _)| a == album) {
                        entry.1.push(idx);
                    } else {
                        albums.push((album.clone(), vec![idx]));
                    }
                } else {
                    ungrouped.push(idx);
                }
            }

            let mut play_idx = None;
            let mut remove_idx = None;

            // Render album groups
            for (album_name, track_indices) in &albums {
                let artist_label = track_indices.first()
                    .and_then(|&i| self.library.tracks[i].artist.as_deref())
                    .unwrap_or("");
                let header = if artist_label.is_empty() {
                    album_name.clone()
                } else {
                    format!("{} — {}", album_name, artist_label)
                };
                egui::CollapsingHeader::new(&header)
                    .default_open(true)
                    .show(ui, |ui| {
                        for &idx in track_indices {
                            let track = &self.library.tracks[idx];
                            let current = self.current_track == Some(idx);
                            let prefix = if current && self.is_playing { "> " } else if current { "| " } else { "  " };
                            let label = format!("{}{}", prefix, track.name);
                            ui.horizontal(|ui| {
                                let r = ui.selectable_label(current, &label);
                                if r.double_clicked() { play_idx = Some(idx); }
                                if ui.small_button("x").on_hover_text("remove from library").clicked() {
                                    remove_idx = Some(idx);
                                }
                            });
                        }
                    });
            }

            // Render ungrouped tracks
            if !ungrouped.is_empty() && !albums.is_empty() {
                ui.separator();
            }
            for idx in &ungrouped {
                let track = &self.library.tracks[*idx];
                let current = self.current_track == Some(*idx);
                let prefix = if current && self.is_playing { "> " } else if current { "| " } else { "  " };
                let label = format!("{}{}", prefix, track.name);
                ui.horizontal(|ui| {
                    let r = ui.selectable_label(current, &label);
                    if r.double_clicked() { play_idx = Some(*idx); }
                    if ui.small_button("x").on_hover_text("remove from library").clicked() {
                        remove_idx = Some(*idx);
                    }
                });
            }

            if let Some(idx) = play_idx { self.play_track(idx); }
            if let Some(idx) = remove_idx { self.remove_track(idx); }
        });
    }

    fn render_file_browser(&mut self, ctx: &Context) {
        let resp = egui::Window::new("add music").collapsible(false).resizable(false).default_width(380.0)
            .show(ctx, |ui| {
                ui.label(self.file_browser.current_dir.to_string_lossy().to_string());
                ui.separator();
                egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
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
        if let Some(r) = &resp { slowcore::dither::draw_window_shadow(ctx, r.response.rect); }
    }
}

impl eframe::App for SlowMusicApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Handle drag and drop of audio files and folders
        let dropped_paths: Vec<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter()
                .filter_map(|file| file.path.clone())
                .collect()
        });

        // Collect all audio files (recursively scanning directories)
        if !dropped_paths.is_empty() {
            let mut audio_files: Vec<PathBuf> = Vec::new();
            for path in dropped_paths {
                if path.is_dir() {
                    collect_audio_files_recursive(&path, &mut audio_files);
                } else if is_audio_file(&path) {
                    audio_files.push(path);
                }
            }
            // Sort by path for consistent ordering
            audio_files.sort();
            for path in audio_files {
                self.add_file(path);
            }
        }

        self.handle_keys(ctx);
        self.check_track_end();

        // Load metadata for current track (lazy, once per track change)
        if let Some(idx) = self.current_track {
            if let Some(track) = self.library.tracks.get(idx) {
                let path = track.path.clone();
                self.load_metadata(ctx, &path);
            }
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            menu_bar(ui, |ui| {
                ui.menu_button("file", |ui| {
                    if ui.button("add music...  ⌘o").clicked() { self.show_file_browser = true; ui.close_menu(); }
                });
                ui.menu_button("help", |ui| {
                    if ui.button("about").clicked() { self.show_about = true; ui.close_menu(); }
                });
            });
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let err = self.error_msg.as_deref().unwrap_or("");
            status_bar(ui, &format!("{} tracks  |  volume: {}%  {}", self.library.tracks.len(), (self.volume * 100.0) as i32, err));
        });
        let controls_height = if self.art_texture.is_some() { 200.0 } else { 140.0 };
        egui::TopBottomPanel::top("controls").min_height(controls_height).show(ctx, |ui| self.render_controls(ui));
        egui::CentralPanel::default().frame(
            egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(8.0))
        ).show(ctx, |ui| self.render_library(ui));

        if self.show_file_browser { self.render_file_browser(ctx); }
        if self.show_about {
            let resp = egui::Window::new("about slowMusic")
                .collapsible(false)
                .resizable(false)
                .default_width(300.0)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("slowMusic");
                        ui.label("version 0.2.0");
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
            if let Some(r) = &resp { slowcore::dither::draw_window_shadow(ctx, r.response.rect); }
        }
    }
}

/// A rodio Source backed by pre-decoded f32 samples
struct SamplesSource {
    samples: Vec<f32>,
    pos: usize,
    sample_rate: u32,
    channels: u16,
}

impl Iterator for SamplesSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.pos < self.samples.len() {
            let s = self.samples[self.pos];
            self.pos += 1;
            Some(s)
        } else {
            None
        }
    }
}

impl Source for SamplesSource {
    fn current_frame_len(&self) -> Option<usize> { Some(self.samples.len() - self.pos) }
    fn channels(&self) -> u16 { self.channels }
    fn sample_rate(&self) -> u32 { self.sample_rate }
    fn total_duration(&self) -> Option<Duration> {
        let total_frames = self.samples.len() as f64 / self.channels as f64;
        Some(Duration::from_secs_f64(total_frames / self.sample_rate as f64))
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        let sample_pos = (pos.as_secs_f64() * self.sample_rate as f64 * self.channels as f64) as usize;
        self.pos = sample_pos.min(self.samples.len());
        Ok(())
    }
}

/// Decode audio using symphonia directly, bypassing rodio's problematic seek-on-init
fn decode_with_symphonia(data: Vec<u8>, ext: &str) -> Result<SamplesSource, String> {
    let cursor = Cursor::new(data);
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let mut hint = Hint::new();
    if !ext.is_empty() { hint.with_extension(ext); }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("probe: {}", e))?;

    let mut format = probed.format;
    let track = format.default_track().ok_or("no audio track found")?;
    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track.codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("codec: {}", e))?;

    let mut samples: Vec<f32> = Vec::new();

    loop {
        match format.next_packet() {
            Ok(packet) => {
                if packet.track_id() != track_id { continue; }
                match decoder.decode(&packet) {
                    Ok(decoded) => {
                        let spec = *decoded.spec();
                        let duration = decoded.capacity() as u64;
                        let mut buf = SampleBuffer::<f32>::new(duration, spec);
                        buf.copy_interleaved_ref(decoded);
                        samples.extend_from_slice(buf.samples());
                    }
                    Err(_) => continue,
                }
            }
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(_) => break,
        }
    }

    if samples.is_empty() {
        return Err("no audio data decoded".into());
    }

    Ok(SamplesSource { samples, pos: 0, sample_rate, channels })
}

fn is_audio_file(path: &std::path::Path) -> bool {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    matches!(ext.as_str(), "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac")
}

fn collect_audio_files_recursive(dir: &std::path::Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_audio_files_recursive(&path, files);
        } else if is_audio_file(&path) {
            files.push(path);
        }
    }
}
