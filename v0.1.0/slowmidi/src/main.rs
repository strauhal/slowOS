//! slowMidi — MIDI notation and sequencer

mod app;

use app::SlowMidiApp;
use slowcore::theme::SlowTheme;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([700.0, 580.0])
            .with_min_inner_size([500.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "slowMidi",
        options,
        Box::new(|cc| {
            SlowTheme::default().apply(&cc.egui_ctx);
            Box::new(SlowMidiApp::new(cc))
        }),
    )
}
