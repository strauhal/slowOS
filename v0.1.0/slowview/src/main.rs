//! slowView — a minimal image and PDF viewer for the Slow Computer

mod app;
mod loader;

use app::SlowViewApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    // Check if a file path was passed as argument
    let initial_path = std::env::args().nth(1).map(std::path::PathBuf::from);

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([520.0, 400.0])
            .with_title("slowView"),
        ..Default::default()
    };
    eframe::run_native("slowView", options, Box::new(move |cc| {
        slowcore::SlowTheme::default().apply(&cc.egui_ctx);
        Box::new(SlowViewApp::new(cc, initial_path))
    }))
}
