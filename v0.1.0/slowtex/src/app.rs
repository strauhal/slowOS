//! SlowTeX - LaTeX editor with side-by-side preview
//! Renders a subset of LaTeX: sections, text, basic math, lists, environments.
//! Built-in PDF export using printpdf (no external pdflatex needed).

use egui::{Context, FontId, Key, Stroke};
use slowcore::storage::{documents_dir, FileBrowser};
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::status_bar;
use std::path::PathBuf;

const DEFAULT_TEMPLATE: &str = r#"\documentclass{article}
\usepackage[utf8]{inputenc}
\usepackage{amsmath}

\title{Untitled Document}
\author{Author}
\date{\today}

\begin{document}

\maketitle

\section{Introduction}

Write your document here.

\end{document}
"#;

pub struct SlowTexApp {
    source: String,
    path: Option<PathBuf>,
    modified: bool,
    preview_lines: Vec<PreviewLine>,
    compile_error: Option<String>,
    show_file_browser: bool,
    file_browser: FileBrowser,
    fb_mode: FbMode,
    save_filename: String,
    show_about: bool,
    show_symbols: bool,
    cursor_line: usize,
    cursor_col: usize,
}

#[derive(PartialEq)]
enum FbMode { Open, Save, ExportPdf }

#[derive(Clone)]
enum PreviewLine {
    Title(String),
    Author(String),
    SectionHeading(String),
    SubsectionHeading(String),
    Paragraph(String),
    Math(String),
    ListItem(String),
    HorizontalRule,
    Blank,
    Error(String),
}

impl SlowTexApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self {
            source: DEFAULT_TEMPLATE.to_string(),
            path: None,
            modified: false,
            preview_lines: Vec::new(),
            compile_error: None,
            show_file_browser: false,
            file_browser: FileBrowser::new(documents_dir())
                .with_filter(vec!["tex".into(), "latex".into()]),
            fb_mode: FbMode::Open,
            save_filename: String::new(),
            show_about: false,
            show_symbols: false,
            cursor_line: 0,
            cursor_col: 0,
        };
        app.update_preview();
        app
    }

    fn update_preview(&mut self) {
        self.preview_lines = parse_latex_preview(&self.source);
    }

    /// Built-in PDF export using printpdf — no external tools needed.
    fn export_pdf(&mut self, pdf_path: PathBuf) {
        use printpdf::*;

        let page_w = Mm(210.0);
        let page_h = Mm(297.0);
        let margin = Mm(25.0);
        let usable_w = page_w.0 - margin.0 * 2.0;

        let (doc, page1, layer1) = PdfDocument::new("SlowTeX Export", page_w, page_h, "Layer 1");

        // Use built-in PDF fonts (no font embedding needed)
        let font_regular = doc.add_builtin_font(BuiltinFont::Helvetica).unwrap();
        let font_bold = doc.add_builtin_font(BuiltinFont::HelveticaBold).unwrap();
        let font_italic = doc.add_builtin_font(BuiltinFont::HelveticaOblique).unwrap();

        let mut current_page = page1;
        let mut current_layer = layer1;
        let mut y = page_h.0 - margin.0; // Start from top

        let lines = parse_latex_preview(&self.source);

        for line in &lines {
            // Check if we need a new page
            if y < margin.0 + 20.0 {
                let (new_page, new_layer) = doc.add_page(page_w, page_h, "Layer 1");
                current_page = new_page;
                current_layer = new_layer;
                y = page_h.0 - margin.0;
            }

            let layer = doc.get_page(current_page).get_layer(current_layer);

            match line {
                PreviewLine::Title(t) => {
                    layer.use_text(t, 24.0, Mm(margin.0), Mm(y), &font_bold);
                    y -= 12.0;
                }
                PreviewLine::Author(a) => {
                    layer.use_text(a, 12.0, Mm(margin.0), Mm(y), &font_italic);
                    y -= 10.0;
                }
                PreviewLine::SectionHeading(s) => {
                    y -= 4.0;
                    layer.use_text(s, 16.0, Mm(margin.0), Mm(y), &font_bold);
                    y -= 8.0;
                }
                PreviewLine::SubsectionHeading(s) => {
                    y -= 3.0;
                    layer.use_text(s, 13.0, Mm(margin.0), Mm(y), &font_bold);
                    y -= 7.0;
                }
                PreviewLine::Paragraph(p) => {
                    // Simple word wrapping
                    let words: Vec<&str> = p.split_whitespace().collect();
                    let mut line_buf = String::new();
                    let chars_per_line = (usable_w / 2.0) as usize; // Approximate

                    for word in words {
                        if line_buf.len() + word.len() + 1 > chars_per_line && !line_buf.is_empty() {
                            if y < margin.0 + 20.0 {
                                let (np, nl) = doc.add_page(page_w, page_h, "Layer 1");
                                current_page = np;
                                current_layer = nl;
                                y = page_h.0 - margin.0;
                            }
                            let l = doc.get_page(current_page).get_layer(current_layer);
                            l.use_text(&line_buf, 11.0, Mm(margin.0), Mm(y), &font_regular);
                            y -= 5.0;
                            line_buf.clear();
                        }
                        if !line_buf.is_empty() { line_buf.push(' '); }
                        line_buf.push_str(word);
                    }
                    if !line_buf.is_empty() {
                        let l = doc.get_page(current_page).get_layer(current_layer);
                        l.use_text(&line_buf, 11.0, Mm(margin.0), Mm(y), &font_regular);
                        y -= 5.0;
                    }
                }
                PreviewLine::Math(m) => {
                    layer.use_text(m, 11.0, Mm(margin.0 + 10.0), Mm(y), &font_italic);
                    y -= 6.0;
                }
                PreviewLine::ListItem(item) => {
                    let text = format!("  - {}", item);
                    layer.use_text(&text, 11.0, Mm(margin.0 + 5.0), Mm(y), &font_regular);
                    y -= 5.0;
                }
                PreviewLine::HorizontalRule => {
                    // Draw a thin line
                    let l = doc.get_page(current_page).get_layer(current_layer);
                    let line_pts = vec![
                        (printpdf::Point::new(Mm(margin.0), Mm(y)), false),
                        (printpdf::Point::new(Mm(page_w.0 - margin.0), Mm(y)), false),
                    ];
                    let line_shape = printpdf::Line {
                        points: line_pts,
                        is_closed: false,
                    };
                    l.add_line(line_shape);
                    y -= 4.0;
                }
                PreviewLine::Blank => {
                    y -= 4.0;
                }
                PreviewLine::Error(e) => {
                    layer.use_text(e, 10.0, Mm(margin.0), Mm(y), &font_italic);
                    y -= 5.0;
                }
            }
        }

        match doc.save(&mut std::io::BufWriter::new(std::fs::File::create(&pdf_path).unwrap())) {
            Ok(()) => {
                self.compile_error = None;
                let _ = open::that_detached(&pdf_path);
            }
            Err(e) => {
                self.compile_error = Some(format!("pdf export failed: {}", e));
            }
        }
    }

    fn open_file(&mut self, path: PathBuf) {
        if let Ok(content) = std::fs::read_to_string(&path) {
            self.source = content;
            self.path = Some(path);
            self.modified = false;
            self.update_preview();
        }
    }

    fn save(&mut self) {
        if let Some(ref path) = self.path {
            let _ = std::fs::write(path, &self.source);
            self.modified = false;
        } else {
            self.fb_mode = FbMode::Save;
            self.save_filename = "document.tex".into();
            self.show_file_browser = true;
        }
    }

    fn insert_snippet(&mut self, snippet: &str) {
        self.source.push_str(snippet);
        self.modified = true;
        self.update_preview();
    }

    fn handle_keys(&mut self, ctx: &Context) {
        // Consume Tab to prevent menu hover
        ctx.input_mut(|i| {
            if i.key_pressed(Key::Tab) {
                i.events.retain(|e| !matches!(e, egui::Event::Key { key: Key::Tab, .. }));
            }
        });
        ctx.input(|i| {
            let cmd = i.modifiers.command;
            if cmd && i.key_pressed(Key::S) { self.save(); }
            if cmd && i.key_pressed(Key::O) { self.fb_mode = FbMode::Open; self.show_file_browser = true; }
            if cmd && i.key_pressed(Key::B) {
                // Export PDF to temp
                let tmp = std::env::temp_dir().join("slowtex_export.pdf");
                self.export_pdf(tmp);
            }
        });
    }

    fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("\\section{}").on_hover_text("insert section").clicked() {
                self.insert_snippet("\n\\section{}\n");
            }
            if ui.button("\\emph{}").clicked() { self.insert_snippet("\\emph{}"); }
            if ui.button("\\textbf{}").clicked() { self.insert_snippet("\\textbf{}"); }
            if ui.button("$ $").on_hover_text("inline math").clicked() { self.insert_snippet("$$"); }
            if ui.button("\\[ \\]").on_hover_text("display math").clicked() { self.insert_snippet("\n\\[\n\n\\]\n"); }
            if ui.button("\\begin{enumerate}").clicked() {
                self.insert_snippet("\n\\begin{enumerate}\n  \\item \n\\end{enumerate}\n");
            }
            if ui.button("\\begin{itemize}").clicked() {
                self.insert_snippet("\n\\begin{itemize}\n  \\item \n\\end{itemize}\n");
            }
            ui.separator();
            if ui.button("symbols").clicked() { self.show_symbols = !self.show_symbols; }
            ui.separator();
            if ui.button("export pdf  ⌘b").clicked() {
                let tmp = std::env::temp_dir().join("slowtex_export.pdf");
                self.export_pdf(tmp);
            }
            if ui.button("save pdf as...").clicked() {
                self.fb_mode = FbMode::ExportPdf;
                self.save_filename = "document.pdf".into();
                self.file_browser = FileBrowser::new(documents_dir());
                self.show_file_browser = true;
            }
        });
    }

    fn render_editor(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let response = ui.add_sized(
            available,
            egui::TextEdit::multiline(&mut self.source)
                .font(egui::FontId::proportional(13.0))
                .desired_width(available.x)
                .code_editor()
        );
        if response.changed() {
            self.modified = true;
            self.update_preview();
        }
    }

    fn render_preview(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            for line in &self.preview_lines {
                match line {
                    PreviewLine::Title(t) => {
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new(t).font(FontId::proportional(24.0)).strong());
                        ui.add_space(5.0);
                    }
                    PreviewLine::Author(a) => {
                        ui.label(egui::RichText::new(a).font(FontId::proportional(14.0)));
                        ui.add_space(10.0);
                    }
                    PreviewLine::SectionHeading(s) => {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(s).font(FontId::proportional(20.0)).strong());
                        ui.add_space(4.0);
                    }
                    PreviewLine::SubsectionHeading(s) => {
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(s).font(FontId::proportional(16.0)).strong());
                        ui.add_space(3.0);
                    }
                    PreviewLine::Paragraph(p) => {
                        ui.label(egui::RichText::new(p).font(FontId::proportional(13.0)));
                    }
                    PreviewLine::Math(m) => {
                        ui.label(egui::RichText::new(m).font(FontId::proportional(14.0)).italics());
                    }
                    PreviewLine::ListItem(item) => {
                        ui.horizontal(|ui| {
                            ui.add_space(20.0);
                            ui.label(egui::RichText::new(format!("- {}", item)).font(FontId::proportional(13.0)));
                        });
                    }
                    PreviewLine::HorizontalRule => { ui.separator(); }
                    PreviewLine::Blank => { ui.add_space(8.0); }
                    PreviewLine::Error(e) => { ui.colored_label(egui::Color32::RED, e); }
                }
            }
        });
    }

    fn render_symbols_window(&mut self, ctx: &Context) {
        egui::Window::new("latex symbols").collapsible(true).resizable(true).show(ctx, |ui| {
            let symbols = [
                ("\\alpha", "a"), ("\\beta", "B"), ("\\gamma", "y"), ("\\delta", "d"),
                ("\\epsilon", "e"), ("\\theta", "0"), ("\\lambda", "A"), ("\\mu", "u"),
                ("\\pi", "n"), ("\\sigma", "o"), ("\\phi", "0"), ("\\omega", "w"),
                ("\\infty", "inf"), ("\\sum", "E"), ("\\prod", "TT"), ("\\int", "S"),
                ("\\partial", "d"), ("\\nabla", "V"), ("\\times", "x"), ("\\div", "/"),
                ("\\neq", "!="), ("\\leq", "<="), ("\\geq", ">="), ("\\approx", "~"),
                ("\\rightarrow", "->"), ("\\leftarrow", "<-"), ("\\Rightarrow", "=>"),
                ("\\forall", "A"), ("\\exists", "E"), ("\\in", "e"), ("\\subset", "c"),
            ];
            egui::Grid::new("symbols_grid").show(ui, |ui| {
                for (idx, (cmd, display)) in symbols.iter().enumerate() {
                    if ui.button(format!("{} {}", display, cmd)).clicked() {
                        self.insert_snippet(cmd);
                    }
                    if (idx + 1) % 4 == 0 { ui.end_row(); }
                }
            });
        });
    }

    fn render_file_browser(&mut self, ctx: &Context) {
        let title = match self.fb_mode {
            FbMode::Open => "open .tex file",
            FbMode::Save => "save .tex file",
            FbMode::ExportPdf => "export pdf",
        };
        egui::Window::new(title).collapsible(false).default_width(400.0).show(ctx, |ui| {
            ui.label(self.file_browser.current_dir.to_string_lossy().to_string());
            ui.separator();
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                let entries = self.file_browser.entries.clone();
                for (idx, entry) in entries.iter().enumerate() {
                    let sel = self.file_browser.selected_index == Some(idx);
                    let r = ui.add(slowcore::widgets::FileListItem::new(&entry.name, entry.is_directory).selected(sel));
                    if r.clicked() { self.file_browser.selected_index = Some(idx); }
                    if r.double_clicked() {
                        if entry.is_directory { self.file_browser.navigate_to(entry.path.clone()); }
                        else if self.fb_mode == FbMode::Open {
                            self.open_file(entry.path.clone());
                            self.show_file_browser = false;
                        }
                    }
                }
            });
            if self.fb_mode == FbMode::Save || self.fb_mode == FbMode::ExportPdf {
                ui.separator();
                ui.horizontal(|ui| { ui.label("filename:"); ui.text_edit_singleline(&mut self.save_filename); });
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("cancel").clicked() { self.show_file_browser = false; }
                let action_label = match self.fb_mode {
                    FbMode::Open => "open",
                    FbMode::Save => "save",
                    FbMode::ExportPdf => "export",
                };
                if ui.button(action_label).clicked() {
                    match self.fb_mode {
                        FbMode::Open => {
                            if let Some(e) = self.file_browser.selected_entry() {
                                if !e.is_directory { let p = e.path.clone(); self.open_file(p); self.show_file_browser = false; }
                            }
                        }
                        FbMode::Save => {
                            if !self.save_filename.is_empty() {
                                let p = self.file_browser.current_dir.join(&self.save_filename);
                                let _ = std::fs::write(&p, &self.source);
                                self.path = Some(p); self.modified = false; self.show_file_browser = false;
                            }
                        }
                        FbMode::ExportPdf => {
                            if !self.save_filename.is_empty() {
                                let p = self.file_browser.current_dir.join(&self.save_filename);
                                self.export_pdf(p);
                                self.show_file_browser = false;
                            }
                        }
                    }
                }
            });
        });
    }
}

impl eframe::App for SlowTexApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.handle_keys(ctx);

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            menu_bar(ui, |ui| {
                ui.menu_button("file", |ui| {
                    if ui.button("new").clicked() { self.source = DEFAULT_TEMPLATE.into(); self.path = None; self.modified = false; self.update_preview(); ui.close_menu(); }
                    if ui.button("open...   ⌘o").clicked() { self.fb_mode = FbMode::Open; self.show_file_browser = true; ui.close_menu(); }
                    ui.separator();
                    if ui.button("save      ⌘s").clicked() { self.save(); ui.close_menu(); }
                    if ui.button("save as...").clicked() {
                        self.fb_mode = FbMode::Save; self.save_filename = "document.tex".into();
                        self.show_file_browser = true; ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("export pdf...").clicked() {
                        self.fb_mode = FbMode::ExportPdf; self.save_filename = "document.pdf".into();
                        self.file_browser = FileBrowser::new(documents_dir());
                        self.show_file_browser = true; ui.close_menu();
                    }
                });
                ui.menu_button("build", |ui| {
                    if ui.button("export pdf  ⌘b").clicked() {
                        let tmp = std::env::temp_dir().join("slowtex_export.pdf");
                        self.export_pdf(tmp); ui.close_menu();
                    }
                });
                ui.menu_button("help", |ui| {
                    if ui.button("about slowTeX").clicked() { self.show_about = true; ui.close_menu(); }
                });
            });
        });
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| self.render_toolbar(ui));

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let name = self.path.as_ref().and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "untitled".into());
            let m = if self.modified { "*" } else { "" };
            let err = self.compile_error.as_deref().unwrap_or("");
            status_bar(ui, &format!("{}{}  |  built-in pdf export  {}", name, m, err));
        });

        egui::SidePanel::right("preview_panel").default_width(400.0)
            .frame(egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(12.0))
                .stroke(Stroke::new(1.0, SlowColors::BLACK)))
            .show(ctx, |ui| {
                ui.label(egui::RichText::new("preview").strong());
                ui.separator();
                self.render_preview(ui);
            });

        egui::CentralPanel::default().frame(
            egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(4.0))
        ).show(ctx, |ui| self.render_editor(ui));

        if self.show_file_browser { self.render_file_browser(ctx); }
        if self.show_symbols { self.render_symbols_window(ctx); }
        if self.show_about {
            egui::Window::new("about slowTeX").collapsible(false).show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("slowTeX");
                    ui.label("version 0.2.0");
                    ui.add_space(10.0);
                    if ui.button("ok").clicked() { self.show_about = false; }
                });
            });
        }
    }
}

// ---------------------------------------------------------------
// LaTeX preview parser (unchanged logic)
// ---------------------------------------------------------------

fn parse_latex_preview(source: &str) -> Vec<PreviewLine> {
    let mut lines = Vec::new();
    let mut in_document = false;
    let mut title = String::new();
    let mut author = String::new();
    let mut in_math = false;
    let mut math_buf = String::new();
    let mut section_count = 0u32;
    let mut subsection_count = 0u32;

    for raw_line in source.lines() {
        let line = raw_line.trim();
        if let Some(rest) = strip_command(line, "\\title") { title = rest; continue; }
        if let Some(rest) = strip_command(line, "\\author") { author = rest; continue; }
        if line == "\\begin{document}" { in_document = true; continue; }
        if line == "\\end{document}" { break; }
        if !in_document { continue; }
        if line.starts_with("\\documentclass") || line.starts_with("\\usepackage") || line.starts_with("\\date") { continue; }
        if line == "\\maketitle" {
            if !title.is_empty() { lines.push(PreviewLine::Title(title.clone())); }
            if !author.is_empty() { lines.push(PreviewLine::Author(author.clone())); }
            lines.push(PreviewLine::HorizontalRule);
            continue;
        }
        if line == "\\[" || line.starts_with("\\begin{equation") || line.starts_with("\\begin{align") {
            in_math = true; math_buf.clear(); continue;
        }
        if line == "\\]" || line.starts_with("\\end{equation") || line.starts_with("\\end{align") {
            in_math = false;
            lines.push(PreviewLine::Math(format!("  {}", render_math_symbols(&math_buf))));
            continue;
        }
        if in_math { math_buf.push_str(line); math_buf.push(' '); continue; }
        if let Some(rest) = strip_command(line, "\\section") {
            section_count += 1; subsection_count = 0;
            lines.push(PreviewLine::SectionHeading(format!("{}. {}", section_count, rest))); continue;
        }
        if let Some(rest) = strip_command(line, "\\subsection") {
            subsection_count += 1;
            lines.push(PreviewLine::SubsectionHeading(format!("{}.{}. {}", section_count, subsection_count, rest))); continue;
        }
        if line == "\\begin{itemize}" || line == "\\begin{enumerate}" || line == "\\end{itemize}" || line == "\\end{enumerate}" { continue; }
        if let Some(rest) = line.strip_prefix("\\item") {
            lines.push(PreviewLine::ListItem(clean_latex(rest.trim()))); continue;
        }
        if line.is_empty() { lines.push(PreviewLine::Blank); continue; }
        if line.starts_with('\\') && !line.contains(' ') { continue; }
        let cleaned = clean_latex(line);
        if !cleaned.is_empty() { lines.push(PreviewLine::Paragraph(cleaned)); }
    }
    lines
}

fn strip_command<'a>(line: &'a str, cmd: &str) -> Option<String> {
    if line.starts_with(cmd) {
        let rest = &line[cmd.len()..];
        if rest.starts_with('{') {
            return Some(rest.trim_start_matches('{').trim_end_matches('}').to_string());
        }
    }
    None
}

fn clean_latex(text: &str) -> String {
    let mut s = text.to_string();
    for (from, to) in [("\\textbf{", "{"), ("\\emph{", "{"), ("\\textit{", "{"), ("\\underline{", "{"), ("\\texttt{", "{")] {
        s = s.replace(from, to);
    }
    s = s.replace('{', "").replace('}', "");
    s = render_inline_math(&s);
    s
}

fn render_inline_math(text: &str) -> String {
    let mut result = String::new();
    let mut in_math = false;
    let mut math_buf = String::new();
    for ch in text.chars() {
        if ch == '$' {
            if in_math { result.push_str(&render_math_symbols(&math_buf)); math_buf.clear(); }
            in_math = !in_math;
        } else if in_math { math_buf.push(ch); }
        else { result.push(ch); }
    }
    result
}

fn render_math_symbols(math: &str) -> String {
    let mut s = math.to_string();
    for (from, to) in [
        ("\\alpha", "alpha"), ("\\beta", "beta"), ("\\gamma", "gamma"), ("\\delta", "delta"),
        ("\\epsilon", "eps"), ("\\theta", "theta"), ("\\lambda", "lambda"), ("\\mu", "mu"),
        ("\\pi", "pi"), ("\\sigma", "sigma"), ("\\phi", "phi"), ("\\omega", "omega"),
        ("\\Omega", "Omega"), ("\\Sigma", "Sigma"), ("\\Delta", "Delta"), ("\\Phi", "Phi"),
        ("\\infty", "inf"), ("\\sum", "SUM"), ("\\prod", "PROD"), ("\\int", "INT"),
        ("\\partial", "d/d"), ("\\nabla", "nabla"), ("\\times", "x"), ("\\div", "/"),
        ("\\neq", "!="), ("\\leq", "<="), ("\\geq", ">="), ("\\approx", "~="),
        ("\\rightarrow", "->"), ("\\leftarrow", "<-"), ("\\Rightarrow", "=>"),
        ("\\forall", "forall"), ("\\exists", "exists"), ("\\in", "in"), ("\\subset", "subset"),
        ("\\sqrt", "sqrt"), ("\\pm", "+/-"), ("\\cdot", "."), ("\\ldots", "..."),
        ("\\frac", "frac"), ("\\left", ""), ("\\right", ""),
    ] { s = s.replace(from, to); }
    s = s.replace('^', "^").replace('_', "_");
    s
}
