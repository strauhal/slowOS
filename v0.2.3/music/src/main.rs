mod app;
use app::SlowMusicApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let initial_file = std::env::args().nth(1).map(std::path::PathBuf::from);

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([460.0, 480.0])
            .with_title("music"),
        ..Default::default()
    };
    eframe::run_native("music", options, Box::new(move |cc| {
        slowcore::SlowTheme::default().apply(&cc.egui_ctx);
        let mut app = SlowMusicApp::new(cc);
        if let Some(path) = initial_file {
            if path.exists() {
                app.add_file(path);
                app.play_track(0);
            }
        }
        Box::new(app)
    }))
}
