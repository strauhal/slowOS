//! slowMidi — MIDI notation and sequencer

mod app;

use app::SlowMidiApp;
use slowcore::theme::SlowTheme;

fn main() -> eframe::Result<()> {
    let initial_file = std::env::args().nth(1).map(std::path::PathBuf::from);

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([760.0, 580.0])
        .with_min_inner_size([560.0, 400.0])
        .with_title("slowMidi");

    if let Some(pos) = slowcore::cascade_position() {
        viewport = viewport.with_position(pos);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "slowMidi",
        options,
        Box::new(move |cc| {
            SlowTheme::default().apply(&cc.egui_ctx);
            let mut app = SlowMidiApp::new(cc);
            if let Some(path) = initial_file {
                if path.exists() {
                    app.load_from_path(path);
                }
            }
            Box::new(app)
        }),
    )
}
