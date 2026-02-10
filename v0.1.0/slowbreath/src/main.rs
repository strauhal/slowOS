//! slowBreath - Mindful breathing timer for the Slow Computer

mod app;

use app::SlowBreathApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([340.0, 420.0])
            .with_title("breath"),
        ..Default::default()
    };

    eframe::run_native(
        "slowBreath",
        options,
        Box::new(|cc| {
            slowcore::SlowTheme::default().apply(&cc.egui_ctx);
            Box::new(SlowBreathApp::new(cc))
        }),
    )
}
