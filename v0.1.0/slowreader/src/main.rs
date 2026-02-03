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
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("slowReader"),
        ..Default::default()
    };

    eframe::run_native(
        "slowReader",
        options,
        Box::new(|cc| {
            slowcore::SlowTheme::default().apply(&cc.egui_ctx);
            Box::new(SlowReaderApp::new(cc))
        }),
    )
}
