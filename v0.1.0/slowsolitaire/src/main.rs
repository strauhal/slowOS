mod app;
use app::SlowSolitaireApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([740.0, 560.0])
            .with_title("solitaire"),
        ..Default::default()
    };
    eframe::run_native("solitaire", options, Box::new(|cc| {
        slowcore::SlowTheme::default().apply(&cc.egui_ctx);
        Box::new(SlowSolitaireApp::new(cc))
    }))
}
