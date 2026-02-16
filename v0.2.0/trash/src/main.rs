mod app;
use app::TrashApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([480.0, 360.0])
            .with_title("trash"),
        ..Default::default()
    };
    eframe::run_native("trash", options, Box::new(|cc| {
        slowcore::SlowTheme::default().apply(&cc.egui_ctx);
        Box::new(TrashApp::new(cc))
    }))
}
