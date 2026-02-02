//! SlowSlides - minimal presentation software
//! Edit slides as text, present them full-screen style.

use egui::{Context, FontId, Key, Rect, Stroke, Vec2};
use serde::{Deserialize, Serialize};
use slowcore::storage::{documents_dir, FileBrowser};
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::status_bar;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Slide {
    title: String,
    body: String,
}

impl Default for Slide {
    fn default() -> Self {
        Self { title: "new slide".into(), body: String::new() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Deck {
    title: String,
    slides: Vec<Slide>,
    #[serde(skip)]
    path: Option<PathBuf>,
    #[serde(skip)]
    modified: bool,
}

impl Default for Deck {
    fn default() -> Self {
        Self {
            title: "untitled presentation".into(),
            slides: vec![Slide { title: "title slide".into(), body: "your presentation starts here.".into() }],
            path: None,
            modified: false,
        }
    }
}

impl Deck {
    fn save(&mut self, path: &PathBuf) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())?;
        self.path = Some(path.clone());
        self.modified = false;
        Ok(())
    }

    fn open(path: PathBuf) -> Result<Self, String> {
        let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let mut deck: Deck = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        deck.path = Some(path);
        deck.modified = false;
        Ok(deck)
    }
}

#[derive(PartialEq)]
enum Mode { Edit, Present }

pub struct SlowSlidesApp {
    deck: Deck,
    current_slide: usize,
    mode: Mode,
    show_file_browser: bool,
    file_browser: FileBrowser,
    fb_mode: FbMode,
    save_filename: String,
    show_about: bool,
}

#[derive(PartialEq)]
enum FbMode { Open, Save }

impl SlowSlidesApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            deck: Deck::default(),
            current_slide: 0,
            mode: Mode::Edit,
            show_file_browser: false,
            file_browser: FileBrowser::new(documents_dir())
                .with_filter(vec!["json".into()]),
            fb_mode: FbMode::Open,
            save_filename: String::new(),
            show_about: false,
        }
    }

    fn add_slide(&mut self) {
        let idx = self.current_slide + 1;
        self.deck.slides.insert(idx, Slide::default());
        self.current_slide = idx;
        self.deck.modified = true;
    }

    fn delete_slide(&mut self) {
        if self.deck.slides.len() > 1 {
            self.deck.slides.remove(self.current_slide);
            if self.current_slide >= self.deck.slides.len() {
                self.current_slide = self.deck.slides.len() - 1;
            }
            self.deck.modified = true;
        }
    }

    fn move_slide_up(&mut self) {
        if self.current_slide > 0 {
            self.deck.slides.swap(self.current_slide, self.current_slide - 1);
            self.current_slide -= 1;
            self.deck.modified = true;
        }
    }

    fn move_slide_down(&mut self) {
        if self.current_slide < self.deck.slides.len() - 1 {
            self.deck.slides.swap(self.current_slide, self.current_slide + 1);
            self.current_slide += 1;
            self.deck.modified = true;
        }
    }

    fn save(&mut self) {
        if let Some(path) = self.deck.path.clone() {
            let _ = self.deck.save(&path);
        } else {
            self.fb_mode = FbMode::Save;
            self.save_filename = "presentation.json".into();
            self.show_file_browser = true;
        }
    }

    fn handle_keys(&mut self, ctx: &Context) {
        // Consume Tab to prevent menu hover
        ctx.input_mut(|i| {
            if i.key_pressed(Key::Tab) {
                i.events.retain(|e| !matches!(e, egui::Event::Key { key: Key::Tab, .. }));
            }
        });
        ctx.input(|i| {
            let cmd = i.modifiers.command;

            if cmd && i.key_pressed(Key::S) { self.save(); }
            if cmd && i.key_pressed(Key::O) {
                self.fb_mode = FbMode::Open;
                self.show_file_browser = true;
            }

            if self.mode == Mode::Present {
                if i.key_pressed(Key::Escape) { self.mode = Mode::Edit; }
                if i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::Space) || i.key_pressed(Key::N) {
                    if self.current_slide < self.deck.slides.len() - 1 { self.current_slide += 1; }
                }
                if i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::P) {
                    if self.current_slide > 0 { self.current_slide -= 1; }
                }
            } else {
                if i.key_pressed(Key::F5) || (cmd && i.key_pressed(Key::Enter)) {
                    self.mode = Mode::Present;
                }
            }
        });
    }

    fn render_slide_list(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            for (idx, slide) in self.deck.slides.iter().enumerate() {
                let current = idx == self.current_slide;
                let label = format!("{}. {}", idx + 1, slide.title);
                if ui.selectable_label(current, &label).clicked() {
                    self.current_slide = idx;
                }
            }
            ui.add_space(10.0);
            if ui.button("+ Add Slide").clicked() { self.add_slide(); }
        });
    }

    fn render_editor(&mut self, ui: &mut egui::Ui) {
        let slide = &mut self.deck.slides[self.current_slide];

        ui.horizontal(|ui| {
            ui.label("title:");
            if ui.text_edit_singleline(&mut slide.title).changed() { self.deck.modified = true; }
        });
        ui.separator();
        ui.label("content (one line per bullet point):");
        let available = ui.available_size();
        if ui.add_sized(
            available,
            egui::TextEdit::multiline(&mut slide.body)
                .font(egui::FontId::proportional(14.0))
                .desired_width(available.x)
        ).changed() {
            self.deck.modified = true;
        }
    }

    fn render_preview(&self, ui: &mut egui::Ui) {
        let slide = &self.deck.slides[self.current_slide];
        let rect = ui.available_rect_before_wrap();

        // 4:3 aspect ratio preview
        let preview_w = rect.width().min(400.0);
        let preview_h = preview_w * 0.75;
        let preview_rect = Rect::from_min_size(
            egui::pos2(rect.center().x - preview_w / 2.0, rect.min.y),
            Vec2::new(preview_w, preview_h),
        );

        let _response = ui.allocate_rect(preview_rect, egui::Sense::hover());
        let painter = ui.painter_at(preview_rect);

        render_slide(&painter, slide, preview_rect);
    }

    fn render_present(&self, ui: &mut egui::Ui) {
        let slide = &self.deck.slides[self.current_slide];
        let rect = ui.available_rect_before_wrap();
        let _response = ui.allocate_rect(rect, egui::Sense::click());
        let painter = ui.painter_at(rect);

        render_slide(&painter, slide, rect);

        // Slide counter
        painter.text(
            egui::pos2(rect.max.x - 20.0, rect.max.y - 20.0),
            egui::Align2::RIGHT_BOTTOM,
            format!("{} / {}", self.current_slide + 1, self.deck.slides.len()),
            egui::FontId::proportional(12.0),
            SlowColors::BLACK,
        );
    }

    fn render_file_browser(&mut self, ctx: &Context) {
        let title = if self.fb_mode == FbMode::Open { "open deck" } else { "save deck" };
        egui::Window::new(title).collapsible(false).default_width(400.0).show(ctx, |ui| {
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
                        else if self.fb_mode == FbMode::Open {
                            if let Ok(deck) = Deck::open(entry.path.clone()) {
                                self.deck = deck;
                                self.current_slide = 0;
                            }
                            self.show_file_browser = false;
                        }
                    }
                }
            });
            if self.fb_mode == FbMode::Save {
                ui.separator();
                ui.horizontal(|ui| { ui.label("filename:"); ui.text_edit_singleline(&mut self.save_filename); });
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("cancel").clicked() { self.show_file_browser = false; }
                if ui.button(if self.fb_mode == FbMode::Open { "open" } else { "save" }).clicked() {
                    match self.fb_mode {
                        FbMode::Open => {
                            if let Some(e) = self.file_browser.selected_entry() {
                                if !e.is_directory {
                                    if let Ok(deck) = Deck::open(e.path.clone()) {
                                        self.deck = deck;
                                        self.current_slide = 0;
                                    }
                                    self.show_file_browser = false;
                                }
                            }
                        }
                        FbMode::Save => {
                            if !self.save_filename.is_empty() {
                                let p = self.file_browser.current_dir.join(&self.save_filename);
                                let _ = self.deck.save(&p);
                                self.show_file_browser = false;
                            }
                        }
                    }
                }
            });
        });
    }
}

fn render_slide(painter: &egui::Painter, slide: &Slide, rect: Rect) {
    painter.rect_filled(rect, 0.0, SlowColors::WHITE);
    painter.rect_stroke(rect, 0.0, Stroke::new(2.0, SlowColors::BLACK));

    let margin = rect.width() * 0.08;
    let title_size = (rect.width() * 0.05).clamp(18.0, 48.0);
    let body_size = (rect.width() * 0.03).clamp(12.0, 28.0);

    // Title
    painter.text(
        egui::pos2(rect.min.x + margin, rect.min.y + margin + title_size),
        egui::Align2::LEFT_BOTTOM,
        &slide.title,
        FontId::proportional(title_size),
        SlowColors::BLACK,
    );

    // Divider
    let div_y = rect.min.y + margin + title_size + 15.0;
    painter.hline(
        (rect.min.x + margin)..=(rect.max.x - margin),
        div_y,
        Stroke::new(2.0, SlowColors::BLACK),
    );

    // Body lines as bullet points
    let mut y = div_y + 25.0;
    for line in slide.body.lines() {
        let line = line.trim();
        if line.is_empty() { y += body_size * 0.5; continue; }

        let text = if line.starts_with("- ") || line.starts_with("* ") {
            format!("• {}", &line[2..])
        } else {
            line.to_string()
        };

        if y + body_size < rect.max.y - margin {
            painter.text(
                egui::pos2(rect.min.x + margin, y),
                egui::Align2::LEFT_TOP,
                &text,
                FontId::proportional(body_size),
                SlowColors::BLACK,
            );
        }
        y += body_size * 1.5;
    }
}

impl eframe::App for SlowSlidesApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.handle_keys(ctx);

        if self.mode == Mode::Present {
            egui::CentralPanel::default().frame(egui::Frame::none().fill(SlowColors::WHITE))
                .show(ctx, |ui| self.render_present(ui));
            return;
        }

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            menu_bar(ui, |ui| {
                ui.menu_button("file", |ui| {
                    if ui.button("new").clicked() { self.deck = Deck::default(); self.current_slide = 0; ui.close_menu(); }
                    if ui.button("Open... ⌘O").clicked() { self.fb_mode = FbMode::Open; self.show_file_browser = true; ui.close_menu(); }
                    ui.separator();
                    if ui.button("Save    ⌘S").clicked() { self.save(); ui.close_menu(); }
                    if ui.button("save as...").clicked() {
                        self.fb_mode = FbMode::Save; self.save_filename = "presentation.json".into();
                        self.show_file_browser = true; ui.close_menu();
                    }
                });
                ui.menu_button("slide", |ui| {
                    if ui.button("add slide").clicked() { self.add_slide(); ui.close_menu(); }
                    if ui.button("delete slide").clicked() { self.delete_slide(); ui.close_menu(); }
                    ui.separator();
                    if ui.button("move up").clicked() { self.move_slide_up(); ui.close_menu(); }
                    if ui.button("move down").clicked() { self.move_slide_down(); ui.close_menu(); }
                });
                ui.menu_button("present", |ui| {
                    if ui.button("start  f5").clicked() { self.mode = Mode::Present; ui.close_menu(); }
                });
                ui.menu_button("help", |ui| {
                    if ui.button("about").clicked() { self.show_about = true; ui.close_menu(); }
                });
            });
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let m = if self.deck.modified { "*" } else { "" };
            status_bar(ui, &format!(
                "{}{}  |  Slide {} of {}  |  F5 to present",
                self.deck.title, m, self.current_slide + 1, self.deck.slides.len()
            ));
        });

        egui::SidePanel::left("slides").default_width(180.0).show(ctx, |ui| self.render_slide_list(ui));

        egui::TopBottomPanel::bottom("preview").min_height(250.0).show(ctx, |ui| {
            ui.label("preview:");
            self.render_preview(ui);
        });

        egui::CentralPanel::default().frame(
            egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(8.0))
        ).show(ctx, |ui| self.render_editor(ui));

        if self.show_file_browser { self.render_file_browser(ctx); }
        if self.show_about {
            egui::Window::new("about slowSlides")
                .collapsible(false)
                .resizable(false)
                .default_width(300.0)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("slowSlides");
                        ui.label("version 0.1.0");
                        ui.add_space(8.0);
                        ui.label("presentation tool for e-ink");
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label("supported formats:");
                    ui.label("  Markdown (.md)");
                    ui.add_space(4.0);
                    ui.label("features:");
                    ui.label("  slide separators (---)");
                    ui.label("  fullscreen presentation mode");
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
