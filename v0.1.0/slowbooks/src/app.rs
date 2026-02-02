//! SlowRead application

use crate::book::Book;
use crate::library::Library;
use crate::reader::Reader;
use egui::{Context, Key};
use slowcore::storage::{documents_dir, FileBrowser};
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::status_bar;
use std::path::PathBuf;

/// Application view
#[derive(Clone, Copy, PartialEq)]
enum View {
    Library,
    Reader,
}

pub struct SlowBooksApp {
    view: View,
    library: Library,
    current_book: Option<Book>,
    reader: Reader,
    show_file_browser: bool,
    file_browser: FileBrowser,
    show_toc: bool,
    show_settings: bool,
    show_about: bool,
}

impl SlowBooksApp {
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
        }
    }
    
    fn open_book(&mut self, path: PathBuf) {
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
        // Consume Tab key
        ctx.input_mut(|i| {
            if i.key_pressed(Key::Tab) {
                i.events.retain(|e| !matches!(e, egui::Event::Key { key: Key::Tab, .. }));
            }
        });
        
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
                if i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::Space) || i.key_pressed(Key::PageDown) {
                    self.reader.next_page(book);
                }
                if i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::PageUp) {
                    self.reader.prev_page(book);
                }
                if shift && i.key_pressed(Key::Space) {
                    self.reader.prev_page(book);
                }

                // N/P for chapter navigation
                if i.key_pressed(Key::N) {
                    self.reader.next_chapter(book);
                }
                if i.key_pressed(Key::P) {
                    self.reader.prev_chapter(book);
                }
                if cmd && i.key_pressed(Key::Equals) {
                    self.reader.increase_font_size();
                }
                if cmd && i.key_pressed(Key::Minus) {
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
                    if ui.button("increase font  ⌘+").clicked() {
                        self.reader.increase_font_size();
                        ui.close_menu();
                    }
                    if ui.button("decrease font  ⌘-").clicked() {
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
                if ui.button("about slowBooks").clicked() {
                    self.show_about = true;
                    ui.close_menu();
                }
            });
        });
    }
    
    fn render_library(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.heading("slowBooks library");
            ui.add_space(10.0);
            
            if ui.button("open book...").clicked() {
                self.show_file_browser = true;
            }
            
            ui.add_space(20.0);
        });
        
        ui.separator();
        
        // Recent books
        if !self.library.books.is_empty() {
            ui.label("recent books:");
            ui.add_space(10.0);
            
            // Collect book info first to avoid borrow issues
            let book_info: Vec<(PathBuf, String, String)> = self.library.recent_books()
                .iter()
                .map(|entry| {
                    let title = if entry.metadata.title.is_empty() {
                        entry.path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "unknown".to_string())
                    } else {
                        entry.metadata.title.clone()
                    };
                    
                    let author = if entry.metadata.author.is_empty() {
                        String::new()
                    } else {
                        format!(" by {}", entry.metadata.author)
                    };
                    
                    (entry.path.clone(), title, author)
                })
                .collect();
            
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (path, title, author) in book_info {
                    ui.horizontal(|ui| {
                        if ui.button(format!("📖 {}{}", title, author)).clicked() {
                            self.open_book(path);
                        }
                    });
                }
            });
        } else {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label("no books in library yet.");
                ui.label("click 'open book...' or drag an epub file to add one.");
            });
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
            .default_width(400.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Location:");
                    ui.label(self.file_browser.current_dir.to_string_lossy().to_string());
                });
                
                ui.separator();
                
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
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
        egui::Window::new("about slowBooks")
            .collapsible(false)
            .resizable(false)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("slowBooks");
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

impl eframe::App for SlowBooksApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.handle_keyboard(ctx);
        
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
                format!("{} books in library", self.library.books.len())
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
}
