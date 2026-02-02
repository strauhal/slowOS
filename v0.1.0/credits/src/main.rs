//! credits — open source credits and attributions

mod app;

use app::CreditsApp;
use slowcore::theme::SlowTheme;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([720.0, 540.0])
            .with_min_inner_size([400.0, 300.0]),
        ..Default::default()
    };

    eframe::run_native(
        "credits",
        options,
        Box::new(|cc| {
            SlowTheme::default().apply(&cc.egui_ctx);
            Box::new(CreditsApp::new(cc))
        }),
    )
}
