//! slowReader - A minimal ebook reader for the Slow Computer
//!
//! Focused reading experience for EPUB and text files.

mod book;
mod reader;
mod library;
mod app;

use app::SlowReaderApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let initial_file = std::env::args().nth(1).map(std::path::PathBuf::from);

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([640.0, 440.0])
            .with_title("slowReader"),
        ..Default::default()
    };

    eframe::run_native(
        "slowReader",
        options,
        Box::new(move |cc| {
            slowcore::SlowTheme::default().apply(&cc.egui_ctx);
            let mut app = SlowReaderApp::new(cc);
            if let Some(path) = initial_file {
                if path.exists() {
                    app.open_book(path);
                }
            }
            Box::new(app)
        }),
    )
}
