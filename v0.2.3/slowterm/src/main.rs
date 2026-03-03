//! terminal — a minimal terminal for the Slow Computer

mod app;

use app::SlowTermApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([520.0, 380.0])
            .with_title("terminal"),
        ..Default::default()
    };
    eframe::run_native("terminal", options, Box::new(|cc| {
        slowcore::SlowTheme::default().apply(&cc.egui_ctx);
        Box::new(SlowTermApp::new(cc))
    }))
}
