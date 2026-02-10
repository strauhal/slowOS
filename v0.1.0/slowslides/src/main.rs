mod app;
use app::SlowSlidesApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let initial_file = std::env::args().nth(1).map(std::path::PathBuf::from);

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([640.0, 580.0])
            .with_title("slowSlides"),
        ..Default::default()
    };
    eframe::run_native("slowSlides", options, Box::new(move |cc| {
        slowcore::SlowTheme::default().apply(&cc.egui_ctx);
        let mut app = SlowSlidesApp::new(cc);
        if let Some(path) = initial_file {
            if path.exists() {
                app.open_file(path);
            }
        }
        Box::new(app)
    }))
}
