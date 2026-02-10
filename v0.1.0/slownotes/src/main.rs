mod app;
use app::SlowNoteApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 380.0])
            .with_title("notes"),
        ..Default::default()
    };
    eframe::run_native("notes", options, Box::new(|cc| {
        slowcore::SlowTheme::default().apply(&cc.egui_ctx);
        Box::new(SlowNoteApp::new(cc))
    }))
}
