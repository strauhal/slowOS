//! slowTerm — a minimal terminal for the Slow Computer

mod app;

use app::SlowTermApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([720.0, 480.0])
            .with_title("slowTerm"),
        ..Default::default()
    };
    eframe::run_native("slowTerm", options, Box::new(|cc| {
        slowcore::SlowTheme::default().apply(&cc.egui_ctx);
        Box::new(SlowTermApp::new(cc))
    }))
}
