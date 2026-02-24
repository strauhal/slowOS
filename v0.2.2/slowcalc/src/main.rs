//! SlowCalc - A calculator for the Slow Computer
//!
//! Basic and scientific calculator modes.

mod app;

use app::SlowCalcApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([260.0, 350.0])
            .with_title("calculator"),
        ..Default::default()
    };

    eframe::run_native(
        "calculator",
        options,
        Box::new(|cc| {
            slowcore::SlowTheme::default().apply(&cc.egui_ctx);
            Box::new(SlowCalcApp::new(cc))
        }),
    )
}
