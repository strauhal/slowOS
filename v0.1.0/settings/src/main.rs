//! settings — System settings for slowOS

mod app;

use app::SettingsApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 500.0])
            .with_title("settings"),
        ..Default::default()
    };

    eframe::run_native(
        "settings",
        options,
        Box::new(|cc| {
            slowcore::SlowTheme::default().apply(&cc.egui_ctx);
            Box::new(SettingsApp::new(cc))
        }),
    )
}
