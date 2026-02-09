//! KeyMacros - Keyboard shortcuts reference for slowOS

mod app;

use app::KeyMacrosApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([520.0, 450.0])
            .with_title("keyMacros"),
        ..Default::default()
    };

    eframe::run_native(
        "keyMacros",
        options,
        Box::new(|cc| {
            slowcore::SlowTheme::default().apply(&cc.egui_ctx);
            Box::new(KeyMacrosApp::new(cc))
        }),
    )
}
