mod chess;
mod app;
use app::SlowChessApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 680.0])
            .with_title("slowChess"),
        ..Default::default()
    };
    eframe::run_native("slowChess", options, Box::new(|cc| {
        slowcore::SlowTheme::default().apply(&cc.egui_ctx);
        Box::new(SlowChessApp::new(cc))
    }))
}
