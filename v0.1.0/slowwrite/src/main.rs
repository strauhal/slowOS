//! SlowWrite - A minimal word processor for the Slow Computer
//!
//! Simplified version using egui's built-in TextEdit for reliable copy/paste.

mod app;

use app::SlowWriteApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let initial_file = std::env::args().nth(1).map(std::path::PathBuf::from);

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([580.0, 440.0])
            .with_title("slowWrite"),
        ..Default::default()
    };

    eframe::run_native(
        "SlowWrite",
        options,
        Box::new(move |cc| {
            slowcore::SlowTheme::default().apply(&cc.egui_ctx);
            let mut app = SlowWriteApp::new(cc);
            if let Some(path) = initial_file {
                if path.exists() {
                    app.open_file(path);
                }
            }
            Box::new(app)
        }),
    )
}
