mod app;
use app::SlowDesignApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 640.0])
            .with_title("slowDesign"),
        ..Default::default()
    };
    eframe::run_native("slowDesign", options, Box::new(|cc| {
        slowcore::SlowTheme::default().apply(&cc.egui_ctx);
        Box::new(SlowDesignApp::new(cc))
    }))
}
