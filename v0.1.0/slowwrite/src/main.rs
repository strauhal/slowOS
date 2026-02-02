//! SlowWrite - A minimal word processor for the Slow Computer
//!
//! Simplified version using egui's built-in TextEdit for reliable copy/paste.

mod app;

use app::SlowWriteApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("slowWrite"),
        ..Default::default()
    };
    
    eframe::run_native(
        "SlowWrite",
        options,
        Box::new(|cc| {
            // Apply our theme
            slowcore::SlowTheme::default().apply(&cc.egui_ctx);
            Box::new(SlowWriteApp::new(cc))
        }),
    )
}
