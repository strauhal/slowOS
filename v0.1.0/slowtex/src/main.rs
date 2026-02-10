mod app;
use app::SlowTexApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let initial_file = std::env::args().nth(1).map(std::path::PathBuf::from);

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([680.0, 500.0])
        .with_title("slowTeX");

    if let Some(pos) = slowcore::cascade_position() {
        viewport = viewport.with_position(pos);
    }

    let options = NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native("slowTeX", options, Box::new(move |cc| {
        slowcore::SlowTheme::default().apply(&cc.egui_ctx);
        let mut app = SlowTexApp::new(cc);
        if let Some(path) = initial_file {
            if path.exists() {
                app.open_file(path);
            }
        }
        Box::new(app)
    }))
}
