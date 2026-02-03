mod app;
use app::SlowTexApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([680.0, 500.0])
            .with_title("slowTeX"),
        ..Default::default()
    };
    eframe::run_native("slowTeX", options, Box::new(|cc| {
        slowcore::SlowTheme::default().apply(&cc.egui_ctx);
        Box::new(SlowTexApp::new(cc))
    }))
}
