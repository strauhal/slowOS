//! slowWrite v0.2.4 — word processor for slowOS
//!
//! HTML-aware text editing with live formatting, find & replace,
//! document outline, focus mode, and writing statistics.

mod app;
mod document;
mod html;

use app::SlowWriteApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let initial_file = std::env::args().nth(1).map(std::path::PathBuf::from);

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([600.0, 440.0])
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
            // Apply base theme (sets fonts, style, pixels_per_point)
            slowcore::SlowTheme::default().apply(&cc.egui_ctx);

            // Build font definitions with the same base fonts as the theme,
            // plus Bold/Italic/BoldItalic families for rich text rendering.
            // Start from FontDefinitions::empty so egui doesn't embed its
            // default Hack/Ubuntu fonts that we'd just overwrite anyway.
            let mut fonts = egui::FontDefinitions::empty();

            // Each font tries /usr/share/slowos/fonts first (shared via
            // kernel page cache across all slowOS processes) and falls
            // back to the embedded copy in dev builds.
            let load = |name: &str, embedded: &'static [u8]| -> egui::FontData {
                let disk = std::path::PathBuf::from("/usr/share/slowos/fonts").join(name);
                if let Ok(b) = std::fs::read(&disk) {
                    egui::FontData::from_owned(b)
                } else {
                    egui::FontData::from_static(embedded)
                }
            };

            // Base fonts (same as theme)
            fonts.font_data.insert("IBMPlexSans".into(),
                load("IBMPlexSans-Text.otf", include_bytes!("../../slowcore/fonts/IBMPlexSans-Text.otf")));
            fonts.font_data.insert("JetBrainsMono".into(),
                load("JetBrainsMono-Regular.ttf", include_bytes!("../../slowcore/fonts/JetBrainsMono-Regular.ttf")));

            // Rich text fonts
            fonts.font_data.insert("IBMPlexSans-Bold".into(),
                load("IBMPlexSans-Bold.otf", include_bytes!("../../slowcore/fonts/IBMPlexSans-Bold.otf")));
            fonts.font_data.insert("IBMPlexSans-Italic".into(),
                load("IBMPlexSans-Italic.otf", include_bytes!("../../slowcore/fonts/IBMPlexSans-Italic.otf")));
            fonts.font_data.insert("IBMPlexSans-BoldItalic".into(),
                load("IBMPlexSans-BoldItalic.otf", include_bytes!("../../slowcore/fonts/IBMPlexSans-BoldItalic.otf")));

            // CJK font (try loading from various locations)
            let cjk_loaded = slowcore::theme::SlowTheme::load_cjk_font_data();
            if let Some(data) = cjk_loaded {
                fonts.font_data.insert("NotoSansCJK".into(), egui::FontData::from_owned(data));
            }
            let has_cjk = fonts.font_data.contains_key("NotoSansCJK");

            // Set up font families
            let mut prop = vec!["IBMPlexSans".to_string()];
            let mut mono = vec!["JetBrainsMono".to_string()];
            let mut bold = vec!["IBMPlexSans-Bold".to_string()];
            let mut italic = vec!["IBMPlexSans-Italic".to_string()];
            let mut bold_italic = vec!["IBMPlexSans-BoldItalic".to_string()];
            if has_cjk {
                prop.push("NotoSansCJK".to_string());
                mono.push("NotoSansCJK".to_string());
                bold.push("NotoSansCJK".to_string());
                italic.push("NotoSansCJK".to_string());
                bold_italic.push("NotoSansCJK".to_string());
            }

            fonts.families.insert(egui::FontFamily::Proportional, prop);
            fonts.families.insert(egui::FontFamily::Monospace, mono);
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
