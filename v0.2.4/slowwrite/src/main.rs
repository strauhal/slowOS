//! slowWrite v0.2.4 — markdown word processor for slowOS
//!
//! Markdown-aware text editing with live formatting, find & replace,
//! document outline, focus mode, and writing statistics.

mod app;
mod document;
mod markdown;

use app::SlowWriteApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let initial_file = std::env::args().nth(1).map(std::path::PathBuf::from);

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([780.0, 560.0])
        .with_title("slowWrite");

    if let Some(pos) = slowcore::cascade_position() {
        viewport = viewport.with_position(pos);
    }

    let options = NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "SlowWrite",
        options,
        Box::new(move |cc| {
            slowcore::SlowTheme::default().apply(&cc.egui_ctx);

            // Register bold and italic font families for markdown rendering
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "IBMPlexSans".into(),
                egui::FontData::from_static(include_bytes!("../../slowcore/fonts/IBMPlexSans-Text.otf")),
            );
            fonts.font_data.insert(
                "IBMPlexSans-Bold".into(),
                egui::FontData::from_static(include_bytes!("../../slowcore/fonts/IBMPlexSans-Bold.otf")),
            );
            fonts.font_data.insert(
                "IBMPlexSans-Italic".into(),
                egui::FontData::from_static(include_bytes!("../../slowcore/fonts/IBMPlexSans-Italic.otf")),
            );
            fonts.font_data.insert(
                "IBMPlexSans-BoldItalic".into(),
                egui::FontData::from_static(include_bytes!("../../slowcore/fonts/IBMPlexSans-BoldItalic.otf")),
            );
            fonts.font_data.insert(
                "JetBrainsMono".into(),
                egui::FontData::from_static(include_bytes!("../../slowcore/fonts/JetBrainsMono-Regular.ttf")),
            );

            // Try loading CJK font
            let cjk_name = "NotoSansCJK-Subset.otf";
            let cjk_paths = [
                std::path::PathBuf::from("/usr/share/slowos/fonts").join(cjk_name),
                std::path::PathBuf::from("/usr/share/fonts").join(cjk_name),
            ];
            for path in &cjk_paths {
                if let Ok(data) = std::fs::read(path) {
                    fonts.font_data.insert("NotoSansCJK".into(), egui::FontData::from_owned(data));
                    break;
                }
            }
            // Also check relative to executable
            if !fonts.font_data.contains_key("NotoSansCJK") {
                if let Ok(exe) = std::env::current_exe() {
                    if let Some(dir) = exe.parent() {
                        for candidate in [
                            dir.join("fonts").join(cjk_name),
                            dir.join(cjk_name),
                        ] {
                            if let Ok(data) = std::fs::read(&candidate) {
                                fonts.font_data.insert("NotoSansCJK".into(), egui::FontData::from_owned(data));
                                break;
                            }
                        }
                    }
                }
            }

            let mut prop = vec!["IBMPlexSans".to_string()];
            let mut mono = vec!["JetBrainsMono".to_string()];
            if fonts.font_data.contains_key("NotoSansCJK") {
                prop.push("NotoSansCJK".to_string());
                mono.push("NotoSansCJK".to_string());
            }

            fonts.families.insert(egui::FontFamily::Proportional, prop.clone());
            fonts.families.insert(egui::FontFamily::Monospace, mono);

            let mut bold = vec!["IBMPlexSans-Bold".to_string()];
            let mut italic = vec!["IBMPlexSans-Italic".to_string()];
            let mut bold_italic = vec!["IBMPlexSans-BoldItalic".to_string()];
            if fonts.font_data.contains_key("NotoSansCJK") {
                bold.push("NotoSansCJK".to_string());
                italic.push("NotoSansCJK".to_string());
                bold_italic.push("NotoSansCJK".to_string());
            }
            fonts.families.insert(egui::FontFamily::Name("Bold".into()), bold);
            fonts.families.insert(egui::FontFamily::Name("Italic".into()), italic);
            fonts.families.insert(egui::FontFamily::Name("BoldItalic".into()), bold_italic);

            cc.egui_ctx.set_fonts(fonts);

            let mut app = SlowWriteApp::new(cc);
            if let Some(path) = initial_file {
                if path.exists() {
                    app.open_file(path);
                }
            }
            Box::new(app)
        }),
    )
}
