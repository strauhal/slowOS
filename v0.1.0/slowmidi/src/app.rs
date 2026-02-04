//! slowMidi — MIDI notation application with piano roll and notation views

use egui::{Context, Key, Pos2, Rect, Sense, Stroke, Vec2};
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use serde::{Deserialize, Serialize};
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::{status_bar, FileListItem};
use slowcore::storage::{FileBrowser, documents_dir};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use std::collections::HashSet;

// ---------------------------------------------------------------
// Constants
// ---------------------------------------------------------------

const PIANO_KEYS: u8 = 88;
const KEY_HEIGHT: f32 = 12.0;
const BEAT_WIDTH: f32 = 80.0;
const PIANO_WIDTH: f32 = 60.0;
const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// Scale types for quantization: (name, semitone intervals from root)
const SCALE_TYPES: &[(&str, &[u8])] = &[
    ("chromatic",  &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
    ("major",      &[0, 2, 4, 5, 7, 9, 11]),
    ("minor",      &[0, 2, 3, 5, 7, 8, 10]),
    ("lydian",     &[0, 2, 4, 6, 7, 9, 11]),
    ("mixolydian", &[0, 2, 4, 5, 7, 9, 10]),
    ("dorian",     &[0, 2, 3, 5, 7, 9, 10]),
    ("phrygian",   &[0, 1, 3, 5, 7, 8, 10]),
    ("blues",      &[0, 3, 5, 6, 7, 10]),
    ("pentatonic", &[0, 2, 4, 7, 9]),
    ("japanese",   &[0, 1, 5, 7, 8]),
    ("whole tone", &[0, 2, 4, 6, 8, 10]),
    ("harmonic minor", &[0, 2, 3, 5, 7, 8, 11]),
];

const SCALE_ROOT_NAMES: &[&str] = &["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// Quantize a MIDI pitch to the nearest note in the given scale.
fn quantize_to_scale(pitch: u8, root: u8, scale_intervals: &[u8]) -> u8 {
    if scale_intervals.len() >= 12 {
        return pitch; // chromatic, no quantize
    }
    let note = (pitch as i32 - root as i32).rem_euclid(12) as u8;
    // Find nearest interval in scale
    let mut best = scale_intervals[0];
    let mut best_dist = 12u8;
    for &interval in scale_intervals {
        let dist_up = (note as i32 - interval as i32).rem_euclid(12) as u8;
        let dist_down = (interval as i32 - note as i32).rem_euclid(12) as u8;
        let dist = dist_up.min(dist_down);
        if dist < best_dist {
            best_dist = dist;
            best = interval;
        }
    }
    let quantized_note = (pitch as i32 - (note as i32 - best as i32).rem_euclid(12) as i32) as u8;
    quantized_note.clamp(21, 108)
}

/// Map chromatic semitone (0-11) to diatonic step (0-6): C=0, D=1, E=2, F=3, G=4, A=5, B=6
const SEMITONE_TO_DIATONIC: [i32; 12] = [0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6];

/// Convert MIDI pitch to a diatonic staff position (integer).
/// Each position is one line or space on the staff.
/// Returns position relative to a global diatonic origin.
fn midi_to_diatonic(pitch: u8) -> i32 {
    let p = pitch as i32;
    let octave = p / 12;
    let semitone = (p % 12) as usize;
    octave * 7 + SEMITONE_TO_DIATONIC[semitone]
}

/// Reference diatonic positions for staff bottom lines
const TREBLE_BOTTOM_DIATONIC: i32 = 37; // E4 (MIDI 64): octave 5 * 7 + 2 = 37
const BASS_BOTTOM_DIATONIC: i32 = 25;   // G2 (MIDI 43): octave 3 * 7 + 4 = 25

/// Convert MIDI pitch to Y position on notation staff.
/// `base_y` is the top of the staff (where top line is drawn).
/// `staff_spacing` is pixels between staff lines.
fn pitch_to_staff_y(pitch: u8, is_treble: bool, base_y: f32, staff_spacing: f32) -> f32 {
    let diatonic = midi_to_diatonic(pitch);
    let ref_diatonic = if is_treble { TREBLE_BOTTOM_DIATONIC } else { BASS_BOTTOM_DIATONIC };
    let steps_above_bottom = diatonic - ref_diatonic;
    // bottom line is at base_y + 4 * staff_spacing
    // each diatonic step moves up by half a staff_spacing
    base_y + 4.0 * staff_spacing - steps_above_bottom as f32 * (staff_spacing / 2.0)
}

/// Reverse: convert Y position on staff to MIDI pitch.
/// Maps to the nearest diatonic pitch (white keys only).
const DIATONIC_TO_SEMITONE: [i32; 7] = [0, 2, 4, 5, 7, 9, 11]; // C, D, E, F, G, A, B

fn staff_y_to_pitch(y: f32, is_treble: bool, base_y: f32, staff_spacing: f32) -> u8 {
    let bottom_y = base_y + 4.0 * staff_spacing;
    let steps_above_bottom = ((bottom_y - y) / (staff_spacing / 2.0)).round() as i32;
    let ref_diatonic = if is_treble { TREBLE_BOTTOM_DIATONIC } else { BASS_BOTTOM_DIATONIC };
    let diatonic = ref_diatonic + steps_above_bottom;
    let octave = diatonic.div_euclid(7);
    let step = diatonic.rem_euclid(7) as usize;
    (octave * 12 + DIATONIC_TO_SEMITONE[step]).clamp(21, 108) as u8
}

// ---------------------------------------------------------------
// Simple sine wave audio source
// ---------------------------------------------------------------

/// A sine wave audio source for a single note
struct SineWave {
    freq: f32,
    sample_rate: u32,
    num_samples: usize,
    current_sample: usize,
}

impl SineWave {
    fn new(freq: f32, duration_ms: u32) -> Self {
        let sample_rate = 44100;
        let num_samples = (sample_rate * duration_ms / 1000) as usize;
        Self {
            freq,
            sample_rate,
            num_samples,
            current_sample: 0,
        }
    }
}

impl Source for SineWave {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_millis((self.num_samples as u64 * 1000) / self.sample_rate as u64))
    }
}

impl Iterator for SineWave {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_sample >= self.num_samples {
            return None;
        }

        let t = self.current_sample as f32 / self.sample_rate as f32;
        self.current_sample += 1;

        // Simple envelope: attack/decay to avoid clicks
        let envelope = if self.current_sample < 500 {
            self.current_sample as f32 / 500.0
        } else if self.current_sample > self.num_samples - 500 {
            (self.num_samples - self.current_sample) as f32 / 500.0
        } else {
            1.0
        };

        // Apply limiter: sine * envelope * master volume, then soft clip
        let sample = (t * self.freq * 2.0 * std::f32::consts::PI).sin() * 0.25 * envelope;
        // Soft limiter to prevent clipping and protect speakers
        Some(sample.tanh())
    }
}

/// Convert MIDI note number to frequency
fn midi_to_freq(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}

// ---------------------------------------------------------------
// MIDI Note representation
// ---------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MidiNote {
    /// MIDI note number (0-127, middle C = 60)
    pub pitch: u8,
    /// Start time in beats (quarter notes)
    pub start: f32,
    /// Duration in beats
    pub duration: f32,
    /// Velocity (0-127)
    pub velocity: u8,
}

impl MidiNote {
    fn new(pitch: u8, start: f32, duration: f32) -> Self {
        Self {
            pitch,
            start,
            duration,
            velocity: 100,
        }
    }
}

// ---------------------------------------------------------------
// Project (song) data
// ---------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MidiProject {
    pub name: String,
    pub tempo: u32, // BPM
    pub time_signature_num: u8,
    pub time_signature_den: u8,
    pub notes: Vec<MidiNote>,
}

impl Default for MidiProject {
    fn default() -> Self {
        Self {
            name: "untitled".into(),
            tempo: 120,
            time_signature_num: 4,
            time_signature_den: 4,
            notes: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------
// View modes
// ---------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    PianoRoll,
    Notation,
}

// ---------------------------------------------------------------
// Tool modes for editing
// ---------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EditTool {
    Select,
    Draw,
    Paint, // Paintbrush - hold and drag to create notes continuously
    Erase,
}

// ---------------------------------------------------------------
// Application state
// ---------------------------------------------------------------

pub struct SlowMidiApp {
    project: MidiProject,
    file_path: Option<PathBuf>,
    modified: bool,

    // Undo/Redo
    undo_stack: Vec<Vec<MidiNote>>,
    redo_stack: Vec<Vec<MidiNote>>,

    // View state
    view_mode: ViewMode,
    scroll_x: f32,
    scroll_y: f32,
    zoom: f32,

    // Editing
    edit_tool: EditTool,
    selected_notes: Vec<usize>,
    note_duration: f32, // Default duration for new notes (in beats)
    grid_division: f32, // Grid line division (1.0 = quarter, 0.5 = eighth, etc.)
    scale_root: u8,     // Scale root note (0=C, 1=C#, ..., 11=B)
    scale_type: usize,  // Index into SCALE_TYPES

    // Paint tool state
    is_painting: bool,
    last_paint_beat: f32,
    last_paint_pitch: u8,

    // Playback
    playing: bool,
    playhead: f32, // Position in beats
    play_start_time: Option<Instant>,
    play_start_beat: f32,

    // Audio output
    _audio_stream: Option<OutputStream>,
    audio_handle: Option<OutputStreamHandle>,
    /// Tracks which notes have been triggered in current playback (by index)
    triggered_notes: HashSet<usize>,

    // UI state
    show_about: bool,
    show_file_browser: bool,
    file_browser: FileBrowser,
    is_saving: bool,
    save_filename: String,
    show_close_confirm: bool,
    close_confirmed: bool,
}

impl SlowMidiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Initialize audio output
        let (stream, handle) = OutputStream::try_default().ok().unzip();

        Self {
            project: MidiProject::default(),
            file_path: None,
            modified: false,

            undo_stack: Vec::new(),
            redo_stack: Vec::new(),

            view_mode: ViewMode::PianoRoll,
            scroll_x: 0.0,
            scroll_y: 30.0 * KEY_HEIGHT, // Start around middle C
            zoom: 1.0,

            edit_tool: EditTool::Draw,
            selected_notes: Vec::new(),
            note_duration: 1.0,
            grid_division: 1.0, // Quarter notes by default
            scale_root: 0,     // C
            scale_type: 0,     // Chromatic (no quantize)

            is_painting: false,
            last_paint_beat: -1.0,
            last_paint_pitch: 255,

            playing: false,
            playhead: 0.0,
            play_start_time: None,
            play_start_beat: 0.0,

            _audio_stream: stream,
            audio_handle: handle,
            triggered_notes: HashSet::new(),

            show_about: false,
            show_file_browser: false,
            file_browser: FileBrowser::new(documents_dir()),
            is_saving: false,
            save_filename: String::new(),
            show_close_confirm: false,
            close_confirmed: false,
        }
    }

    /// Save current state to undo stack before making changes
    fn save_undo_state(&mut self) {
        self.undo_stack.push(self.project.notes.clone());
        self.redo_stack.clear();
        // Limit undo history to 50 states
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
    }

    /// Undo last change
    fn undo(&mut self) {
        if let Some(prev_notes) = self.undo_stack.pop() {
            self.redo_stack.push(self.project.notes.clone());
            self.project.notes = prev_notes;
            self.selected_notes.clear();
            self.modified = true;
        }
    }

    /// Redo last undone change
    fn redo(&mut self) {
        if let Some(next_notes) = self.redo_stack.pop() {
            self.undo_stack.push(self.project.notes.clone());
            self.project.notes = next_notes;
            self.selected_notes.clear();
            self.modified = true;
        }
    }

    /// Play a single note as a sine wave
    fn play_note(&self, pitch: u8, duration_beats: f32) {
        if let Some(ref handle) = self.audio_handle {
            let freq = midi_to_freq(pitch);
            // Convert duration in beats to milliseconds
            let duration_ms = (duration_beats * 60.0 * 1000.0 / self.project.tempo as f32) as u32;
            let duration_ms = duration_ms.min(2000); // Cap at 2 seconds
            let source = SineWave::new(freq, duration_ms);
            if let Ok(sink) = Sink::try_new(handle) {
                // Conservative volume to protect speakers
                sink.set_volume(0.3);
                sink.append(source);
                sink.detach(); // Let it play without blocking
            }
        }
    }

    fn handle_keys(&mut self, ctx: &Context) {
        // Consume Tab and Cmd+/- to prevent menu focus and zoom
        slowcore::theme::consume_special_keys(ctx);

        // Handle dropped MIDI files (drag-and-drop)
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter()
                .filter_map(|f| f.path.clone())
                .filter(|p| {
                    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                    ext == "mid" || ext == "midi" || ext == "json"
                })
                .collect()
        });
        if let Some(path) = dropped.into_iter().next() {
            self.load_from_path(path);
        }

        ctx.input(|i| {
            let cmd = i.modifiers.command;

            // Transport
            if i.key_pressed(Key::Space) {
                self.toggle_playback();
            }

            // File operations
            if cmd && i.key_pressed(Key::N) {
                self.new_project();
            }
            if cmd && i.key_pressed(Key::O) {
                self.show_open_dialog();
            }
            if cmd && i.key_pressed(Key::S) {
                self.save_project();
            }

            // View switching
            if i.key_pressed(Key::Num1) {
                self.view_mode = ViewMode::PianoRoll;
            }
            if i.key_pressed(Key::Num2) {
                self.view_mode = ViewMode::Notation;
            }

            // Tool switching
            if i.key_pressed(Key::V) {
                self.edit_tool = EditTool::Select;
            }
            if i.key_pressed(Key::D) {
                self.edit_tool = EditTool::Draw;
            }
            if i.key_pressed(Key::P) {
                self.edit_tool = EditTool::Paint;
            }
            if i.key_pressed(Key::E) {
                self.edit_tool = EditTool::Erase;
            }

            // Delete selected
            if i.key_pressed(Key::Backspace) || i.key_pressed(Key::Delete) {
                self.delete_selected();
            }

            // Select all
            if cmd && i.key_pressed(Key::A) {
                self.select_all();
            }

            // Undo/Redo
            if cmd && i.key_pressed(Key::Z) {
                if i.modifiers.shift {
                    self.redo();
                } else {
                    self.undo();
                }
            }
        });
    }

    fn toggle_playback(&mut self) {
        if self.playing {
            self.playing = false;
            self.play_start_time = None;
        } else {
            self.playing = true;
            self.play_start_time = Some(Instant::now());
            self.play_start_beat = self.playhead;
            // Clear triggered notes when starting playback
            self.triggered_notes.clear();
        }
    }

    fn update_playback(&mut self) {
        if self.playing {
            if let Some(start_time) = self.play_start_time {
                let elapsed_secs = start_time.elapsed().as_secs_f32();
                let beats_per_second = self.project.tempo as f32 / 60.0;
                let old_playhead = self.playhead;
                self.playhead = self.play_start_beat + elapsed_secs * beats_per_second;

                // Find notes that the playhead just passed over
                let notes_to_play: Vec<(usize, u8, f32)> = self.project.notes.iter().enumerate()
                    .filter(|(idx, note)| {
                        // Note starts between old and new playhead position
                        note.start >= old_playhead && note.start < self.playhead
                            && !self.triggered_notes.contains(idx)
                    })
                    .map(|(idx, note)| (idx, note.pitch, note.duration))
                    .collect();

                // Mark notes as triggered and play them
                for (idx, pitch, duration) in notes_to_play {
                    self.triggered_notes.insert(idx);
                    self.play_note(pitch, duration);
                }

                // Loop at end of content
                let max_beat = self.project.notes.iter()
                    .map(|n| n.start + n.duration)
                    .fold(4.0_f32, |a, b| a.max(b));
                if self.playhead > max_beat {
                    self.playhead = 0.0;
                    self.play_start_time = Some(Instant::now());
                    self.play_start_beat = 0.0;
                    self.triggered_notes.clear(); // Reset for loop
                }
            }
        }
    }

    fn new_project(&mut self) {
        self.project = MidiProject::default();
        self.file_path = None;
        self.modified = false;
        self.selected_notes.clear();
        self.playhead = 0.0;
        self.playing = false;
    }

    fn show_open_dialog(&mut self) {
        self.file_browser = FileBrowser::new(documents_dir())
            .with_filter(vec!["mid".into(), "midi".into(), "json".into()]);
        self.show_file_browser = true;
        self.is_saving = false;
    }

    fn show_save_dialog(&mut self) {
        self.file_browser = FileBrowser::new(documents_dir())
            .with_filter(vec!["mid".into(), "midi".into()]);
        self.show_file_browser = true;
        self.is_saving = true;
        self.save_filename = "untitled.mid".into();
    }

    fn save_project(&mut self) {
        if let Some(ref path) = self.file_path {
            self.save_to_path(path.clone());
        } else {
            self.show_save_dialog();
        }
    }

    fn save_to_path(&mut self, path: PathBuf) {
        // Export as standard MIDI file
        if let Ok(data) = self.export_midi() {
            if std::fs::write(&path, data).is_ok() {
                self.file_path = Some(path);
                self.modified = false;
            }
        }
    }

    fn export_midi(&self) -> Result<Vec<u8>, ()> {
        use midly::{Header, Format, Timing, Smf, Track, TrackEvent, TrackEventKind, MidiMessage};
        use midly::num::{u4, u7, u28};

        let ticks_per_beat: u16 = 480;

        // Create MIDI events from notes
        let mut events: Vec<(u32, TrackEventKind)> = Vec::new();

        // Add tempo meta event at start (microseconds per beat = 60_000_000 / BPM)
        let tempo_us = 60_000_000 / self.project.tempo;
        events.push((0, TrackEventKind::Meta(midly::MetaMessage::Tempo(
            midly::num::u24::new(tempo_us)
        ))));

        // Convert notes to MIDI events
        for note in &self.project.notes {
            let start_tick = (note.start * ticks_per_beat as f32) as u32;
            let end_tick = ((note.start + note.duration) * ticks_per_beat as f32) as u32;
            let channel = u4::new(0);
            let key = u7::new(note.pitch);
            let vel = u7::new(note.velocity);

            // Note on
            events.push((start_tick, TrackEventKind::Midi {
                channel,
                message: MidiMessage::NoteOn { key, vel },
            }));

            // Note off
            events.push((end_tick, TrackEventKind::Midi {
                channel,
                message: MidiMessage::NoteOff { key, vel: u7::new(0) },
            }));
        }

        // Sort by time
        events.sort_by_key(|(time, _)| *time);

        // Convert to delta times
        let mut track: Track = Vec::new();
        let mut last_time: u32 = 0;
        for (time, kind) in events {
            let delta = time - last_time;
            track.push(TrackEvent {
                delta: u28::new(delta),
                kind,
            });
            last_time = time;
        }

        // Add end of track
        track.push(TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(midly::MetaMessage::EndOfTrack),
        });

        let smf = Smf {
            header: Header {
                format: Format::SingleTrack,
                timing: Timing::Metrical(midly::num::u15::new(ticks_per_beat)),
            },
            tracks: vec![track],
        };

        let mut buffer = Vec::new();
        smf.write(&mut buffer).map_err(|_| ())?;
        Ok(buffer)
    }

    fn load_from_path(&mut self, path: PathBuf) {
        // Try loading as JSON first
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(project) = serde_json::from_str::<MidiProject>(&content) {
                self.project = project;
                self.file_path = Some(path);
                self.modified = false;
                self.selected_notes.clear();
                self.playhead = 0.0;
                return;
            }
        }

        // Try loading as MIDI file
        if let Ok(data) = std::fs::read(&path) {
            if let Ok(smf) = midly::Smf::parse(&data) {
                self.import_midi(&smf);
                self.file_path = Some(path);
                self.modified = false;
                self.selected_notes.clear();
                self.playhead = 0.0;
            }
        }
    }

    fn import_midi(&mut self, smf: &midly::Smf) {
        self.project = MidiProject::default();
        let ticks_per_beat = match smf.header.timing {
            midly::Timing::Metrical(tpb) => tpb.as_int() as f32,
            _ => 480.0,
        };

        for track in &smf.tracks {
            let mut time: u32 = 0;
            let mut pending_notes: std::collections::HashMap<u8, (f32, u8)> = std::collections::HashMap::new();

            for event in track {
                time += event.delta.as_int();
                let beat = time as f32 / ticks_per_beat;

                match event.kind {
                    midly::TrackEventKind::Midi { message, .. } => {
                        match message {
                            midly::MidiMessage::NoteOn { key, vel } => {
                                if vel.as_int() > 0 {
                                    pending_notes.insert(key.as_int(), (beat, vel.as_int()));
                                } else {
                                    // Note off
                                    if let Some((start, velocity)) = pending_notes.remove(&key.as_int()) {
                                        self.project.notes.push(MidiNote {
                                            pitch: key.as_int(),
                                            start,
                                            duration: (beat - start).max(0.1),
                                            velocity,
                                        });
                                    }
                                }
                            }
                            midly::MidiMessage::NoteOff { key, .. } => {
                                if let Some((start, velocity)) = pending_notes.remove(&key.as_int()) {
                                    self.project.notes.push(MidiNote {
                                        pitch: key.as_int(),
                                        start,
                                        duration: (beat - start).max(0.1),
                                        velocity,
                                    });
                                }
                            }
                            _ => {}
                        }
                    }
                    midly::TrackEventKind::Meta(midly::MetaMessage::Tempo(tempo)) => {
                        self.project.tempo = (60_000_000 / tempo.as_int()) as u32;
                    }
                    _ => {}
                }
            }
        }
    }

    fn delete_selected(&mut self) {
        if !self.selected_notes.is_empty() {
            self.save_undo_state();
            let mut indices: Vec<usize> = self.selected_notes.drain(..).collect();
            indices.sort_by(|a, b| b.cmp(a)); // Sort descending
            for idx in indices {
                if idx < self.project.notes.len() {
                    self.project.notes.remove(idx);
                }
            }
            self.modified = true;
        }
    }

    fn select_all(&mut self) {
        self.selected_notes = (0..self.project.notes.len()).collect();
    }

    fn note_name(pitch: u8) -> String {
        let octave = (pitch as i32 / 12) - 1;
        let note = NOTE_NAMES[(pitch % 12) as usize];
        format!("{}{}", note, octave)
    }

    fn is_black_key(pitch: u8) -> bool {
        matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
    }

    // ---------------------------------------------------------------
    // Rendering
    // ---------------------------------------------------------------

    fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Transport controls
            let play_label = if self.playing { "stop" } else { "play" };
            if ui.button(play_label).clicked() {
                self.toggle_playback();
            }

            if ui.button("|<").on_hover_text("rewind").clicked() {
                self.playhead = 0.0;
                self.play_start_time = Some(Instant::now());
                self.play_start_beat = 0.0;
            }

            ui.separator();

            // Tool dropdown
            let tool_name = match self.edit_tool {
                EditTool::Select => "select",
                EditTool::Draw => "draw",
                EditTool::Paint => "paint",
                EditTool::Erase => "erase",
            };
            ui.menu_button(format!("tool: {}", tool_name), |ui| {
                if ui.button("select (v)").clicked() {
                    self.edit_tool = EditTool::Select;
                    ui.close_menu();
                }
                if ui.button("draw (d)").clicked() {
                    self.edit_tool = EditTool::Draw;
                    ui.close_menu();
                }
                if ui.button("paint (p)").clicked() {
                    self.edit_tool = EditTool::Paint;
                    ui.close_menu();
                }
                if ui.button("erase (e)").clicked() {
                    self.edit_tool = EditTool::Erase;
                    ui.close_menu();
                }
            });

            ui.separator();

            // Duration dropdown
            let dur_name = match self.note_duration {
                d if (d - 0.25).abs() < 0.01 => "1/16",
                d if (d - 0.5).abs() < 0.01 => "1/8",
                d if (d - 1.0).abs() < 0.01 => "1/4",
                d if (d - 2.0).abs() < 0.01 => "1/2",
                d if (d - 4.0).abs() < 0.01 => "1",
                _ => "1/4",
            };
            ui.menu_button(format!("duration: {}", dur_name), |ui| {
                if ui.button("1/16 (sixteenth)").clicked() {
                    self.note_duration = 0.25;
                    ui.close_menu();
                }
                if ui.button("1/8 (eighth)").clicked() {
                    self.note_duration = 0.5;
                    ui.close_menu();
                }
                if ui.button("1/4 (quarter)").clicked() {
                    self.note_duration = 1.0;
                    ui.close_menu();
                }
                if ui.button("1/2 (half)").clicked() {
                    self.note_duration = 2.0;
                    ui.close_menu();
                }
                if ui.button("1 (whole)").clicked() {
                    self.note_duration = 4.0;
                    ui.close_menu();
                }
            });

            ui.separator();

            // Grid division dropdown
            let grid_name = match self.grid_division {
                d if (d - 0.25).abs() < 0.01 => "1/16",
                d if (d - 0.5).abs() < 0.01 => "1/8",
                d if (d - 1.0).abs() < 0.01 => "1/4",
                d if (d - 2.0).abs() < 0.01 => "1/2",
                d if (d - 4.0).abs() < 0.01 => "1",
                _ => "1/4",
            };
            ui.menu_button(format!("grid: {}", grid_name), |ui| {
                if ui.button("1/16 (sixteenth)").clicked() {
                    self.grid_division = 0.25;
                    ui.close_menu();
                }
                if ui.button("1/8 (eighth)").clicked() {
                    self.grid_division = 0.5;
                    ui.close_menu();
                }
                if ui.button("1/4 (quarter)").clicked() {
                    self.grid_division = 1.0;
                    ui.close_menu();
                }
                if ui.button("1/2 (half)").clicked() {
                    self.grid_division = 2.0;
                    ui.close_menu();
                }
                if ui.button("1 (whole)").clicked() {
                    self.grid_division = 4.0;
                    ui.close_menu();
                }
            });

            ui.separator();

            // Scale quantize - root note
            let root_label = SCALE_ROOT_NAMES[self.scale_root as usize];
            egui::ComboBox::from_id_source("scale_root")
                .selected_text(root_label)
                .width(36.0)
                .show_ui(ui, |ui| {
                    for (i, name) in SCALE_ROOT_NAMES.iter().enumerate() {
                        ui.selectable_value(&mut self.scale_root, i as u8, *name);
                    }
                });

            // Scale quantize - scale type
            let scale_label = SCALE_TYPES[self.scale_type].0;
            egui::ComboBox::from_id_source("scale_type")
                .selected_text(scale_label)
                .width(80.0)
                .show_ui(ui, |ui| {
                    for (i, (name, _)) in SCALE_TYPES.iter().enumerate() {
                        ui.selectable_value(&mut self.scale_type, i, *name);
                    }
                });

            ui.separator();

            // Tempo
            ui.label("tempo:");
            let mut tempo = self.project.tempo as i32;
            if ui.add(egui::DragValue::new(&mut tempo).clamp_range(40..=240)).changed() {
                self.project.tempo = tempo.clamp(40, 240) as u32;
                self.modified = true;
            }
            ui.label("BPM");
        });
    }

    fn render_piano_roll(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let (response, painter) = ui.allocate_painter(available, Sense::click_and_drag());
        let rect = response.rect;

        // Background
        painter.rect_filled(rect, 0.0, SlowColors::WHITE);

        let key_height = KEY_HEIGHT * self.zoom;
        let beat_width = BEAT_WIDTH * self.zoom;
        let piano_width = PIANO_WIDTH;

        let visible_start_key = (self.scroll_y / key_height) as u8;
        let visible_keys = (rect.height() / key_height) as u8 + 2;

        // Draw grid in the piano roll area
        let grid_rect = Rect::from_min_max(
            Pos2::new(rect.min.x + piano_width, rect.min.y),
            rect.max,
        );
        painter.rect_filled(grid_rect, 0.0, SlowColors::WHITE);

        // Horizontal grid lines (key divisions)
        for i in 0..visible_keys {
            let y = rect.min.y + (i as f32) * key_height - (self.scroll_y % key_height);
            painter.hline(
                grid_rect.x_range(),
                y,
                Stroke::new(0.5, SlowColors::BLACK),
            );
        }

        // Vertical grid lines (based on grid division)
        let grid_step = self.grid_division;
        let grid_step_width = grid_step * beat_width;
        let visible_start = (self.scroll_x / grid_step_width).floor() * grid_step;
        let visible_end = visible_start + (grid_rect.width() / grid_step_width).ceil() * grid_step + grid_step;

        let mut beat = visible_start;
        while beat < visible_end {
            let x = grid_rect.min.x + beat * beat_width - self.scroll_x;
            // Thicker line at measure boundaries (every 4 beats)
            let stroke_width = if (beat % 4.0).abs() < 0.001 {
                1.5
            } else if (beat % 1.0).abs() < 0.001 {
                // Quarter note boundary
                1.0
            } else {
                0.5
            };
            painter.vline(
                x,
                grid_rect.y_range(),
                Stroke::new(stroke_width, SlowColors::BLACK),
            );
            beat += grid_step;
        }

        // Draw playhead
        let playhead_x = grid_rect.min.x + self.playhead * beat_width - self.scroll_x;
        if playhead_x >= grid_rect.min.x && playhead_x <= grid_rect.max.x {
            painter.vline(
                playhead_x,
                grid_rect.y_range(),
                Stroke::new(2.0, SlowColors::BLACK),
            );
        }

        // Draw notes
        for (idx, note) in self.project.notes.iter().enumerate() {
            let note_x = grid_rect.min.x + note.start * beat_width - self.scroll_x;
            let note_w = note.duration * beat_width;
            let note_y = rect.min.y + ((127 - note.pitch) as f32) * key_height - self.scroll_y;

            let note_rect = Rect::from_min_size(
                Pos2::new(note_x, note_y),
                Vec2::new(note_w, key_height),
            );

            // Skip if not visible
            if !note_rect.intersects(grid_rect) {
                continue;
            }

            let is_selected = self.selected_notes.contains(&idx);
            let fill = if is_selected {
                slowcore::dither::draw_dither_selection(&painter, note_rect);
                continue;
            } else {
                SlowColors::BLACK
            };

            painter.rect_filled(note_rect, 0.0, fill);
            painter.rect_stroke(note_rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));
        }

        // Draw opaque background for piano keys to cover grid and notes
        let piano_bg_rect = Rect::from_min_size(
            rect.min,
            Vec2::new(piano_width, rect.height()),
        );
        painter.rect_filled(piano_bg_rect, 0.0, SlowColors::WHITE);

        // Draw piano keys on the left (after notes so they're always on top)
        for i in 0..visible_keys {
            let key = 127u8.saturating_sub(visible_start_key + i);
            if key > 127 {
                continue;
            }

            let y = rect.min.y + (i as f32) * key_height - (self.scroll_y % key_height);
            let key_rect = Rect::from_min_size(
                Pos2::new(rect.min.x, y),
                Vec2::new(piano_width, key_height),
            );

            // Key color - fully opaque
            let fill = if Self::is_black_key(key) {
                SlowColors::BLACK
            } else {
                SlowColors::WHITE
            };
            let text_color = if Self::is_black_key(key) {
                SlowColors::WHITE
            } else {
                SlowColors::BLACK
            };

            painter.rect_filled(key_rect, 0.0, fill);
            painter.rect_stroke(key_rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));

            // Note name (only for C notes)
            if key % 12 == 0 {
                painter.text(
                    key_rect.left_center() + Vec2::new(4.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    Self::note_name(key),
                    egui::FontId::proportional(9.0),
                    text_color,
                );
            }
        }

        // Handle interactions
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                if pos.x > rect.min.x + piano_width {
                    // Click in grid area
                    let beat = ((pos.x - grid_rect.min.x + self.scroll_x) / beat_width).max(0.0);
                    let pitch = 127 - ((pos.y - rect.min.y + self.scroll_y) / key_height) as u8;

                    match self.edit_tool {
                        EditTool::Draw | EditTool::Paint => {
                            // Check if clicking on an existing note - if so, remove it (toggle behavior)
                            let mut existing_note = None;
                            for (idx, note) in self.project.notes.iter().enumerate() {
                                let note_x = grid_rect.min.x + note.start * beat_width - self.scroll_x;
                                let note_w = note.duration * beat_width;
                                let note_y = rect.min.y + ((127 - note.pitch) as f32) * key_height - self.scroll_y;
                                let note_rect = Rect::from_min_size(
                                    Pos2::new(note_x, note_y),
                                    Vec2::new(note_w, key_height),
                                );
                                if note_rect.contains(pos) {
                                    existing_note = Some(idx);
                                    break;
                                }
                            }

                            self.save_undo_state();
                            if let Some(idx) = existing_note {
                                // Remove existing note (toggle off)
                                self.project.notes.remove(idx);
                                self.selected_notes.clear();
                            } else {
                                // Add new note
                                let quantized_beat = (beat / self.note_duration).floor() * self.note_duration;
                                let pitch = quantize_to_scale(pitch, self.scale_root, SCALE_TYPES[self.scale_type].1);
                                self.project.notes.push(MidiNote::new(pitch, quantized_beat, self.note_duration));
                                // Play preview sound
                                self.play_note(pitch, self.note_duration.min(0.5));
                                // Track for paint tool
                                self.last_paint_beat = quantized_beat;
                                self.last_paint_pitch = pitch;
                            }
                            self.modified = true;
                        }
                        EditTool::Select => {
                            // Find clicked note
                            self.selected_notes.clear();
                            for (idx, note) in self.project.notes.iter().enumerate() {
                                let note_x = grid_rect.min.x + note.start * beat_width - self.scroll_x;
                                let note_w = note.duration * beat_width;
                                let note_y = rect.min.y + ((127 - note.pitch) as f32) * key_height - self.scroll_y;
                                let note_rect = Rect::from_min_size(
                                    Pos2::new(note_x, note_y),
                                    Vec2::new(note_w, key_height),
                                );
                                if note_rect.contains(pos) {
                                    self.selected_notes.push(idx);
                                    break;
                                }
                            }
                        }
                        EditTool::Erase => {
                            // Find and remove clicked note
                            let mut to_remove = None;
                            for (idx, note) in self.project.notes.iter().enumerate() {
                                let note_x = grid_rect.min.x + note.start * beat_width - self.scroll_x;
                                let note_w = note.duration * beat_width;
                                let note_y = rect.min.y + ((127 - note.pitch) as f32) * key_height - self.scroll_y;
                                let note_rect = Rect::from_min_size(
                                    Pos2::new(note_x, note_y),
                                    Vec2::new(note_w, key_height),
                                );
                                if note_rect.contains(pos) {
                                    to_remove = Some(idx);
                                    break;
                                }
                            }
                            if let Some(idx) = to_remove {
                                self.save_undo_state();
                                self.project.notes.remove(idx);
                                self.modified = true;
                            }
                        }
                    }
                } else {
                    // Click on piano keys - play the note
                    let pitch = 127 - ((pos.y - rect.min.y + self.scroll_y) / key_height) as u8;
                    self.play_note(pitch, 0.5);
                }
            }
        }

        // Paint tool - continuous drawing while dragging
        if self.edit_tool == EditTool::Paint && response.dragged_by(egui::PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                if pos.x > rect.min.x + piano_width {
                    let beat = ((pos.x - grid_rect.min.x + self.scroll_x) / beat_width).max(0.0);
                    let raw_pitch = 127 - ((pos.y - rect.min.y + self.scroll_y) / key_height) as u8;
                    let pitch = quantize_to_scale(raw_pitch, self.scale_root, SCALE_TYPES[self.scale_type].1);
                    let quantized_beat = (beat / self.note_duration).floor() * self.note_duration;

                    // Only add note if position changed significantly
                    if (quantized_beat - self.last_paint_beat).abs() >= self.note_duration * 0.5
                        || pitch != self.last_paint_pitch
                    {
                        // Check if note already exists at this position
                        let exists = self.project.notes.iter().any(|n| {
                            (n.start - quantized_beat).abs() < 0.01 && n.pitch == pitch
                        });

                        if !exists {
                            self.project.notes.push(MidiNote::new(pitch, quantized_beat, self.note_duration));
                            // Play preview when pitch changes
                            if pitch != self.last_paint_pitch {
                                self.play_note(pitch, self.note_duration.min(0.25));
                            }
                            self.last_paint_beat = quantized_beat;
                            self.last_paint_pitch = pitch;
                            self.modified = true;
                        }
                    }
                }
            }
        }

        // Erase tool - continuous erasing while dragging
        if self.edit_tool == EditTool::Erase && response.dragged_by(egui::PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                if pos.x > rect.min.x + piano_width {
                    let beat = ((pos.x - grid_rect.min.x + self.scroll_x) / beat_width).max(0.0);
                    let pitch = 127 - ((pos.y - rect.min.y + self.scroll_y) / key_height) as u8;

                    // Find and remove any note under the cursor
                    let mut to_remove = None;
                    for (idx, note) in self.project.notes.iter().enumerate() {
                        let note_x = grid_rect.min.x + note.start * beat_width - self.scroll_x;
                        let note_w = note.duration * beat_width;
                        let note_y = rect.min.y + ((127 - note.pitch) as f32) * key_height - self.scroll_y;
                        let note_rect = Rect::from_min_size(
                            Pos2::new(note_x, note_y),
                            Vec2::new(note_w, key_height),
                        );
                        if note_rect.contains(pos) {
                            to_remove = Some(idx);
                            break;
                        }
                    }
                    if let Some(idx) = to_remove {
                        self.project.notes.remove(idx);
                        self.modified = true;
                    }
                }
            }
        }

        // Reset paint state when not dragging
        if !response.dragged() {
            self.is_painting = false;
        }

        // Scroll with drag (right mouse button)
        if response.dragged_by(egui::PointerButton::Secondary) {
            let delta = response.drag_delta();
            self.scroll_x = (self.scroll_x - delta.x).max(0.0);
            self.scroll_y = (self.scroll_y - delta.y).max(0.0);
        }

        // Scroll with mouse wheel
        if response.hovered() {
            ui.input(|i| {
                let scroll = i.raw_scroll_delta;
                if scroll != Vec2::ZERO {
                    // Horizontal scroll (shift+scroll or trackpad horizontal)
                    self.scroll_x = (self.scroll_x - scroll.x * 2.0).max(0.0);
                    // Vertical scroll
                    self.scroll_y = (self.scroll_y - scroll.y * 2.0).max(0.0);
                }
            });
        }

        // Auto-scroll when playhead goes past the view
        if self.playing {
            let view_width = grid_rect.width();
            let playhead_screen_x = self.playhead * beat_width - self.scroll_x;
            if playhead_screen_x > view_width * 0.9 {
                // Snap to next "page"
                self.scroll_x = self.playhead * beat_width - view_width * 0.1;
            }
        }

        // Border
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));
    }

    fn render_notation(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let (response, painter) = ui.allocate_painter(available, Sense::click_and_drag());
        let rect = response.rect;

        // Background
        painter.rect_filled(rect, 0.0, SlowColors::WHITE);

        // Staff settings
        let staff_spacing = 10.0;
        let measure_width = BEAT_WIDTH * 3.0; // Slightly narrower for wrapping
        let clef_margin = 50.0;
        let system_height = 120.0; // Height for one treble+bass system
        let system_margin = 20.0;

        // Calculate how many measures fit per line
        let usable_width = rect.width() - clef_margin - 20.0;
        let measures_per_line = ((usable_width / measure_width) as i32).max(1);
        let line_width = measures_per_line as f32 * measure_width;

        // Find total number of measures needed (always add extra lines for adding notes)
        let max_beat = self.project.notes.iter()
            .map(|n| n.start + n.duration)
            .fold(4.0_f32, |a, b| a.max(b));
        let total_measures = ((max_beat / 4.0).ceil() as i32).max(4) + measures_per_line * 2; // Extra lines
        let num_lines = ((total_measures + measures_per_line - 1) / measures_per_line).max(2);

        // Calculate how many lines fit in view
        let lines_visible = ((rect.height() - 40.0) / (system_height + system_margin)) as i32;

        // Draw each line (system) - render all lines, not just visible+1
        for line in 0..num_lines {
            let line_y = rect.min.y + 30.0 + (line as f32) * (system_height + system_margin) - self.scroll_y;

            // Skip if off screen
            if line_y + system_height < rect.min.y || line_y > rect.max.y {
                continue;
            }

            let staff_start_y = line_y;
            let bass_start_y = staff_start_y + 70.0;
            let line_start_x = rect.min.x + clef_margin;
            let line_end_x = (line_start_x + line_width).min(rect.max.x);

            // Draw treble clef staff (5 lines)
            for i in 0..5 {
                let y = staff_start_y + (i as f32) * staff_spacing;
                painter.hline(
                    line_start_x..=line_end_x,
                    y,
                    Stroke::new(1.0, SlowColors::BLACK),
                );
            }

            // Draw bass clef staff
            for i in 0..5 {
                let y = bass_start_y + (i as f32) * staff_spacing;
                painter.hline(
                    line_start_x..=line_end_x,
                    y,
                    Stroke::new(1.0, SlowColors::BLACK),
                );
            }

            // Draw treble clef (stylized G shape)
            let treble_x = rect.min.x + 20.0;
            let treble_y = staff_start_y + 2.0 * staff_spacing; // Center on G line
            // Draw a stylized treble clef
            painter.circle_stroke(
                Pos2::new(treble_x, treble_y + 5.0),
                8.0,
                Stroke::new(2.0, SlowColors::BLACK),
            );
            painter.line_segment(
                [Pos2::new(treble_x + 4.0, treble_y + 12.0), Pos2::new(treble_x + 4.0, treble_y - 25.0)],
                Stroke::new(2.0, SlowColors::BLACK),
            );
            painter.circle_stroke(
                Pos2::new(treble_x + 4.0, treble_y - 20.0),
                5.0,
                Stroke::new(2.0, SlowColors::BLACK),
            );

            // Draw bass clef (stylized F shape)
            let bass_x = rect.min.x + 20.0;
            let bass_y = bass_start_y + 1.0 * staff_spacing; // Center on F line
            painter.circle_filled(
                Pos2::new(bass_x, bass_y),
                4.0,
                SlowColors::BLACK,
            );
            painter.circle_filled(
                Pos2::new(bass_x + 10.0, bass_y - 6.0),
                2.0,
                SlowColors::BLACK,
            );
            painter.circle_filled(
                Pos2::new(bass_x + 10.0, bass_y + 6.0),
                2.0,
                SlowColors::BLACK,
            );
            // Curved line for bass clef
            painter.line_segment(
                [Pos2::new(bass_x + 4.0, bass_y - 2.0), Pos2::new(bass_x + 8.0, bass_y - 10.0)],
                Stroke::new(2.0, SlowColors::BLACK),
            );
            painter.line_segment(
                [Pos2::new(bass_x + 4.0, bass_y + 2.0), Pos2::new(bass_x + 8.0, bass_y + 10.0)],
                Stroke::new(2.0, SlowColors::BLACK),
            );

            // Draw bar lines for this line
            let first_measure = line * measures_per_line;
            for m in 0..=measures_per_line {
                let measure_num = first_measure + m;
                if measure_num > total_measures { break; }
                let x = line_start_x + (m as f32) * measure_width;
                if x <= line_end_x {
                    painter.vline(
                        x,
                        staff_start_y..=staff_start_y + 4.0 * staff_spacing,
                        Stroke::new(1.0, SlowColors::BLACK),
                    );
                    painter.vline(
                        x,
                        bass_start_y..=bass_start_y + 4.0 * staff_spacing,
                        Stroke::new(1.0, SlowColors::BLACK),
                    );
                }
            }

            // Draw playhead on this line if applicable
            let playhead_measure = (self.playhead / 4.0) as i32;
            if playhead_measure >= first_measure && playhead_measure < first_measure + measures_per_line {
                let playhead_in_line = (self.playhead / 4.0) - first_measure as f32;
                let playhead_x = line_start_x + playhead_in_line * measure_width;
                painter.vline(
                    playhead_x,
                    staff_start_y - 5.0..=bass_start_y + 4.0 * staff_spacing + 5.0,
                    Stroke::new(2.0, SlowColors::BLACK),
                );
            }

            // Draw notes on this line
            for (idx, note) in self.project.notes.iter().enumerate() {
                let note_measure = (note.start / 4.0) as i32;
                if note_measure < first_measure || note_measure >= first_measure + measures_per_line {
                    continue;
                }

                let is_treble = note.pitch >= 60;
                let base_y = if is_treble { staff_start_y } else { bass_start_y };
                let note_y = pitch_to_staff_y(note.pitch, is_treble, base_y, staff_spacing);
                let note_in_line = (note.start / 4.0) - first_measure as f32;
                let note_x = line_start_x + note_in_line * measure_width;

                if note_x >= rect.min.x && note_x <= rect.max.x {
                    let note_size = 5.0;
                    let is_selected = self.selected_notes.contains(&idx);

                    if is_selected {
                        painter.circle_filled(
                            Pos2::new(note_x, note_y),
                            note_size + 3.0,
                            SlowColors::WHITE,
                        );
                        painter.circle_stroke(
                            Pos2::new(note_x, note_y),
                            note_size + 3.0,
                            Stroke::new(2.0, SlowColors::BLACK),
                        );
                    }

                    painter.circle_filled(
                        Pos2::new(note_x, note_y),
                        note_size,
                        SlowColors::BLACK,
                    );

                    if note.duration <= 1.0 {
                        let stem_dir: f32 = if note_y < base_y + 2.0 * staff_spacing { 1.0 } else { -1.0 };
                        painter.line_segment(
                            [
                                Pos2::new(note_x + note_size * stem_dir.signum(), note_y),
                                Pos2::new(note_x + note_size * stem_dir.signum(), note_y - 25.0 * stem_dir),
                            ],
                            Stroke::new(1.0, SlowColors::BLACK),
                        );
                    }
                }
            }
        }

        // Handle scroll for vertical navigation
        let scroll_delta = ui.input(|i| i.raw_scroll_delta);
        if scroll_delta.y != 0.0 {
            self.scroll_y = (self.scroll_y - scroll_delta.y).max(0.0);
        }

        // Handle click interactions for editing
        // Determine which line (system) was clicked
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let click_x = pos.x;
                let click_y = pos.y;

                // Calculate which line was clicked
                let line_float = (click_y - rect.min.y - 30.0 + self.scroll_y) / (system_height + system_margin);
                let clicked_line = line_float.floor() as i32;
                let clicked_line = clicked_line.max(0).min(num_lines - 1);

                // Calculate staff positions for this line
                let line_staff_start_y = rect.min.y + 30.0 + (clicked_line as f32) * (system_height + system_margin) - self.scroll_y;
                let line_bass_start_y = line_staff_start_y + 70.0;
                let line_start_x = rect.min.x + clef_margin;

                // Check if click is on a note (for selection)
                let mut clicked_note = None;
                for (idx, note) in self.project.notes.iter().enumerate() {
                    // Calculate which line this note is on
                    let note_measure = (note.start / 4.0) as i32;
                    let note_line = note_measure / measures_per_line;
                    if note_line != clicked_line { continue; }

                    let is_treble = note.pitch >= 60;
                    let base_y = if is_treble { line_staff_start_y } else { line_bass_start_y };
                    let note_y = pitch_to_staff_y(note.pitch, is_treble, base_y, staff_spacing);
                    let beat_in_line = note.start - (note_line * measures_per_line) as f32 * 4.0;
                    let note_x = line_start_x + (beat_in_line / 4.0) * measure_width;

                    let dist = ((click_x - note_x).powi(2) + (click_y - note_y).powi(2)).sqrt();
                    if dist < 10.0 {
                        clicked_note = Some(idx);
                        break;
                    }
                }

                match self.edit_tool {
                    EditTool::Select => {
                        self.selected_notes.clear();
                        if let Some(idx) = clicked_note {
                            self.selected_notes.push(idx);
                        }
                    }
                    EditTool::Draw | EditTool::Paint => {
                        // Toggle behavior - if clicking on note, remove it; otherwise add
                        self.save_undo_state();
                        if let Some(idx) = clicked_note {
                            self.project.notes.remove(idx);
                            self.selected_notes.clear();
                            self.modified = true;
                        } else if click_x > rect.min.x + 50.0 {
                            // Calculate beat from x position (relative to line start)
                            let x_in_line = click_x - line_start_x;
                            let beat_in_line = (x_in_line / measure_width) * 4.0;
                            let base_beat = (clicked_line * measures_per_line) as f32 * 4.0;
                            let beat = base_beat + beat_in_line;
                            let quantized_beat = (beat / self.note_duration).floor() * self.note_duration;

                            // Calculate pitch from y position
                            // Midpoint between staves: treble bottom + gap to bass top
                            let treble_bottom = line_staff_start_y + 4.0 * staff_spacing;
                            let midpoint = (treble_bottom + line_bass_start_y) / 2.0;

                            let is_treble = click_y < midpoint;
                            let staff_base = if is_treble { line_staff_start_y } else { line_bass_start_y };
                            let raw_pitch = staff_y_to_pitch(click_y, is_treble, staff_base, staff_spacing);
                            let final_pitch = quantize_to_scale(raw_pitch, self.scale_root, SCALE_TYPES[self.scale_type].1);
                            self.project.notes.push(MidiNote::new(final_pitch, quantized_beat, self.note_duration));
                            // Play preview sound
                            self.play_note(final_pitch, self.note_duration.min(0.5));
                            // Track for paint tool
                            self.last_paint_beat = quantized_beat;
                            self.last_paint_pitch = final_pitch;
                            self.modified = true;
                        }
                    }
                    EditTool::Erase => {
                        // Delete clicked note
                        if let Some(idx) = clicked_note {
                            self.save_undo_state();
                            self.project.notes.remove(idx);
                            self.selected_notes.clear();
                            self.modified = true;
                        }
                    }
                }
            }
        }

        // Paint tool - continuous drawing while dragging in notation view
        if self.edit_tool == EditTool::Paint && response.dragged_by(egui::PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                if pos.x > rect.min.x + 50.0 {
                    // Calculate which line we're on
                    let line_float = (pos.y - rect.min.y - 30.0 + self.scroll_y) / (system_height + system_margin);
                    let drag_line = (line_float.floor() as i32).max(0).min(num_lines - 1);
                    let line_staff_start_y = rect.min.y + 30.0 + (drag_line as f32) * (system_height + system_margin) - self.scroll_y;
                    let line_bass_start_y = line_staff_start_y + 70.0;
                    let line_start_x = rect.min.x + clef_margin;

                    // Calculate beat from x position (relative to line)
                    let x_in_line = pos.x - line_start_x;
                    let beat_in_line = (x_in_line / measure_width) * 4.0;
                    let base_beat = (drag_line * measures_per_line) as f32 * 4.0;
                    let beat = base_beat + beat_in_line;
                    let quantized_beat = (beat / self.note_duration).floor() * self.note_duration;

                    // Calculate pitch from y position
                    let treble_bottom = line_staff_start_y + 4.0 * staff_spacing;
                    let midpoint = (treble_bottom + line_bass_start_y) / 2.0;

                    let is_treble = pos.y < midpoint;
                    let staff_base = if is_treble { line_staff_start_y } else { line_bass_start_y };
                    let raw_pitch = staff_y_to_pitch(pos.y, is_treble, staff_base, staff_spacing);
                    let pitch = quantize_to_scale(raw_pitch, self.scale_root, SCALE_TYPES[self.scale_type].1);

                    // Only add note if position changed significantly
                    if (quantized_beat - self.last_paint_beat).abs() >= self.note_duration * 0.5
                        || pitch != self.last_paint_pitch
                    {
                        // Check if note already exists at this position
                        let exists = self.project.notes.iter().any(|n| {
                            (n.start - quantized_beat).abs() < 0.01 && n.pitch == pitch
                        });

                        if !exists {
                            self.project.notes.push(MidiNote::new(pitch, quantized_beat, self.note_duration));
                            // Play preview when pitch changes
                            if pitch != self.last_paint_pitch {
                                self.play_note(pitch, self.note_duration.min(0.25));
                            }
                            self.last_paint_beat = quantized_beat;
                            self.last_paint_pitch = pitch;
                            self.modified = true;
                        }
                    }
                }
            }
        }

        // Erase tool - continuous erasing while dragging in notation view
        if self.edit_tool == EditTool::Erase && response.dragged_by(egui::PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                // Calculate which line we're on
                let line_float = (pos.y - rect.min.y - 30.0 + self.scroll_y) / (system_height + system_margin);
                let drag_line = (line_float.floor() as i32).max(0).min(num_lines - 1);
                let line_staff_start_y = rect.min.y + 30.0 + (drag_line as f32) * (system_height + system_margin) - self.scroll_y;
                let line_bass_start_y = line_staff_start_y + 70.0;
                let line_start_x = rect.min.x + clef_margin;

                // Find and remove any note near the cursor on this line
                let mut to_remove = None;
                for (idx, note) in self.project.notes.iter().enumerate() {
                    let note_measure = (note.start / 4.0) as i32;
                    let note_line = note_measure / measures_per_line;
                    if note_line != drag_line { continue; }

                    let is_treble = note.pitch >= 60;
                    let base_y = if is_treble { line_staff_start_y } else { line_bass_start_y };
                    let note_y = pitch_to_staff_y(note.pitch, is_treble, base_y, staff_spacing);
                    let beat_in_line = note.start - (note_line * measures_per_line) as f32 * 4.0;
                    let note_x = line_start_x + (beat_in_line / 4.0) * measure_width;

                    let dist = ((pos.x - note_x).powi(2) + (pos.y - note_y).powi(2)).sqrt();
                    if dist < 12.0 {
                        to_remove = Some(idx);
                        break;
                    }
                }
                if let Some(idx) = to_remove {
                    self.project.notes.remove(idx);
                    self.modified = true;
                }
            }
        }

        // Scroll with drag
        if response.dragged_by(egui::PointerButton::Secondary) {
            let delta = response.drag_delta();
            self.scroll_x = (self.scroll_x - delta.x).max(0.0);
        }

        // Scroll with mouse wheel
        if response.hovered() {
            ui.input(|i| {
                let scroll = i.raw_scroll_delta;
                if scroll != Vec2::ZERO {
                    // Horizontal scroll
                    self.scroll_x = (self.scroll_x - scroll.x * 2.0 - scroll.y * 2.0).max(0.0);
                }
            });
        }

        // Auto-scroll when playhead goes past the view
        if self.playing {
            let view_width = rect.width() - 50.0;
            let playhead_screen_x = (self.playhead / 4.0) * measure_width - self.scroll_x;
            if playhead_screen_x > view_width * 0.9 {
                // Snap to next "page"
                self.scroll_x = (self.playhead / 4.0) * measure_width - view_width * 0.1;
            }
        }

        // Instructions
        painter.text(
            Pos2::new(rect.center().x, rect.max.y - 20.0),
            egui::Align2::CENTER_CENTER,
            "click to add/remove notes • scroll to navigate",
            egui::FontId::proportional(11.0),
            SlowColors::BLACK,
        );

        // Border
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));
    }

    fn render_close_confirm(&mut self, ctx: &Context) {
        egui::Window::new("unsaved changes")
            .collapsible(false)
            .resizable(false)
            .default_width(300.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("you have unsaved changes.");
                ui.label("do you want to save before closing?");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("don't save").clicked() {
                        self.close_confirmed = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button("cancel").clicked() {
                        self.show_close_confirm = false;
                    }
                    if ui.button("save").clicked() {
                        self.save_project();
                        if !self.modified {
                            // Save succeeded
                            self.close_confirmed = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                });
            });
    }

    fn render_file_browser(&mut self, ctx: &Context) {
        let title = if self.is_saving { "save project" } else { "open file" };

        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("location:");
                    ui.label(self.file_browser.current_dir.to_string_lossy().to_string());
                });
                ui.separator();

                egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                    let entries = self.file_browser.entries.clone();
                    for (idx, entry) in entries.iter().enumerate() {
                        let selected = self.file_browser.selected_index == Some(idx);
                        let response = ui.add(FileListItem::new(&entry.name, entry.is_directory).selected(selected));
                        if response.clicked() {
                            self.file_browser.selected_index = Some(idx);
                        }
                        if response.double_clicked() {
                            if entry.is_directory {
                                self.file_browser.navigate_to(entry.path.clone());
                            } else if !self.is_saving {
                                self.load_from_path(entry.path.clone());
                                self.show_file_browser = false;
                            }
                        }
                    }
                });

                if self.is_saving {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("filename:");
                        ui.text_edit_singleline(&mut self.save_filename);
                    });
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("cancel").clicked() {
                        self.show_file_browser = false;
                    }
                    let action = if self.is_saving { "save" } else { "open" };
                    if ui.button(action).clicked() {
                        if self.is_saving {
                            if !self.save_filename.is_empty() {
                                let path = self.file_browser.save_directory().join(&self.save_filename);
                                let path = if path.extension().is_none() {
                                    path.with_extension("mid")
                                } else {
                                    path
                                };
                                self.save_to_path(path);
                                self.show_file_browser = false;
                            }
                        } else if let Some(entry) = self.file_browser.selected_entry() {
                            if !entry.is_directory {
                                self.load_from_path(entry.path.clone());
                                self.show_file_browser = false;
                            }
                        }
                    }
                });
            });
    }
}

impl eframe::App for SlowMidiApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.handle_keys(ctx);
        self.update_playback();

        // Request repaint during playback
        if self.playing {
            ctx.request_repaint();
        }

        // Menu bar
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            menu_bar(ui, |ui| {
                ui.menu_button("file", |ui| {
                    if ui.button("new        ⌘N").clicked() {
                        self.new_project();
                        ui.close_menu();
                    }
                    if ui.button("open...    ⌘O").clicked() {
                        self.show_open_dialog();
                        ui.close_menu();
                    }
                    if ui.button("save       ⌘S").clicked() {
                        self.save_project();
                        ui.close_menu();
                    }
                    if ui.button("save as...").clicked() {
                        self.show_save_dialog();
                        ui.close_menu();
                    }
                });
                ui.menu_button("edit", |ui| {
                    let can_undo = !self.undo_stack.is_empty();
                    let can_redo = !self.redo_stack.is_empty();
                    if ui.add_enabled(can_undo, egui::Button::new("undo        ⌘Z")).clicked() {
                        self.undo();
                        ui.close_menu();
                    }
                    if ui.add_enabled(can_redo, egui::Button::new("redo        ⇧⌘Z")).clicked() {
                        self.redo();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("select all  ⌘A").clicked() {
                        self.select_all();
                        ui.close_menu();
                    }
                    if ui.button("delete      ⌫").clicked() {
                        self.delete_selected();
                        ui.close_menu();
                    }
                });
                ui.menu_button("view", |ui| {
                    if ui.button("piano roll  1").clicked() {
                        self.view_mode = ViewMode::PianoRoll;
                        ui.close_menu();
                    }
                    if ui.button("notation    2").clicked() {
                        self.view_mode = ViewMode::Notation;
                        ui.close_menu();
                    }
                });
                ui.menu_button("transport", |ui| {
                    let play_text = if self.playing { "stop   space" } else { "play   space" };
                    if ui.button(play_text).clicked() {
                        self.toggle_playback();
                        ui.close_menu();
                    }
                    if ui.button("rewind").clicked() {
                        self.playhead = 0.0;
                        self.play_start_beat = 0.0;
                        self.play_start_time = Some(Instant::now());
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
            self.render_toolbar(ui);
        });

        // Status bar
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let status = format!(
                "{} notes | beat {:.1} | {} BPM | {}",
                self.project.notes.len(),
                self.playhead,
                self.project.tempo,
                if self.modified { "modified" } else { "saved" }
            );
            status_bar(ui, &status);
        });

        // Main content
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE))
            .show(ctx, |ui| {
                match self.view_mode {
                    ViewMode::PianoRoll => self.render_piano_roll(ui),
                    ViewMode::Notation => self.render_notation(ui),
                }
            });

        // File browser
        if self.show_file_browser {
            self.render_file_browser(ctx);
        }

        // Close confirmation dialog
        if self.show_close_confirm {
            self.render_close_confirm(ctx);
        }

        // About dialog
        if self.show_about {
            egui::Window::new("about slowMidi")
                .collapsible(false)
                .resizable(false)
                .default_width(300.0)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("slowMidi");
                        ui.label("version 0.1.0");
                        ui.add_space(8.0);
                        ui.label("MIDI sequencer for slowOS");
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label("supported formats:");
                    ui.label("  MIDI (.mid, .midi), JSON project");
                    ui.add_space(4.0);
                    ui.label("features:");
                    ui.label("  piano roll and notation views");
                    ui.label("  create and edit MIDI sequences");
                    ui.label("  variable note durations");
                    ui.add_space(4.0);
                    ui.label("frameworks:");
                    ui.label("  egui/eframe (MIT), midly (MIT)");
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        if ui.button("ok").clicked() {
                            self.show_about = false;
                        }
                    });
                });
        }

        // Handle close request
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.modified && !self.close_confirmed {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.show_close_confirm = true;
            }
        }
    }
}
