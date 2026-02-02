mod app;
use app::SlowSlidesApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 650.0])
            .with_title("slowSlides"),
        ..Default::default()
    };
    eframe::run_native("slowSlides", options, Box::new(|cc| {
        slowcore::SlowTheme::default().apply(&cc.egui_ctx);
        Box::new(SlowSlidesApp::new(cc))
    }))
}
