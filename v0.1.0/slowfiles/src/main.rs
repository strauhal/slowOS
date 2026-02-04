mod app;
use app::SlowFilesApp;
use eframe::NativeOptions;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let start_dir = std::env::args().nth(1).map(PathBuf::from);

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 400.0])
            .with_title("slowFiles"),
        ..Default::default()
    };
    eframe::run_native("slowFiles", options, Box::new(move |cc| {
        slowcore::SlowTheme::default().apply(&cc.egui_ctx);
        Box::new(SlowFilesApp::new_with_dir(cc, start_dir))
    }))
}
