//! SlowRead application

use crate::book::Book;
use crate::library::Library;
use crate::reader::Reader;
use egui::{Context, Key, Rect, Sense, Stroke, Vec2};
use slowcore::storage::{documents_dir, FileBrowser};
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::status_bar;
use std::collections::HashSet;
use std::path::PathBuf;

/// Path to the slowLibrary folder with pre-installed ebooks
fn slow_library_dir() -> PathBuf {
    // Look for slowLibrary in parent directories (for development)
    let mut path = std::env::current_exe().unwrap_or_default();
    for _ in 0..5 {
        path = path.parent().unwrap_or(&path).to_path_buf();
        // Check for slowLibrary directly (repo structure)
        let lib_path = path.join("slowLibrary");
        if lib_path.exists() {
            return lib_path;
        }
        // Also check Books/slowLibrary (installed structure)
        let books_path = path.join("Books").join("slowLibrary");
        if books_path.exists() {
            return books_path;
        }
    }
    // Fallback to home directory locations
    let home = dirs_home().unwrap_or_default();
    let home_lib = home.join("slowLibrary");
    if home_lib.exists() {
        return home_lib;
    }
    home.join("Books").join("slowLibrary")
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
    show_shortcuts: bool,
    /// Cached list of books from slowLibrary folder
    slow_library_books: Vec<(PathBuf, String)>,
    /// Show search bar
    show_search: bool,
    /// Search query
    search_query: String,
    /// Search results: list of (chapter_idx, page_idx, snippet)
    search_results: Vec<(usize, usize, String)>,
    /// Current search result index
    search_result_idx: usize,
    /// Fullscreen mode
    fullscreen: bool,
    /// Show menu bar temporarily in fullscreen when cursor near top
    fullscreen_menu_visible: bool,
    /// Selected books for deletion (only user books can be selected)
    selected_books: HashSet<PathBuf>,
    /// Delete mode - when true, show selection circles on user books
    delete_mode: bool,
}

impl SlowReaderApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let slow_library_books = scan_slow_library();

        Self {
            view: View::Library,
            library: Library::load(),
            current_book: None,
            reader: Reader::new(),
            show_file_browser: false,
            file_browser: FileBrowser::new(documents_dir())
                .with_filter(vec!["epub".into(), "txt".into(), "pdf".into()]),
            show_toc: false,
            show_settings: false,
            show_about: false,
            show_shortcuts: false,
            slow_library_books,
            show_search: false,
            search_query: String::new(),
            search_results: Vec::new(),
            search_result_idx: 0,
            fullscreen: false,
            fullscreen_menu_visible: false,
            selected_books: HashSet::new(),
            delete_mode: false,
        }
    }

    /// Delete selected books from the library
    fn delete_selected_books(&mut self) {
        for path in self.selected_books.drain() {
            // Remove from library
            self.library.books.retain(|b| b.path != path);
        }
        self.library.save();
    }
    
    /// Add a book to the library without opening it for reading
    fn add_book_to_library(&mut self, path: PathBuf) {
        // Skip if already in library
        if self.library.books.iter().any(|b| b.path == path) {
            return;
        }

        let result = if path.extension().map(|e| e == "epub").unwrap_or(false) {
            Book::open_epub(path.clone())
        } else {
            Book::open_text(path.clone())
        };

        if let Ok(book) = result {
            self.library.add_book(path, book.metadata.clone(), book.chapter_count());
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
                self.library.add_book(path, book.metadata.clone(), book.chapter_count());

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

        if !dropped.is_empty() {
            // Add all dropped books to library
            for path in dropped.iter() {
                self.add_book_to_library(path.clone());
            }
            // If only one book, open it for reading
            if dropped.len() == 1 {
                self.open_book(dropped.into_iter().next().unwrap());
            }
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
            // Ctrl+F / Cmd+F for search
            if cmd && i.key_pressed(Key::F) && self.current_book.is_some() {
                self.show_search = true;
                self.search_query.clear();
                self.search_results.clear();
                self.search_result_idx = 0;
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
                // F for fullscreen (without cmd, to not conflict with Cmd+F search)
                if i.key_pressed(Key::F) && !cmd {
                    self.fullscreen = !self.fullscreen;
                }
                if i.key_pressed(Key::Escape) {
                    if self.fullscreen {
                        self.fullscreen = false;
                    } else {
                        self.close_book();
                    }
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
                // Delete mode toggle (only when in library view)
                if self.view == View::Library {
                    ui.separator();
                    if self.delete_mode {
                        // Show delete action if books are selected
                        if !self.selected_books.is_empty() {
                            let count = self.selected_books.len();
                            let label = if count == 1 {
                                "delete book".to_string()
                            } else {
                                format!("delete {} books", count)
                            };
                            if ui.button(label).clicked() {
                                self.delete_selected_books();
                                self.delete_mode = false;
                                ui.close_menu();
                            }
                        }
                        if ui.button("cancel delete").clicked() {
                            self.delete_mode = false;
                            self.selected_books.clear();
                            ui.close_menu();
                        }
                    } else {
                        if ui.button("delete books...").clicked() {
                            self.delete_mode = true;
                            ui.close_menu();
                        }
                    }
                }
            });
            
            if self.current_book.is_some() {
                ui.menu_button("view", |ui| {
                    let fullscreen_label = if self.fullscreen { "exit fullscreen  F" } else { "fullscreen       F" };
                    if ui.button(fullscreen_label).clicked() {
                        self.fullscreen = !self.fullscreen;
                        ui.close_menu();
                    }
                    ui.separator();
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
                if ui.button("keyboard shortcuts").clicked() {
                    self.show_shortcuts = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("about").clicked() {
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

        // Collect user books (from recent/opened) with progress info
        // (path, title, progress_percent: Option<u8>)
        let slow_lib_dir = slow_library_dir();
        let slow_lib_paths: HashSet<&PathBuf> = self.slow_library_books.iter().map(|(p, _)| p).collect();
        let mut user_books: Vec<(PathBuf, String, Option<u8>)> = Vec::new();
        for entry in self.library.recent_books() {
            let title = if entry.metadata.title.is_empty() {
                entry.path.file_stem()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            } else {
                entry.metadata.title.clone()
            };
            // Don't include slowLibrary books in user section
            let is_slow_lib = slow_lib_paths.contains(&entry.path) || entry.path.starts_with(&slow_lib_dir);
            if !is_slow_lib {
                // Calculate progress percentage
                let progress = if entry.total_chapters > 0 {
                    let pct = ((entry.last_chapter + 1) as f32 / entry.total_chapters as f32 * 100.0) as u8;
                    Some(pct.min(100))
                } else {
                    None
                };
                user_books.push((entry.path.clone(), title, progress));
            }
        }

        // Collect library books with progress info, sorted by last read (recent first)
        let books_by_path: std::collections::HashMap<&PathBuf, &crate::library::LibraryEntry> = self.library.books.iter().map(|b| (&b.path, b)).collect();
        let mut library_books: Vec<(PathBuf, String, Option<u8>, u64)> = self.slow_library_books.iter().map(|(path, title)| {
            // Look up progress in library
            let (progress, last_read) = books_by_path.get(path)
                .map(|entry| {
                    let pct = if entry.total_chapters > 0 {
                        let p = ((entry.last_chapter + 1) as f32 / entry.total_chapters as f32 * 100.0) as u8;
                        Some(p.min(100))
                    } else {
                        None
                    };
                    (pct, entry.last_read)
                })
                .unwrap_or((None, 0));
            (path.clone(), title.clone(), progress, last_read)
        }).collect();
        // Sort: recently read books first, then unread books in original order
        library_books.sort_by(|a, b| b.3.cmp(&a.3));
        let library_books: Vec<(PathBuf, String, Option<u8>)> = library_books.into_iter()
            .map(|(p, t, prog, _)| (p, t, prog))
            .collect();

        let mut book_to_open: Option<PathBuf> = None;
        let mut toggle_selection: Option<PathBuf> = None;

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
                // User books can be selected only when in delete mode
                Self::render_book_grid(ui, &user_books, &mut book_to_open, &mut toggle_selection, &self.selected_books, self.delete_mode);
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
                // slowLibrary books cannot be selected for deletion
                Self::render_book_grid(ui, &library_books, &mut book_to_open, &mut None, &HashSet::new(), false);
            }
        });

        // Toggle selection after the loop
        if let Some(path) = toggle_selection {
            if self.selected_books.contains(&path) {
                self.selected_books.remove(&path);
            } else {
                self.selected_books.insert(path);
            }
        }

        // Open book after the loop to avoid borrow issues
        if let Some(path) = book_to_open {
            self.open_book(path);
        }
    }

    fn render_book_grid(
        ui: &mut egui::Ui,
        books: &[(PathBuf, String, Option<u8>)],
        book_to_open: &mut Option<PathBuf>,
        toggle_selection: &mut Option<PathBuf>,
        selected_books: &HashSet<PathBuf>,
        show_selection_circles: bool,
    ) {
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

                    let (path, title, progress) = &books[idx];

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

                        // Draw bookmark with reading progress (if available)
                        if let Some(pct) = progress {
                            // Bookmark shape on top-right corner
                            let bm_width = 22.0;
                            let bm_height = 32.0;
                            let bm_x = rect.max.x - bm_width - 2.0;
                            let bm_y = rect.min.y - 1.0; // Slightly above to drape over edge

                            // Draw bookmark ribbon shape (rectangle with pointed bottom)
                            let bm_points = [
                                egui::pos2(bm_x, bm_y),                           // top-left
                                egui::pos2(bm_x + bm_width, bm_y),                // top-right
                                egui::pos2(bm_x + bm_width, bm_y + bm_height),    // bottom-right
                                egui::pos2(bm_x + bm_width / 2.0, bm_y + bm_height - 6.0), // bottom-center (notch)
                                egui::pos2(bm_x, bm_y + bm_height),               // bottom-left
                            ];
                            painter.add(egui::Shape::convex_polygon(
                                bm_points.to_vec(),
                                SlowColors::BLACK,
                                Stroke::NONE,
                            ));

                            // Draw percentage text in white
                            let pct_text = format!("{}%", pct);
                            painter.text(
                                egui::pos2(bm_x + bm_width / 2.0, bm_y + bm_height / 2.0 - 2.0),
                                egui::Align2::CENTER_CENTER,
                                &pct_text,
                                egui::FontId::proportional(9.0),
                                SlowColors::WHITE,
                            );
                        }

                        // Draw selection circle in bottom-right corner (only for user books)
                        if show_selection_circles {
                            let circle_radius = 8.0;
                            let circle_center = egui::pos2(
                                rect.max.x - circle_radius - 4.0,
                                rect.max.y - circle_radius - 4.0,
                            );
                            let is_selected = selected_books.contains(path);

                            if is_selected {
                                // Filled circle for selected
                                painter.circle_filled(circle_center, circle_radius, SlowColors::BLACK);
                            } else {
                                // Empty circle for unselected
                                painter.circle_stroke(circle_center, circle_radius, Stroke::new(1.5, SlowColors::BLACK));
                            }
                        }
                    }

                    // Handle book clicks
                    if response.clicked() {
                        if show_selection_circles {
                            // In delete mode, clicking anywhere on the book toggles selection
                            *toggle_selection = Some(path.clone());
                        } else {
                            // Normal mode: open the book
                            *book_to_open = Some(path.clone());
                        }
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
            if self.fullscreen {
                self.reader.render_fullscreen(ui, book, rect);
            } else {
                self.reader.render(ui, book, rect);
            }
        }
    }
    
    fn render_toc(&mut self, ctx: &Context) {
        if let Some(ref book) = self.current_book {
            let resp = egui::Window::new("table of contents")
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
                    if ui.button("close").clicked() {
                        self.show_toc = false;
                    }
                });

            if let Some(r) = &resp {
                slowcore::dither::draw_window_shadow(ctx, r.response.rect);
            }

            // Click outside TOC window to dismiss
            if let Some(inner) = resp {
                let toc_rect = inner.response.rect;
                if ctx.input(|i| i.pointer.primary_clicked()) {
                    if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                        if !toc_rect.contains(pos) {
                            self.show_toc = false;
                        }
                    }
                }
            }
        }
    }
    
    fn render_file_browser(&mut self, ctx: &Context) {
        let resp = egui::Window::new("open book")
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
                    let mut clicked_idx = None;
                    let mut nav_path = None;
                    let mut open_path = None;
                    for (idx, entry) in self.file_browser.entries.iter().enumerate() {
                        let selected = self.file_browser.selected_index == Some(idx);
                        let response = ui.add(
                            slowcore::widgets::FileListItem::new(&entry.name, entry.is_directory)
                                .selected(selected)
                        );

                        if response.clicked() {
                            clicked_idx = Some(idx);
                        }

                        if response.double_clicked() {
                            if entry.is_directory {
                                nav_path = Some(entry.path.clone());
                            } else {
                                open_path = Some(entry.path.clone());
                            }
                        }
                    }
                    if let Some(idx) = clicked_idx { self.file_browser.selected_index = Some(idx); }
                    if let Some(path) = nav_path { self.file_browser.navigate_to(path); }
                    if let Some(path) = open_path {
                        self.open_book(path);
                        self.show_file_browser = false;
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
        if let Some(r) = &resp {
            slowcore::dither::draw_window_shadow(ctx, r.response.rect);
        }
    }
    
    fn render_settings(&mut self, ctx: &Context) {
        let resp = egui::Window::new("reading settings")
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
        if let Some(r) = &resp {
            slowcore::dither::draw_window_shadow(ctx, r.response.rect);
        }
    }

    fn render_about(&mut self, ctx: &Context) {
        let resp = egui::Window::new("about slowReader")
            .collapsible(false)
            .resizable(false)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("slowReader");
                    ui.label("version 0.2.0");
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
        if let Some(r) = &resp {
            slowcore::dither::draw_window_shadow(ctx, r.response.rect);
        }
    }

    /// Search the current book for a query string
    fn search_book(&mut self, query: &str) {
        self.search_results.clear();
        self.search_result_idx = 0;

        if query.is_empty() {
            return;
        }

        let query_lower = query.to_lowercase();

        if let Some(ref book) = self.current_book {
            for (chapter_idx, chapter) in book.chapters.iter().enumerate() {
                // Search through chapter content
                for block in &chapter.content {
                    let text = match block {
                        crate::book::ContentBlock::Paragraph(t) => t,
                        crate::book::ContentBlock::Heading { text, .. } => text,
                        crate::book::ContentBlock::Quote(t) => t,
                        crate::book::ContentBlock::Code(t) => t,
                        crate::book::ContentBlock::ListItem(t) => t,
                        _ => continue,
                    };

                    if text.to_lowercase().contains(&query_lower) {
                        // Extract a snippet around the match
                        let text_lower = text.to_lowercase();
                        if let Some(pos) = text_lower.find(&query_lower) {
                            let start = pos.saturating_sub(30);
                            let end = (pos + query.len() + 30).min(text.len());
                            let mut snippet = text[start..end].to_string();
                            if start > 0 {
                                snippet = format!("...{}", snippet);
                            }
                            if end < text.len() {
                                snippet = format!("{}...", snippet);
                            }
                            // Store chapter and page 0 (we'll navigate to chapter start)
                            self.search_results.push((chapter_idx, 0, snippet));
                        }
                    }
                }
            }
        }
    }

    fn render_search(&mut self, ctx: &Context) {
        let resp = egui::Window::new("find")
            .collapsible(false)
            .resizable(false)
            .default_width(350.0)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 60.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("find:");
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .desired_width(200.0)
                    );

                    // Auto-focus the text field
                    if !response.has_focus() {
                        response.request_focus();
                    }

                    // Search on Enter or when text changes
                    if response.changed() || ui.input(|i| i.key_pressed(Key::Enter)) {
                        let query = self.search_query.clone();
                        self.search_book(&query);
                    }

                    if ui.button("×").clicked() {
                        self.show_search = false;
                        self.search_query.clear();
                        self.search_results.clear();
                    }
                });

                if !self.search_results.is_empty() {
                    ui.add_space(8.0);
                    ui.label(format!("{} results", self.search_results.len()));

                    ui.add_space(4.0);
                    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        let mut go_to: Option<(usize, usize)> = None;
                        for (idx, (chapter_idx, page_idx, snippet)) in self.search_results.iter().enumerate() {
                            let is_current = idx == self.search_result_idx;
                            let label = format!("Ch.{}: {}", chapter_idx + 1, snippet);
                            if ui.selectable_label(is_current, &label).clicked() {
                                self.search_result_idx = idx;
                                go_to = Some((*chapter_idx, *page_idx));
                            }
                        }

                        if let Some((chapter_idx, _page_idx)) = go_to {
                            if let Some(ref book) = self.current_book {
                                self.reader.go_to_chapter(chapter_idx, book);
                                self.view = View::Reader;
                            }
                        }
                    });

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button("◀ prev").clicked() && !self.search_results.is_empty() {
                            if self.search_result_idx > 0 {
                                self.search_result_idx -= 1;
                            } else {
                                self.search_result_idx = self.search_results.len() - 1;
                            }
                            if let Some((chapter_idx, _, _)) = self.search_results.get(self.search_result_idx) {
                                if let Some(ref book) = self.current_book {
                                    self.reader.go_to_chapter(*chapter_idx, book);
                                }
                            }
                        }
                        if ui.button("next ▶").clicked() && !self.search_results.is_empty() {
                            self.search_result_idx = (self.search_result_idx + 1) % self.search_results.len();
                            if let Some((chapter_idx, _, _)) = self.search_results.get(self.search_result_idx) {
                                if let Some(ref book) = self.current_book {
                                    self.reader.go_to_chapter(*chapter_idx, book);
                                }
                            }
                        }
                    });
                } else if !self.search_query.is_empty() {
                    ui.add_space(8.0);
                    ui.label("no results found");
                }
            });
        if let Some(r) = &resp {
            slowcore::dither::draw_window_shadow(ctx, r.response.rect);
        }
    }

    fn render_shortcuts(&mut self, ctx: &Context) {
        let resp = egui::Window::new("keyboard shortcuts")
            .collapsible(false)
            .resizable(false)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.heading("reading");
                ui.add_space(4.0);
                egui::Grid::new("reading_shortcuts").show(ui, |ui| {
                    ui.label("→ or Space");
                    ui.label("next page");
                    ui.end_row();
                    ui.label("← or Shift+Space");
                    ui.label("previous page");
                    ui.end_row();
                    ui.label("N");
                    ui.label("next chapter");
                    ui.end_row();
                    ui.label("P");
                    ui.label("previous chapter");
                    ui.end_row();
                    ui.label("T");
                    ui.label("toggle table of contents");
                    ui.end_row();
                    ui.label("Escape");
                    ui.label("close book / return to library");
                    ui.end_row();
                });

                ui.add_space(12.0);
                ui.heading("font & display");
                ui.add_space(4.0);
                egui::Grid::new("font_shortcuts").show(ui, |ui| {
                    ui.label("+ / =");
                    ui.label("increase font size");
                    ui.end_row();
                    ui.label("-");
                    ui.label("decrease font size");
                    ui.end_row();
                });

                ui.add_space(12.0);
                ui.heading("file");
                ui.add_space(4.0);
                egui::Grid::new("file_shortcuts").show(ui, |ui| {
                    ui.label("⌘O");
                    ui.label("open book");
                    ui.end_row();
                    ui.label("⌘W");
                    ui.label("close book");
                    ui.end_row();
                    ui.label("⌘F");
                    ui.label("search in book");
                    ui.end_row();
                });

                ui.add_space(12.0);
                ui.separator();
                if ui.button("close").clicked() {
                    self.show_shortcuts = false;
                }
            });
        if let Some(r) = &resp {
            slowcore::dither::draw_window_shadow(ctx, r.response.rect);
        }
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

        // Menu bar: always visible in normal mode, hover-to-show in fullscreen
        if self.fullscreen {
            let near_top = ctx.input(|i| {
                i.pointer.hover_pos().map_or(false, |p| p.y < 40.0)
            });
            self.fullscreen_menu_visible = near_top;
        }
        if !self.fullscreen || self.fullscreen_menu_visible {
            egui::TopBottomPanel::top("menu").show(ctx, |ui| {
                self.render_menu_bar(ui);
            });
        }

        // Title bar (only in reader mode, hidden in fullscreen)
        if self.view == View::Reader && !self.fullscreen {
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

        // Status bar (hidden in fullscreen)
        if !self.fullscreen {
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
        }

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
        if self.show_shortcuts {
            self.render_shortcuts(ctx);
        }

        // Search dialog (Ctrl+F)
        if self.show_search {
            self.render_search(ctx);
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
