//! slowCredits — open source credits and attributions

mod app;

use app::SlowCreditsApp;
use slowcore::theme::SlowTheme;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([720.0, 540.0])
            .with_min_inner_size([400.0, 300.0]),
        ..Default::default()
    };

    eframe::run_native(
        "slowCredits",
        options,
        Box::new(|cc| {
            SlowTheme::default().apply(&cc.egui_ctx);
            Ok(Box::new(SlowCreditsApp::new(cc)))
        }),
    )
}
