mod app;
use app::SlowFilesApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 550.0])
            .with_title("slowFiles"),
        ..Default::default()
    };
    eframe::run_native("slowFiles", options, Box::new(|cc| {
        slowcore::SlowTheme::default().apply(&cc.egui_ctx);
        Box::new(SlowFilesApp::new(cc))
    }))
}
