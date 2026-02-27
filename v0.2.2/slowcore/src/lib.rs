//! slowcore — shared library for slow computer applications

pub mod dither;
pub mod drag;
pub mod minimize;
pub mod repaint;
pub mod safety;
pub mod storage;
pub mod text_edit;
pub mod theme;
pub mod widgets;

pub use repaint::RepaintController;
pub use theme::SlowTheme;

/// Get cascade window position offset from environment variable
/// Returns (x_offset, y_offset) based on SLOWOS_CASCADE env var
/// Used for staggering multiple window instances
pub fn cascade_position() -> Option<egui::Pos2> {
    std::env::var("SLOWOS_CASCADE").ok()
        .and_then(|s| s.parse::<u32>().ok())
        .map(|n| {
            let offset = (n as f32) * 30.0;
            egui::Pos2::new(100.0 + offset, 100.0 + offset)
        })
}
