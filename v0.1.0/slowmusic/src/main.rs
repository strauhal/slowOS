mod app;
use app::SlowMusicApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([460.0, 480.0])
            .with_title("slowMusic"),
        ..Default::default()
    };
    eframe::run_native("slowMusic", options, Box::new(|cc| {
        slowcore::SlowTheme::default().apply(&cc.egui_ctx);
        Box::new(SlowMusicApp::new(cc))
    }))
}
