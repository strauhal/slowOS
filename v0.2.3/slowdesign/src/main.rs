mod app;
use app::SlowDesignApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    // Install panic handler that logs to file for debugging
    std::panic::set_hook(Box::new(|info| {
        let msg = format!(
            "[slowDesign crash] {}\nbacktrace:\n{:?}",
            info,
            std::backtrace::Backtrace::force_capture()
        );
        eprintln!("{}", msg);
        let _ = std::fs::write("/tmp/slowdesign-crash.log", &msg);
    }));

    eprintln!("[slowDesign] starting...");

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([720.0, 520.0])
        .with_title("slowDesign");

    if let Some(pos) = slowcore::cascade_position() {
        viewport = viewport.with_position(pos);
    }

    let options = NativeOptions {
        viewport,
        ..Default::default()
    };

    eprintln!("[slowDesign] creating window...");

    eframe::run_native("slowDesign", options, Box::new(|cc| {
        eprintln!("[slowDesign] applying theme...");
        slowcore::SlowTheme::default().apply(&cc.egui_ctx);
        eprintln!("[slowDesign] creating app...");
        let app = SlowDesignApp::new(cc);
        eprintln!("[slowDesign] ready.");
        Box::new(app)
    }))
}
