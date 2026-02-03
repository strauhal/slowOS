mod sheet;
mod app;

use app::SlowSheetsApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([640.0, 480.0])
            .with_title("slowSheets"),
        ..Default::default()
    };
    eframe::run_native(
        "SlowSheets",
        options,
        Box::new(|cc| {
            slowcore::SlowTheme::default().apply(&cc.egui_ctx);
            Box::new(SlowSheetsApp::new(cc))
        }),
    )
}
