mod app;

use app::SlowDateApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([400.0, 420.0])
        .with_title("slowDate");

    if let Some(pos) = slowcore::cascade_position() {
        viewport = viewport.with_position(pos);
    }

    let options = NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native("slowDate", options, Box::new(move |cc| {
        slowcore::SlowTheme::default().apply(&cc.egui_ctx);
        Box::new(SlowDateApp::new(cc))
    }))
}
