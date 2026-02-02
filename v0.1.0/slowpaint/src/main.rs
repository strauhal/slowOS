//! SlowPaint - A minimal bitmap image editor for the Slow Computer
//! 
//! Classic MacPaint-inspired pixel art and image editing.

mod canvas;
mod tools;
mod app;

use app::SlowPaintApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_title("slowPaint"),
        ..Default::default()
    };
    
    eframe::run_native(
        "SlowPaint",
        options,
        Box::new(|cc| {
            slowcore::SlowTheme::default().apply(&cc.egui_ctx);
            Box::new(SlowPaintApp::new(cc))
        }),
    )
}
