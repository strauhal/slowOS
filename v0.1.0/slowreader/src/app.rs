//! SlowRead application

use crate::book::Book;
use crate::library::Library;
use crate::reader::Reader;
use egui::{Context, Key, Rect, Sense, Stroke, Vec2};
use slowcore::storage::{documents_dir, FileBrowser};
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::status_bar;
use std::path::PathBuf;

/// Path to the slowLibrary folder with pre-installed ebooks
fn slow_library_dir() -> PathBuf {
    // Look for slowLibrary in parent directories
    let mut path = std::env::current_exe().unwrap_or_default();
    for _ in 0..5 {
        path = path.parent().unwrap_or(&path).to_path_buf();
        let lib_path = path.join("slowLibrary");
        if lib_path.exists() {
            return lib_path;
        }
    }
    // Fallback to home directory
    dirs_home().unwrap_or_default().join("slowLibrary")
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

/// Scan slowLibrary folder for epub files
fn scan_slow_library() -> Vec<(PathBuf, String)> {
    let lib_dir = slow_library_dir();
    let mut books = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&lib_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("epub") {
                let name = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .replace('_', " ");
                // Capitalize words
                let name: String = name.split_whitespace()
                    .map(|w| {
                        let mut chars = w.chars();
                        match chars.next() {
                            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                            None => String::new(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                books.push((path, name));
            }
        }
    }
    books.sort_by(|a, b| a.1.cmp(&b.1));
    books
}

/// Application view
#[derive(Clone, Copy, PartialEq)]
enum View {
    Library,
    Reader,
}

pub struct SlowReaderApp {
    view: View,
    library: Library,
    current_book: Option<Book>,
    reader: Reader,
    show_file_browser: bool,
    file_browser: FileBrowser,
    show_toc: bool,
    show_settings: bool,
    show_about: bool,
    /// Cached list of books from slowLibrary folder
    slow_library_books: Vec<(PathBuf, String)>,
}

impl SlowReaderApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            view: View::Library,
            library: Library::load(),
            current_book: None,
            reader: Reader::new(),
            show_file_browser: false,
            file_browser: FileBrowser::new(documents_dir())
                .with_filter(vec!["epub".into(), "txt".into()]),
            show_toc: false,
            show_settings: false,
            show_about: false,
            slow_library_books: scan_slow_library(),
        }
    }
    
    pub fn open_book(&mut self, path: PathBuf) {
        let result = if path.extension().map(|e| e == "epub").unwrap_or(false) {
            Book::open_epub(path.clone())
        } else {
            Book::open_text(path.clone())
        };
        
        match result {
            Ok(book) => {
                // Restore position if we have one
                if let Some((chapter, page)) = self.library.get_position(&path) {
                    self.reader.position.chapter = chapter;
                    self.reader.position.page = page as usize;
                } else {
                    self.reader.position.chapter = 0;
                    self.reader.position.page = 0;
                }
                
                // Add to library
                self.library.add_book(path, book.metadata.clone());
                
                self.current_book = Some(book);
                self.view = View::Reader;
            }
            Err(_e) => {
                eprintln!("Failed to open book");
            }
        }
    }
    
    fn close_book(&mut self) {
        // Save position
        if let Some(ref book) = self.current_book {
            self.library.update_position(
                &book.path,
                self.reader.position.chapter,
                self.reader.position.page as f32,
            );
        }

        self.current_book = None;
        self.view = View::Library;
    }
    
    fn handle_keyboard(&mut self, ctx: &Context) {
        slowcore::theme::consume_special_keys(ctx);

        // Handle dropped files (drag-and-drop epub)
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter()
                .filter_map(|f| f.path.clone())
                .filter(|p| {
                    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                    ext == "epub" || ext == "txt"
                })
                .collect()
        });
        if let Some(path) = dropped.into_iter().next() {
            self.open_book(path);
        }
        
        ctx.input(|i| {
            let cmd = i.modifiers.command;
            let shift = i.modifiers.shift;
            
            // Global shortcuts
            if cmd && i.key_pressed(Key::O) {
                self.show_file_browser = true;
            }
            if cmd && i.key_pressed(Key::W) && self.current_book.is_some() {
                self.close_book();
            }
            
            // Reader shortcuts - horizontal page flipping only
            if self.view == View::Reader && self.current_book.is_some() {
                let book = self.current_book.as_ref().unwrap();

                // Page navigation - all directions flip pages
                // Shift+Space goes back, Space alone goes forward
                let space_pressed = i.key_pressed(Key::Space);
                if shift && space_pressed {
                    self.reader.prev_page(book);
                } else if i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::PageDown) {
                    self.reader.next_page(book);
                } else if space_pressed && !shift {
                    self.reader.next_page(book);
                }
                if i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::PageUp) {
                    self.reader.prev_page(book);
                }

                // N/P for chapter navigation
                if i.key_pressed(Key::N) {
                    self.reader.next_chapter(book);
                }
                if i.key_pressed(Key::P) {
                    self.reader.prev_chapter(book);
                }
                if i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals) {
                    self.reader.increase_font_size();
                }
                if i.key_pressed(Key::Minus) {
                    self.reader.decrease_font_size();
                }
                if i.key_pressed(Key::T) {
                    self.show_toc = !self.show_toc;
                }
                if i.key_pressed(Key::Escape) {
                    self.close_book();
                }
            }
        });
    }
    
    fn render_menu_bar(&mut self, ui: &mut egui::Ui) {
        menu_bar(ui, |ui| {
            ui.menu_button("file", |ui| {
                if ui.button("open...     ⌘o").clicked() {
                    self.show_file_browser = true;
                    ui.close_menu();
                }
                if self.current_book.is_some() {
                    if ui.button("close book  ⌘W").clicked() {
                        self.close_book();
                        ui.close_menu();
                    }
                }
                ui.separator();
                if ui.button("library").clicked() {
                    self.view = View::Library;
                    ui.close_menu();
                }
            });
            
            if self.current_book.is_some() {
                ui.menu_button("view", |ui| {
                    if ui.button("table of contents  t").clicked() {
                        self.show_toc = !self.show_toc;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("increase font  +").clicked() {
                        self.reader.increase_font_size();
                        ui.close_menu();
                    }
                    if ui.button("decrease font  -").clicked() {
                        self.reader.decrease_font_size();
                        ui.close_menu();
                    }
                    if ui.button("settings...").clicked() {
                        self.show_settings = true;
                        ui.close_menu();
                    }
                });
                
                ui.menu_button("go", |ui| {
                    if ui.button("next page        →").clicked() {
                        if let Some(ref book) = self.current_book {
                            self.reader.next_page(book);
                        }
                        ui.close_menu();
                    }
                    if ui.button("previous page    ←").clicked() {
                        if let Some(ref book) = self.current_book {
                            self.reader.prev_page(book);
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("next chapter     n").clicked() {
                        if let Some(ref book) = self.current_book {
                            self.reader.next_chapter(book);
                        }
                        ui.close_menu();
                    }
                    if ui.button("previous chapter p").clicked() {
                        if let Some(ref book) = self.current_book {
                            self.reader.prev_chapter(book);
                        }
                        ui.close_menu();
                    }
                });
            }
            
            ui.menu_button("help", |ui| {
                if ui.button("about slowReader").clicked() {
                    self.show_about = true;
                    ui.close_menu();
                }
            });
        });
    }
    
    fn render_library(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(10.0);
            ui.heading("slowReader");
            ui.add_space(5.0);

            if ui.button("open book...").clicked() {
                self.show_file_browser = true;
            }

            ui.add_space(10.0);
        });

        ui.separator();

        // Collect user books (from recent/opened)
        let mut user_books: Vec<(PathBuf, String)> = Vec::new();
        for entry in self.library.recent_books() {
            let title = if entry.metadata.title.is_empty() {
                entry.path.file_stem()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            } else {
                entry.metadata.title.clone()
            };
            // Don't include slowLibrary books in user section
            let is_slow_lib = self.slow_library_books.iter().any(|(p, _)| p == &entry.path);
            if !is_slow_lib {
                user_books.push((entry.path.clone(), title));
            }
        }

        let library_books: Vec<(PathBuf, String)> = self.slow_library_books.clone();

        let mut book_to_open: Option<PathBuf> = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            // --- User Library section ---
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(egui::RichText::new("user library").strong());
            });
            ui.add_space(4.0);

            if user_books.is_empty() {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label("want to grow your book collection?");
                    ui.add_space(4.0);
                    ui.label("ePub files can be bought at ebooks.com");
                });
                ui.add_space(20.0);
            } else {
                Self::render_book_grid(ui, &user_books, &mut book_to_open);
            }

            ui.add_space(8.0);
            ui.separator();

            // --- slowLibrary section ---
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                ui.label(egui::RichText::new("slowLibrary").strong());
            });
            ui.add_space(4.0);

            if library_books.is_empty() {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label("no public domain books found.");
                });
                ui.add_space(20.0);
            } else {
                Self::render_book_grid(ui, &library_books, &mut book_to_open);
            }
        });

        // Open book after the loop to avoid borrow issues
        if let Some(path) = book_to_open {
            self.open_book(path);
        }
    }

    fn render_book_grid(ui: &mut egui::Ui, books: &[(PathBuf, String)], book_to_open: &mut Option<PathBuf>) {
        let available_width = ui.available_width();
        let book_width: f32 = 100.0;
        let book_height: f32 = 140.0;
        let padding: f32 = 10.0;
        let cols = ((available_width - padding) / (book_width + padding)).max(1.0) as usize;

        let rows = (books.len() + cols - 1) / cols;
        for row in 0..rows {
            ui.horizontal(|ui| {
                ui.add_space(padding);
                for col in 0..cols {
                    let idx = row * cols + col;
                    if idx >= books.len() {
                        break;
                    }

                    let (path, title) = &books[idx];

                    // Draw book cover placeholder
                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(book_width, book_height),
                        Sense::click(),
                    );

                    if ui.is_rect_visible(rect) {
                        let painter = ui.painter();

                        // Book background
                        painter.rect_filled(rect, 2.0, SlowColors::WHITE);
                        painter.rect_stroke(rect, 2.0, Stroke::new(2.0, SlowColors::BLACK));

                        // Hover/selection effect
                        if response.hovered() {
                            slowcore::dither::draw_dither_hover(painter, rect);
                        }

                        // Book spine decoration
                        let spine_rect = Rect::from_min_size(
                            rect.min,
                            Vec2::new(8.0, rect.height()),
                        );
                        painter.rect_filled(spine_rect, 0.0, SlowColors::BLACK);

                        // Title text (wrapped)
                        let title_rect = Rect::from_min_max(
                            egui::pos2(rect.min.x + 12.0, rect.min.y + 10.0),
                            egui::pos2(rect.max.x - 4.0, rect.max.y - 10.0),
                        );

                        // Simple word wrap for title
                        let words: Vec<&str> = title.split_whitespace().collect();
                        let mut lines: Vec<String> = Vec::new();
                        let mut current_line = String::new();
                        let max_chars_per_line = 10;

                        for word in words {
                            if current_line.len() + word.len() + 1 > max_chars_per_line && !current_line.is_empty() {
                                lines.push(current_line);
                                current_line = word.to_string();
                            } else {
                                if !current_line.is_empty() {
                                    current_line.push(' ');
                                }
                                current_line.push_str(word);
                            }
                        }
                        if !current_line.is_empty() {
                            lines.push(current_line);
                        }

                        // Draw title lines
                        for (i, line) in lines.iter().take(5).enumerate() {
                            painter.text(
                                egui::pos2(title_rect.min.x, title_rect.min.y + i as f32 * 14.0),
                                egui::Align2::LEFT_TOP,
                                line,
                                egui::FontId::proportional(11.0),
                                SlowColors::BLACK,
                            );
                        }
                    }

                    if response.clicked() {
                        *book_to_open = Some(path.clone());
                    }

                    ui.add_space(padding);
                }
            });
            ui.add_space(padding);
        }
    }
    
    fn render_reader(&mut self, ui: &mut egui::Ui) {
        if let Some(ref book) = self.current_book {
            let rect = ui.available_rect_before_wrap();
            self.reader.render(ui, book, rect);
        }
    }
    
    fn render_toc(&mut self, ctx: &Context) {
        if let Some(ref book) = self.current_book {
            egui::Window::new("table of contents")
                .collapsible(false)
                .resizable(true)
                .default_width(300.0)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (idx, chapter) in book.chapters.iter().enumerate() {
                            let current = idx == self.reader.position.chapter;
                            let title = if chapter.title.is_empty() {
                                format!("chapter {}", idx + 1)
                            } else {
                                chapter.title.clone()
                            };
                            
                            let response = ui.selectable_label(current, &title);
                            if response.clicked() {
                                self.reader.go_to_chapter(idx, book);
                                self.show_toc = false;
                            }
                        }
                    });
                    
                    ui.separator();
                    if ui.button("Close").clicked() {
                        self.show_toc = false;
                    }
                });
        }
    }
    
    fn render_file_browser(&mut self, ctx: &Context) {
        egui::Window::new("open book")
            .collapsible(false)
            .resizable(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Location:");
                    ui.label(self.file_browser.current_dir.to_string_lossy().to_string());
                });

                ui.separator();

                egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                    let entries = self.file_browser.entries.clone();
                    for (idx, entry) in entries.iter().enumerate() {
                        let selected = self.file_browser.selected_index == Some(idx);
                        let response = ui.add(
                            slowcore::widgets::FileListItem::new(&entry.name, entry.is_directory)
                                .selected(selected)
                        );
                        
                        if response.clicked() {
                            self.file_browser.selected_index = Some(idx);
                        }
                        
                        if response.double_clicked() {
                            if entry.is_directory {
                                self.file_browser.navigate_to(entry.path.clone());
                            } else {
                                self.open_book(entry.path.clone());
                                self.show_file_browser = false;
                            }
                        }
                    }
                });
                
                ui.separator();
                
                ui.horizontal(|ui| {
                    if ui.button("cancel").clicked() {
                        self.show_file_browser = false;
                    }
                    if ui.button("open").clicked() {
                        if let Some(entry) = self.file_browser.selected_entry() {
                            if !entry.is_directory {
                                let path = entry.path.clone();
                                self.open_book(path);
                                self.show_file_browser = false;
                            }
                        }
                    }
                });
            });
    }
    
    fn render_settings(&mut self, ctx: &Context) {
        egui::Window::new("reading settings")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("font size:");
                    ui.add(egui::Slider::new(&mut self.reader.settings.font_size, 12.0..=32.0));
                });
                
                ui.horizontal(|ui| {
                    ui.label("line height:");
                    ui.add(egui::Slider::new(&mut self.reader.settings.line_height, 1.0..=2.5));
                });
                
                ui.horizontal(|ui| {
                    ui.label("margin:");
                    ui.add(egui::Slider::new(&mut self.reader.settings.margin, 10.0..=100.0));
                });
                
                ui.separator();
                
                if ui.button("close").clicked() {
                    self.show_settings = false;
                }
            });
    }
    
    fn render_about(&mut self, ctx: &Context) {
        egui::Window::new("about slowReader")
            .collapsible(false)
            .resizable(false)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("slowReader");
                    ui.label("version 0.1.0");
                    ui.add_space(8.0);
                    ui.label("ebook reader for slowOS");
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.label("supported formats:");
                ui.label("  EPUB (.epub)");
                ui.add_space(4.0);
                ui.label("features:");
                ui.label("  chapter navigation, bookmarks");
                ui.label("  CJK font support");
                ui.add_space(4.0);
                ui.label("frameworks:");
                ui.label("  egui/eframe (MIT), epub-rs (MIT)");
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    if ui.button("ok").clicked() {
                        self.show_about = false;
                    }
                });
            });
    }
}

impl eframe::App for SlowReaderApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.handle_keyboard(ctx);

        // Auto-save position periodically when reading
        if self.view == View::Reader {
            if let Some(ref book) = self.current_book {
                self.library.update_position(
                    &book.path,
                    self.reader.position.chapter,
                    self.reader.position.page as f32,
                );
            }
        }

        // Menu bar
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            self.render_menu_bar(ui);
        });

        // Title bar (only in reader mode)
        if self.view == View::Reader {
            if let Some(ref book) = self.current_book {
                egui::TopBottomPanel::top("title").show(ctx, |ui| {
                    slowcore::theme::SlowTheme::title_bar_frame().show(ui, |ui| {
                        ui.centered_and_justified(|ui| {
                            ui.label(&book.metadata.title);
                        });
                    });
                });
            }
        }

        // Status bar
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let status = if self.view == View::Reader {
                if let Some(ref book) = self.current_book {
                    let (page, total) = self.reader.page_info();
                    format!(
                        "chapter {} of {}  |  page {} of {}  |  ←/→ or click to turn",
                        self.reader.position.chapter + 1,
                        book.chapter_count(),
                        page,
                        total
                    )
                } else {
                    String::new()
                }
            } else {
                format!("{} books in library", self.library.books.len() + self.slow_library_books.len())
            };
            status_bar(ui, &status);
        });

        // Main content
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE))
            .show(ctx, |ui| {
                match self.view {
                    View::Library => self.render_library(ui),
                    View::Reader => self.render_reader(ui),
                }
            });

        // Dialogs
        if self.show_file_browser {
            self.render_file_browser(ctx);
        }
        if self.show_toc {
            self.render_toc(ctx);
        }
        if self.show_settings {
            self.render_settings(ctx);
        }
        if self.show_about {
            self.render_about(ctx);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Save position on exit
        if let Some(ref book) = self.current_book {
            self.library.update_position(
                &book.path,
                self.reader.position.chapter,
                self.reader.position.page as f32,
            );
        }
    }
}
