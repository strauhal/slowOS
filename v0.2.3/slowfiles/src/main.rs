mod app;
use app::SlowFilesApp;
use eframe::NativeOptions;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let start_dir = std::env::args().nth(1).map(PathBuf::from);

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([560.0, 400.0])
        .with_title("files");

    // Apply cascade position for window staggering
    if let Some(pos) = slowcore::cascade_position() {
        viewport = viewport.with_position(pos);
    }

    let options = NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native("files", options, Box::new(move |cc| {
        slowcore::SlowTheme::default().apply(&cc.egui_ctx);
        Box::new(SlowFilesApp::new_with_dir(cc, start_dir))
    }))
}
