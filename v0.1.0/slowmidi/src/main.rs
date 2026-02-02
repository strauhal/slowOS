//! slowMidi — MIDI notation and sequencer

mod app;

use app::SlowMidiApp;
use slowcore::theme::SlowTheme;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([960.0, 640.0])
            .with_min_inner_size([640.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "slowMidi",
        options,
        Box::new(|cc| {
            SlowTheme::default().apply(&cc.egui_ctx);
            Ok(Box::new(SlowMidiApp::new(cc)))
        }),
    )
}
