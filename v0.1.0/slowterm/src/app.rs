//! slowTerm application
//!
//! A minimal terminal emulator for the slow computer.
//! Runs shell commands via /bin/sh, tracks working directory,
//! supports command history, and renders output in a scrollable buffer.

use egui::{Context, FontFamily, FontId, Key, Pos2, Rect, Sense, Stroke, Vec2};
use slowcore::safety::{snap_to_char_boundary, safe_slice_to};
use slowcore::theme::SlowColors;
use std::env;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

/// A single line in the terminal output
#[derive(Clone, Debug)]
struct TermLine {
    text: String,
    kind: LineKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum LineKind {
    /// The prompt + command the user typed
    Command,
    /// Standard output from a command
    Stdout,
    /// Standard error from a command
    Stderr,
    /// System messages (cd confirmation, errors, etc.)
    System,
}

/// Shared state for async command output
#[derive(Clone, Default)]
struct AsyncOutput {
    inner: Arc<Mutex<AsyncOutputInner>>,
}

#[derive(Default)]
struct AsyncOutputInner {
    lines: Vec<TermLine>,
    done: bool,
}

impl AsyncOutput {
    fn push(&self, line: TermLine) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.lines.push(line);
        }
    }

    fn finish(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.done = true;
        }
    }

    fn drain(&self) -> (Vec<TermLine>, bool) {
        if let Ok(mut inner) = self.inner.lock() {
            let lines = std::mem::take(&mut inner.lines);
            (lines, inner.done)
        } else {
            (Vec::new(), false)
        }
    }
}

pub struct SlowTermApp {
    /// All terminal output lines
    buffer: Vec<TermLine>,
    /// Current input line
    input: String,
    /// Cursor position within input
    cursor: usize,
    /// Command history
    history: Vec<String>,
    /// Current position in history (for up/down navigation)
    history_pos: Option<usize>,
    /// Saved input when browsing history
    saved_input: String,
    /// Current working directory
    cwd: PathBuf,
    /// Scroll offset (in lines from bottom)
    scroll_offset: f32,
    /// Whether a command is currently running
    running: bool,
    /// Async output collector for running commands
    async_output: Option<AsyncOutput>,
    /// Max lines to keep in buffer
    max_lines: usize,
    /// Whether to auto-scroll to bottom
    auto_scroll: bool,
    /// Show about dialog
    show_about: bool,
    /// Font size for the terminal
    font_size: f32,
}

impl SlowTermApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let cwd = env::current_dir().unwrap_or_else(|_| {
            dirs_home().unwrap_or_else(|| PathBuf::from("/"))
        });

        let mut app = Self {
            buffer: Vec::new(),
            input: String::new(),
            cursor: 0,
            history: Vec::new(),
            history_pos: None,
            saved_input: String::new(),
            cwd,
            scroll_offset: 0.0,
            running: false,
            async_output: None,
            max_lines: 10_000,
            auto_scroll: true,
            show_about: false,
            font_size: 14.0,
        };

        app.push_line(TermLine {
            text: "slowTerm v0.1.0".to_string(),
            kind: LineKind::System,
        });
        app.push_line(TermLine {
            text: format!("type a command. working directory: {}", app.cwd.display()),
            kind: LineKind::System,
        });

        app
    }

    fn push_line(&mut self, line: TermLine) {
        self.buffer.push(line);
        // Trim buffer if too large
        if self.buffer.len() > self.max_lines {
            let excess = self.buffer.len() - self.max_lines;
            self.buffer.drain(0..excess);
        }
    }

    fn prompt(&self) -> String {
        let dir = self.cwd.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| self.cwd.to_string_lossy().to_string());
        format!("{}$ ", dir)
    }

    fn execute_command(&mut self, cmd_str: &str) {
        let trimmed = cmd_str.trim();
        if trimmed.is_empty() {
            return;
        }

        // Show the command in the buffer
        self.push_line(TermLine {
            text: format!("{}{}", self.prompt(), trimmed),
            kind: LineKind::Command,
        });

        // Add to history (skip duplicates of last command)
        if self.history.last().map(|h| h.as_str()) != Some(trimmed) {
            self.history.push(trimmed.to_string());
        }
        self.history_pos = None;

        // Handle built-in commands
        if let Some(rest) = trimmed.strip_prefix("cd") {
            let target = rest.trim();
            self.handle_cd(target);
            return;
        }

        if trimmed == "clear" {
            self.buffer.clear();
            return;
        }

        if trimmed == "pwd" {
            self.push_line(TermLine {
                text: self.cwd.to_string_lossy().to_string(),
                kind: LineKind::Stdout,
            });
            return;
        }

        if trimmed == "exit" || trimmed == "quit" {
            std::process::exit(0);
        }

        // External command — run asynchronously
        self.running = true;
        let output = AsyncOutput::default();
        self.async_output = Some(output.clone());

        let cwd = self.cwd.clone();
        let cmd = trimmed.to_string();

        thread::spawn(move || {
            let shell = if cfg!(target_os = "windows") { "cmd" } else { "sh" };
            let flag = if cfg!(target_os = "windows") { "/C" } else { "-c" };

            let result = Command::new(shell)
                .arg(flag)
                .arg(&cmd)
                .current_dir(&cwd)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn();

            match result {
                Ok(mut child) => {
                    // Read stdout
                    if let Some(mut stdout) = child.stdout.take() {
                        let mut buf = String::new();
                        let _ = stdout.read_to_string(&mut buf);
                        for line in buf.lines() {
                            output.push(TermLine {
                                text: line.to_string(),
                                kind: LineKind::Stdout,
                            });
                        }
                    }

                    // Read stderr
                    if let Some(mut stderr) = child.stderr.take() {
                        let mut buf = String::new();
                        let _ = stderr.read_to_string(&mut buf);
                        for line in buf.lines() {
                            output.push(TermLine {
                                text: line.to_string(),
                                kind: LineKind::Stderr,
                            });
                        }
                    }

                    let _ = child.wait();
                }
                Err(e) => {
                    output.push(TermLine {
                        text: format!("error: {}", e),
                        kind: LineKind::Stderr,
                    });
                }
            }

            output.finish();
        });
    }

    fn handle_cd(&mut self, target: &str) {
        let path = if target.is_empty() || target == "~" {
            dirs_home().unwrap_or_else(|| self.cwd.clone())
        } else if target.starts_with('~') {
            dirs_home()
                .map(|h| h.join(&target[2..]))
                .unwrap_or_else(|| self.cwd.join(target))
        } else if target.starts_with('/') {
            PathBuf::from(target)
        } else {
            self.cwd.join(target)
        };

        match std::fs::canonicalize(&path) {
            Ok(canonical) => {
                if canonical.is_dir() {
                    self.cwd = canonical;
                    self.push_line(TermLine {
                        text: format!("{}", self.cwd.display()),
                        kind: LineKind::System,
                    });
                } else {
                    self.push_line(TermLine {
                        text: format!("cd: not a directory: {}", path.display()),
                        kind: LineKind::Stderr,
                    });
                }
            }
            Err(e) => {
                self.push_line(TermLine {
                    text: format!("cd: {}: {}", path.display(), e),
                    kind: LineKind::Stderr,
                });
            }
        }
    }

    /// Tab completion for file/directory names
    fn tab_complete(&mut self) {
        let cursor = snap_to_char_boundary(&self.input, self.cursor);
        self.cursor = cursor;
        let input = &self.input[..cursor];
        // Find the last word (space-separated)
        let last_space = input.rfind(' ').map(|i| i + 1).unwrap_or(0);
        let partial = &input[last_space..];
        if partial.is_empty() { return; }

        // Resolve the partial path relative to cwd
        let partial_path = if partial.starts_with('/') {
            PathBuf::from(partial)
        } else if partial.starts_with("~/") {
            dirs_home()
                .map(|h| h.join(&partial[2..]))
                .unwrap_or_else(|| self.cwd.join(partial))
        } else {
            self.cwd.join(partial)
        };

        // Split into directory to search and prefix to match
        let (search_dir, prefix) = if partial_path.is_dir() && partial.ends_with('/') {
            (partial_path.clone(), String::new())
        } else {
            let dir = partial_path.parent().unwrap_or(&self.cwd).to_path_buf();
            let name = partial_path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            (dir, name)
        };

        // List matching entries
        let mut matches: Vec<String> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&search_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(&prefix) {
                    let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                    let suffix = if is_dir { "/" } else { " " };
                    matches.push(format!("{}{}", name, suffix));
                }
            }
        }
        matches.sort();

        if matches.len() == 1 {
            // Single match — complete it
            let completion = &matches[0];
            let to_add = &completion[prefix.len()..];
            self.input.insert_str(self.cursor, to_add);
            self.cursor += to_add.len();
        } else if matches.len() > 1 {
            // Multiple matches — find common prefix and show options
            let common = common_prefix(&matches);
            if common.len() > prefix.len() {
                let to_add = &common[prefix.len()..];
                self.input.insert_str(self.cursor, to_add);
                self.cursor += to_add.len();
            } else {
                // Show all matches
                self.push_line(TermLine {
                    text: format!("{}{}", self.prompt(), self.input),
                    kind: LineKind::Command,
                });
                let display: Vec<&str> = matches.iter().map(|m| m.trim_end()).collect();
                self.push_line(TermLine {
                    text: display.join("  "),
                    kind: LineKind::System,
                });
            }
        }
    }

    /// Poll for async command output
    fn poll_output(&mut self) {
        if let Some(ref ao) = self.async_output {
            let (lines, done) = ao.drain();
            for line in lines {
                self.push_line(line);
            }
            if done {
                self.running = false;
                self.async_output = None;
            }
        }
    }

    fn handle_input(&mut self, ctx: &Context) {
        // Snap cursor to valid char boundary (defensive)
        self.cursor = snap_to_char_boundary(&self.input, self.cursor);

        // Consume Tab key so egui doesn't use it for widget navigation
        let tab_pressed = ctx.input_mut(|i| {
            let pressed = i.key_pressed(Key::Tab);
            if pressed {
                i.events.retain(|e| !matches!(e, egui::Event::Key { key: Key::Tab, .. }));
            }
            pressed
        });

        ctx.input(|i| {
            // Typed characters
            for event in &i.events {
                match event {
                    egui::Event::Text(t) => {
                        if !self.running {
                            self.input.insert_str(self.cursor, t);
                            self.cursor += t.len();
                        }
                    }
                    _ => {}
                }
            }

            if self.running {
                // Ctrl+C to cancel (just marks as done)
                if i.modifiers.ctrl && i.key_pressed(Key::C) {
                    self.push_line(TermLine {
                        text: "^C".to_string(),
                        kind: LineKind::System,
                    });
                    self.running = false;
                    self.async_output = None;
                }
                return;
            }

            // Tab — autocomplete file/directory names
            if tab_pressed && !self.running {
                self.tab_complete();
            }

            // Enter — execute command
            if i.key_pressed(Key::Enter) {
                let cmd = self.input.clone();
                self.input.clear();
                self.cursor = 0;
                self.execute_command(&cmd);
                self.auto_scroll = true;
            }

            // Backspace
            if i.key_pressed(Key::Backspace) {
                if self.cursor > 0 {
                    // Find the previous char boundary
                    let prev = self.input[..self.cursor]
                        .char_indices()
                        .last()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.input.drain(prev..self.cursor);
                    self.cursor = prev;
                }
            }

            // Delete
            if i.key_pressed(Key::Delete) {
                if self.cursor < self.input.len() {
                    let next = self.input[self.cursor..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| self.cursor + i)
                        .unwrap_or(self.input.len());
                    self.input.drain(self.cursor..next);
                }
            }

            // Left arrow / Ctrl+B
            if i.key_pressed(Key::ArrowLeft) || (i.modifiers.ctrl && i.key_pressed(Key::B)) {
                if self.cursor > 0 {
                    self.cursor = self.input[..self.cursor]
                        .char_indices()
                        .last()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                }
            }

            // Right arrow / Ctrl+F
            if i.key_pressed(Key::ArrowRight) || (i.modifiers.ctrl && i.key_pressed(Key::F)) {
                if self.cursor < self.input.len() {
                    self.cursor = self.input[self.cursor..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| self.cursor + i)
                        .unwrap_or(self.input.len());
                }
            }

            // Home / Ctrl+A
            if i.key_pressed(Key::Home) || (i.modifiers.ctrl && i.key_pressed(Key::A)) {
                self.cursor = 0;
            }

            // End / Ctrl+E
            if i.key_pressed(Key::End) || (i.modifiers.ctrl && i.key_pressed(Key::E)) {
                self.cursor = self.input.len();
            }

            // Ctrl+K — kill to end of line
            if i.modifiers.ctrl && i.key_pressed(Key::K) {
                self.input.truncate(self.cursor);
            }

            // Ctrl+U — kill whole line
            if i.modifiers.ctrl && i.key_pressed(Key::U) {
                self.input.clear();
                self.cursor = 0;
            }

            // Ctrl+W — kill last word
            if i.modifiers.ctrl && i.key_pressed(Key::W) {
                if self.cursor > 0 {
                    let before = &self.input[..self.cursor];
                    let trimmed = before.trim_end();
                    let word_start = trimmed.rfind(' ').map(|i| i + 1).unwrap_or(0);
                    self.input.drain(word_start..self.cursor);
                    self.cursor = word_start;
                }
            }

            // Up arrow — history previous
            if i.key_pressed(Key::ArrowUp) {
                if !self.history.is_empty() {
                    match self.history_pos {
                        None => {
                            self.saved_input = self.input.clone();
                            self.history_pos = Some(self.history.len() - 1);
                            self.input = self.history.last().cloned().unwrap_or_default();
                            self.cursor = self.input.len();
                        }
                        Some(pos) if pos > 0 => {
                            self.history_pos = Some(pos - 1);
                            self.input = self.history.get(pos - 1).cloned().unwrap_or_default();
                            self.cursor = self.input.len();
                        }
                        _ => {}
                    }
                }
            }

            // Down arrow — history next
            if i.key_pressed(Key::ArrowDown) {
                match self.history_pos {
                    Some(pos) => {
                        if pos + 1 < self.history.len() {
                            self.history_pos = Some(pos + 1);
                            self.input = self.history[pos + 1].clone();
                            self.cursor = self.input.len();
                        } else {
                            self.history_pos = None;
                            self.input = self.saved_input.clone();
                            self.cursor = self.input.len();
                        }
                    }
                    None => {}
                }
            }

            // Ctrl+L — clear screen
            if i.modifiers.ctrl && i.key_pressed(Key::L) {
                self.buffer.clear();
            }
        });
    }
}

impl eframe::App for SlowTermApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Poll for async output
        self.poll_output();

        // Handle keyboard input
        self.handle_input(ctx);

        // Request repaint while running
        if self.running {
            ctx.request_repaint();
        }

        let font = FontId::new(self.font_size, FontFamily::Monospace);
        let line_height = self.font_size * 1.4;

        // Menu bar
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            slowcore::theme::menu_bar(ui, |ui| {
                ui.menu_button("shell", |ui| {
                    if ui.button("clear  ⌃L").clicked() {
                        self.buffer.clear();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("increase font").clicked() {
                        self.font_size = (self.font_size + 1.0).min(24.0);
                        ui.close_menu();
                    }
                    if ui.button("decrease font").clicked() {
                        self.font_size = (self.font_size - 1.0).max(10.0);
                        ui.close_menu();
                    }
                });
                ui.menu_button("help", |ui| {
                    if ui.button("about slowTerm").clicked() {
                        self.show_about = true;
                        ui.close_menu();
                    }
                });
            });
        });

        // Status bar
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let status = if self.running {
                "running...  (⌃C to cancel)".to_string()
            } else {
                format!("{}", self.cwd.display())
            };
            slowcore::widgets::status_bar(ui, &status);
        });

        // Main terminal area
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(4.0)))
            .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();

                // Reserve space for the input line at the bottom
                let input_height = line_height + 8.0;
                let output_rect = Rect::from_min_max(
                    rect.min,
                    Pos2::new(rect.max.x, rect.max.y - input_height - 2.0),
                );
                let input_rect = Rect::from_min_max(
                    Pos2::new(rect.min.x, rect.max.y - input_height),
                    rect.max,
                );

                // --- Output area ---
                let visible_lines = (output_rect.height() / line_height) as usize;
                let total_lines = self.buffer.len();

                // Handle scrolling
                let response = ui.allocate_rect(output_rect, Sense::click_and_drag());
                if response.hovered() {
                    ui.input(|i| {
                        let scroll = i.raw_scroll_delta.y;
                        if scroll != 0.0 {
                            self.scroll_offset = (self.scroll_offset - scroll / line_height)
                                .max(0.0)
                                .min((total_lines as f32 - visible_lines as f32).max(0.0));
                            self.auto_scroll = false;
                        }
                    });
                }

                // Auto-scroll when new output arrives
                if self.auto_scroll {
                    self.scroll_offset = (total_lines as f32 - visible_lines as f32).max(0.0);
                }

                let painter = ui.painter_at(output_rect);
                painter.rect_filled(output_rect, 0.0, SlowColors::WHITE);

                let start_line = self.scroll_offset as usize;
                let end_line = (start_line + visible_lines + 1).min(total_lines);

                for (i, line_idx) in (start_line..end_line).enumerate() {
                    if let Some(line) = self.buffer.get(line_idx) {
                        let y = output_rect.min.y + i as f32 * line_height;
                        let color = match line.kind {
                            LineKind::Command => SlowColors::BLACK,
                            LineKind::Stdout => SlowColors::BLACK,
                            LineKind::Stderr => SlowColors::BLACK,
                            LineKind::System => SlowColors::BLACK,
                        };
                        // Prefix stderr lines with a marker
                        let text = match line.kind {
                            LineKind::Stderr => format!("! {}", line.text),
                            _ => line.text.clone(),
                        };
                        painter.text(
                            Pos2::new(output_rect.min.x + 4.0, y),
                            egui::Align2::LEFT_TOP,
                            &text,
                            font.clone(),
                            color,
                        );
                    }
                }

                // --- Separator ---
                let sep_y = input_rect.min.y - 1.0;
                painter.hline(
                    rect.min.x..=rect.max.x,
                    sep_y,
                    Stroke::new(1.0, SlowColors::BLACK),
                );

                // --- Input line ---
                let input_painter = ui.painter_at(input_rect);
                input_painter.rect_filled(input_rect, 0.0, SlowColors::WHITE);

                let prompt = self.prompt();
                let full_input = format!("{}{}", prompt, self.input);

                input_painter.text(
                    Pos2::new(input_rect.min.x + 4.0, input_rect.min.y + 2.0),
                    egui::Align2::LEFT_TOP,
                    &full_input,
                    font.clone(),
                    SlowColors::BLACK,
                );

                // Cursor — measure prompt + input up to cursor position
                let prefix = format!("{}{}", prompt, &self.input[..self.cursor]);
                let galley = input_painter.layout_no_wrap(prefix, font.clone(), SlowColors::BLACK);
                let cursor_x = input_rect.min.x + 4.0 + galley.rect.width();
                let cursor_y_top = input_rect.min.y + 2.0;
                let cursor_y_bot = cursor_y_top + line_height;
                input_painter.vline(
                    cursor_x,
                    cursor_y_top..=cursor_y_bot,
                    Stroke::new(1.0, SlowColors::BLACK),
                );

                // Keep focus
                ctx.memory_mut(|m| m.request_focus(response.id));
            });

        // About dialog
        if self.show_about {
            egui::Window::new("about slowTerm")
                .collapsible(false)
                .resizable(false)
                .default_width(300.0)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("slowTerm");
                        ui.label("version 0.1.0");
                        ui.add_space(8.0);
                        ui.label("terminal emulator for slowOS");
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label("features:");
                    ui.label("  shell command execution");
                    ui.label("  command history, autocomplete");
                    ui.label("  Ctrl+C interrupt support");
                    ui.add_space(4.0);
                    ui.label("frameworks:");
                    ui.label("  egui/eframe (MIT)");
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        if ui.button("ok").clicked() { self.show_about = false; }
                    });
                });
        }
    }
}

/// Get user home directory without the `directories` crate
fn dirs_home() -> Option<PathBuf> {
    env::var("HOME").ok().map(PathBuf::from)
}

/// Find the longest common prefix among a list of strings
fn common_prefix(strings: &[String]) -> String {
    if strings.is_empty() { return String::new(); }
    let first = &strings[0];
    let mut len = first.len();
    for s in &strings[1..] {
        len = len.min(s.len());
        for (i, (a, b)) in first.chars().zip(s.chars()).enumerate() {
            if a != b {
                len = len.min(i);
                break;
            }
        }
    }
    first[..len].to_string()
}
