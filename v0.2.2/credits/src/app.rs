//! credits — open source credits and attributions for slowOS

use egui::{Context, ScrollArea};
use slowcore::repaint::RepaintController;
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::status_bar;

/// Credit category for organizing attributions
#[derive(Clone, Copy, PartialEq, Eq)]
enum Category {
    Overview,
    Building,
    LinuxFoundation,
    Fonts,
    Icons,
    Frameworks,
    Libraries,
    Tools,
}

impl Category {
    fn all() -> &'static [Category] {
        &[
            Category::Overview,
            Category::Building,
            Category::LinuxFoundation,
            Category::Fonts,
            Category::Icons,
            Category::Frameworks,
            Category::Libraries,
            Category::Tools,
        ]
    }

    fn name(&self) -> &'static str {
        match self {
            Category::Overview => "overview",
            Category::Building => "building slowOS",
            Category::LinuxFoundation => "linux foundation",
            Category::Fonts => "fonts",
            Category::Icons => "icons",
            Category::Frameworks => "frameworks",
            Category::Libraries => "libraries",
            Category::Tools => "build tools",
        }
    }
}

pub struct CreditsApp {
    selected_category: Category,
    show_about: bool,
    repaint: RepaintController,
}

impl CreditsApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            selected_category: Category::Overview,
            show_about: false,
            repaint: RepaintController::new(),
        }
    }

    fn handle_keys(&mut self, ctx: &Context) {
        slowcore::theme::consume_special_keys(ctx);
    }

    fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.heading("categories");
        ui.separator();
        ui.add_space(4.0);

        for cat in Category::all() {
            let selected = self.selected_category == *cat;
            if ui.selectable_label(selected, cat.name()).clicked() {
                self.selected_category = *cat;
            }
        }
    }

    fn render_content(&self, ui: &mut egui::Ui) {
        ScrollArea::vertical().show(ui, |ui| {
            match self.selected_category {
                Category::Overview => self.render_overview(ui),
                Category::Building => self.render_building(ui),
                Category::LinuxFoundation => self.render_linux(ui),
                Category::Fonts => self.render_fonts(ui),
                Category::Icons => self.render_icons(ui),
                Category::Frameworks => self.render_frameworks(ui),
                Category::Libraries => self.render_libraries(ui),
                Category::Tools => self.render_tools(ui),
            }
        });
    }

    fn render_overview(&self, ui: &mut egui::Ui) {
        ui.heading("open source credits");
        ui.add_space(8.0);
        ui.label("slowOS is built entirely on open source software.");
        ui.label("This application acknowledges the projects and");
        ui.label("communities that made slowOS possible.");
        ui.add_space(12.0);
        ui.label("Select a category from the sidebar to view");
        ui.label("detailed attribution information.");
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);
        ui.label("slowOS is licensed under the MIT License.");
        ui.label("Copyright (c) 2024 The Slow Computer Company");
    }

    fn render_building(&self, ui: &mut egui::Ui) {
        ui.heading("building slowOS");
        ui.add_space(8.0);
        ui.label("slowOS is open source. You can download, modify,");
        ui.label("and build the entire operating system yourself.");
        ui.add_space(12.0);

        ui.group(|ui| {
            ui.strong("getting the source code");
            ui.add_space(4.0);
            ui.label("Clone the repository from GitHub:");
            ui.add_space(4.0);
            ui.monospace("git clone https://github.com/strauhal/slowOS.git");
            ui.monospace("cd slowOS");
        });
        ui.add_space(8.0);

        ui.group(|ui| {
            ui.strong("building the applications");
            ui.add_space(4.0);
            ui.label("All slowOS applications are written in Rust.");
            ui.label("Make sure you have Rust installed, then:");
            ui.add_space(4.0);
            ui.monospace("cd v0.1.0");
            ui.monospace("cargo build --release");
            ui.add_space(4.0);
            ui.label("Binaries will be in target/release/");
        });
        ui.add_space(8.0);

        ui.group(|ui| {
            ui.strong("modifying applications");
            ui.add_space(4.0);
            ui.label("Each application is in its own folder:");
            ui.add_space(4.0);
            ui.monospace("slowwrite/    — word processor");
            ui.monospace("slowpaint/    — bitmap editor");
            ui.monospace("slowreader/   — ebook reader");
            ui.monospace("slowmidi/     — MIDI sequencer");
            ui.monospace("slowdesktop/  — desktop environment");
            ui.add_space(4.0);
            ui.label("Edit src/app.rs in any folder to modify that app.");
        });
        ui.add_space(8.0);

        ui.group(|ui| {
            ui.strong("running the desktop");
            ui.add_space(4.0);
            ui.label("After building, run the desktop environment:");
            ui.add_space(4.0);
            ui.monospace("./target/release/slowdesktop");
            ui.add_space(4.0);
            ui.label("The desktop will find and launch other apps.");
        });
        ui.add_space(8.0);

        ui.group(|ui| {
            ui.strong("creating a new application");
            ui.add_space(4.0);
            ui.label("1. Create a new folder: slowmyapp/");
            ui.label("2. Add Cargo.toml with egui and slowcore deps");
            ui.label("3. Create src/main.rs and src/app.rs");
            ui.label("4. Add to workspace Cargo.toml members list");
            ui.label("5. Register in slowdesktop/src/process_manager.rs");
        });
        ui.add_space(12.0);

        ui.separator();
        ui.add_space(8.0);
        ui.label("contributions welcome");
        ui.label("github.com/strauhal/slowOS");
    }

    fn render_linux(&self, ui: &mut egui::Ui) {
        ui.heading("linux foundation");
        ui.add_space(8.0);

        self.credit_item(ui, "Linux Kernel", "GPL-2.0",
            "The core of slowOS, providing hardware abstraction,\nprocess management, and system calls.");

        self.credit_item(ui, "Buildroot", "GPL-2.0",
            "Build system for generating embedded Linux systems.\nUsed to create the minimal slowOS root filesystem.");

        self.credit_item(ui, "BusyBox", "GPL-2.0",
            "Compact Unix utilities in a single executable.\nProvides core system commands for slowOS.");

        self.credit_item(ui, "musl libc", "MIT",
            "Lightweight, fast, and simple C standard library.\nUsed as the system C library for small footprint.");

        self.credit_item(ui, "systemd / init", "LGPL-2.1+",
            "System and service manager (or simple init).\nManages startup and system services.");
    }

    fn render_fonts(&self, ui: &mut egui::Ui) {
        ui.heading("fonts");
        ui.add_space(8.0);

        self.credit_item(ui, "IBM Plex Sans", "OFL-1.1 (SIL Open Font License)",
            "Primary system font for slowOS.\nDesigned by Mike Abbink at IBM.\nClean, modern sans-serif optimized for readability.");

        self.credit_item(ui, "Noto Sans CJK", "OFL-1.1 (SIL Open Font License)",
            "Fallback font for Chinese, Japanese, Korean,\nGreek, Cyrillic, and accented Latin characters.\nPart of Google's Noto font family.");

        self.credit_item(ui, "JetBrains Mono", "OFL-1.1 (SIL Open Font License)",
            "Monospace font for terminal, code, and slowWrite.\nDesigned by JetBrains for developers.");
    }

    fn render_icons(&self, ui: &mut egui::Ui) {
        ui.heading("icons");
        ui.add_space(8.0);

        ui.group(|ui| {
            ui.strong("slowOS icons");
            ui.add_space(4.0);
            ui.label("All the icons in slowOS were designed and");
            ui.label("made by Ernest Strauhal.");
            ui.add_space(8.0);
            ui.label("This includes application icons, file type icons,");
            ui.label("folder icons, and all other visual elements.");
        });
        ui.add_space(12.0);

        ui.group(|ui| {
            ui.strong("license and usage");
            ui.add_space(4.0);
            ui.label("All slowOS icons are released into the public");
            ui.label("domain under the CC0 1.0 Universal dedication.");
            ui.add_space(8.0);
            ui.label("You are free to use, copy, modify, and distribute");
            ui.label("the icons, even for commercial purposes,");
            ui.label("without asking permission.");
            ui.add_space(8.0);
            ui.label("No attribution is required, though it is");
            ui.label("appreciated.");
        });
    }

    fn render_frameworks(&self, ui: &mut egui::Ui) {
        ui.heading("frameworks");
        ui.add_space(8.0);

        self.credit_item(ui, "egui", "MIT / Apache-2.0",
            "Immediate mode GUI library for Rust.\nProvides all UI rendering for slowOS applications.\nhttps://github.com/emilk/egui");

        self.credit_item(ui, "eframe", "MIT / Apache-2.0",
            "Framework for running egui applications.\nHandles windowing, input, and rendering backend.");

        self.credit_item(ui, "Rust", "MIT / Apache-2.0",
            "Systems programming language.\nAll slowOS applications are written in Rust.\nhttps://www.rust-lang.org");
    }

    fn render_libraries(&self, ui: &mut egui::Ui) {
        ui.heading("libraries");
        ui.add_space(8.0);

        self.credit_item(ui, "image-rs", "MIT",
            "Image decoding and encoding library.\nUsed by slowView, slowPaint for image handling.");

        self.credit_item(ui, "tiny-skia", "BSD-3-Clause",
            "2D graphics library for software rendering.\nUsed by slowPaint for vector drawing.");

        self.credit_item(ui, "ropey", "MIT",
            "Rope data structure for efficient text editing.\nUsed by slowWrite for document handling.");

        self.credit_item(ui, "arboard", "MIT / Apache-2.0",
            "Cross-platform clipboard library.\nProvides copy/paste for slowWrite, slowPaint.");

        self.credit_item(ui, "rodio", "MIT / Apache-2.0",
            "Audio playback library.\nUsed by slowMusic for music playback.");

        self.credit_item(ui, "symphonia", "MPL-2.0",
            "Audio decoding library.\nSupports MP3, FLAC, OGG, WAV formats.");

        self.credit_item(ui, "epub-rs", "MIT",
            "EPUB parsing library.\nUsed by slowReader for ebook reading.");

        self.credit_item(ui, "chrono", "MIT / Apache-2.0",
            "Date and time library.\nUsed throughout slowOS for timestamps.");

        self.credit_item(ui, "serde", "MIT / Apache-2.0",
            "Serialization framework.\nUsed for saving settings and documents.");

        self.credit_item(ui, "serde_json", "MIT / Apache-2.0",
            "JSON serialization.\nUsed for config files and data storage.");
    }

    fn render_tools(&self, ui: &mut egui::Ui) {
        ui.heading("build tools");
        ui.add_space(8.0);

        self.credit_item(ui, "Cargo", "MIT / Apache-2.0",
            "Rust's package manager and build system.\nManages dependencies and builds all apps.");

        self.credit_item(ui, "rustc", "MIT / Apache-2.0",
            "The Rust compiler.\nCompiles all slowOS applications.");

        self.credit_item(ui, "cross", "MIT / Apache-2.0",
            "Cross-compilation tool for Rust.\nEnables building for ARM (Raspberry Pi).");

        self.credit_item(ui, "GNU Make", "GPL-3.0",
            "Build automation tool.\nUsed in the build process.");

        self.credit_item(ui, "Git", "GPL-2.0",
            "Version control system.\nUsed for source code management.");
    }

    fn credit_item(&self, ui: &mut egui::Ui, name: &str, license: &str, description: &str) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.strong(name);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(license).small());
                });
            });
            ui.add_space(2.0);
            for line in description.lines() {
                ui.label(line);
            }
        });
        ui.add_space(4.0);
    }
}

impl eframe::App for CreditsApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.repaint.begin_frame(ctx);
        self.handle_keys(ctx);

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            menu_bar(ui, |ui| {
                ui.menu_button("file", |ui| {
                    if ui.button("close   ⌘W").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            status_bar(ui, "thank you");
        });

        egui::SidePanel::left("sidebar")
            .default_width(150.0)
            .show(ctx, |ui| {
                self.render_sidebar(ui);
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(12.0)))
            .show(ctx, |ui| {
                self.render_content(ui);
            });

        if self.show_about {
            let resp = egui::Window::new("about credits")
                .collapsible(false)
                .resizable(false)
                .default_width(300.0)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("credits");
                        ui.label("version 0.2.2");
                        ui.add_space(8.0);
                        ui.label("open source credits viewer");
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label("displays attribution information for");
                    ui.label("all open source components in slowOS");
                    ui.add_space(4.0);
                    ui.label("frameworks:");
                    ui.label("  egui/eframe (MIT)");
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        if ui.button("ok").clicked() {
                            self.show_about = false;
                        }
                    });
                });
            if let Some(r) = &resp { slowcore::dither::draw_window_shadow(ctx, r.response.rect); }
        }
        self.repaint.end_frame(ctx);
    }
}
