//! slowBooks - A minimal ebook reader for the Slow Computer
//! 
//! Focused reading experience for EPUB and text files.

mod book;
mod reader;
mod library;
mod app;

use app::SlowBooksApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("slowBooks"),
        ..Default::default()
    };
    
    eframe::run_native(
        "slowBooks",
        options,
        Box::new(|cc| {
            slowcore::SlowTheme::default().apply(&cc.egui_ctx);
            Box::new(SlowBooksApp::new(cc))
        }),
    )
}
