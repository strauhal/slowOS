//! SlowSheets application — with multi-cell selection

use crate::sheet::{col_letter, CellAddr, CellValue, Sheet};
use egui::{Context, Key, Pos2, Rect, Sense, Stroke, Vec2};
use slowcore::dither::draw_dither_selection;
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::status_bar;
use slowcore::storage::{documents_dir, FileBrowser};
use std::collections::HashSet;
use std::path::PathBuf;

pub struct SlowSheetsApp {
    sheet: Sheet,
    // Active cell (cursor)
    sel_col: usize,
    sel_row: usize,
    // Rectangular range selection: anchor point for shift+click / drag
    range_anchor: Option<(usize, usize)>,
    // Individual extra-selected cells from cmd+click
    extra_cells: HashSet<(usize, usize)>,
    // Drag state
    dragging: bool,
    drag_start: Option<(usize, usize)>,
    // Editing
    editing: bool,
    edit_buf: String,
    formula_bar_focus: bool,
    // File browser
    show_file_browser: bool,
    file_browser: FileBrowser,
    fb_mode: FbMode,
    save_filename: String,
    // UI state
    show_about: bool,
    scroll_row: usize,
    scroll_col: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum FbMode { Open, Save }

impl SlowSheetsApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            sheet: Sheet::new(),
            sel_col: 0,
            sel_row: 0,
            range_anchor: None,
            extra_cells: HashSet::new(),
            dragging: false,
            drag_start: None,
            editing: false,
            edit_buf: String::new(),
            formula_bar_focus: false,
            show_file_browser: false,
            file_browser: FileBrowser::new(documents_dir())
                .with_filter(vec!["csv".into(), "json".into()]),
            fb_mode: FbMode::Open,
            save_filename: String::new(),
            show_about: false,
            scroll_row: 0,
            scroll_col: 0,
        }
    }

    fn commit_edit(&mut self) {
        if self.editing {
            self.sheet.set_input(self.sel_col, self.sel_row, self.edit_buf.clone());
            self.editing = false;
        }
    }

    fn cancel_edit(&mut self) {
        self.editing = false;
        self.edit_buf.clear();
    }

    fn start_edit(&mut self) {
        self.edit_buf = self.sheet.get_input(self.sel_col, self.sel_row).to_string();
        self.editing = true;
    }

    fn clear_selection(&mut self) {
        self.range_anchor = None;
        self.extra_cells.clear();
    }

    /// Check if a cell is in the current selection (either range or extra cells)
    fn is_selected(&self, col: usize, row: usize) -> bool {
        // Active cell is always selected
        if col == self.sel_col && row == self.sel_row {
            return true;
        }
        // Check rectangular range selection
        if let Some((ac, ar)) = self.range_anchor {
            let (c0, c1) = (ac.min(self.sel_col), ac.max(self.sel_col));
            let (r0, r1) = (ar.min(self.sel_row), ar.max(self.sel_row));
            if col >= c0 && col <= c1 && row >= r0 && row <= r1 {
                return true;
            }
        }
        // Check extra individual cells
        self.extra_cells.contains(&(col, row))
    }

    /// Check if there's a multi-cell selection (more than just the active cell)
    fn has_multi_selection(&self) -> bool {
        self.range_anchor.is_some() || !self.extra_cells.is_empty()
    }

    /// Get all selected cells as a sorted list of (col, row)
    fn selected_cells(&self) -> Vec<(usize, usize)> {
        let mut cells = HashSet::new();
        cells.insert((self.sel_col, self.sel_row));

        if let Some((ac, ar)) = self.range_anchor {
            let (c0, c1) = (ac.min(self.sel_col), ac.max(self.sel_col));
            let (r0, r1) = (ar.min(self.sel_row), ar.max(self.sel_row));
            for c in c0..=c1 {
                for r in r0..=r1 {
                    cells.insert((c, r));
                }
            }
        }

        for &cell in &self.extra_cells {
            cells.insert(cell);
        }

        let mut result: Vec<_> = cells.into_iter().collect();
        result.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
        result
    }

    /// Build a formula reference string from the current selection
    /// e.g. "A1:B3" for a range, or "A1,A3,B2" for individual cells
    fn selection_reference(&self) -> String {
        if let Some((ac, ar)) = self.range_anchor {
            // Rectangular range
            let (c0, c1) = (ac.min(self.sel_col), ac.max(self.sel_col));
            let (r0, r1) = (ar.min(self.sel_row), ar.max(self.sel_row));
            if c0 == c1 && r0 == r1 {
                return CellAddr::new(c0, r0).label();
            }
            format!("{}:{}", CellAddr::new(c0, r0).label(), CellAddr::new(c1, r1).label())
        } else if !self.extra_cells.is_empty() {
            // Individual cells — include active cell + extras
            let cells = self.selected_cells();
            // Try to detect if they form a contiguous rectangular range
            if let Some(range_str) = try_compact_range(&cells) {
                return range_str;
            }
            cells.iter()
                .map(|(c, r)| CellAddr::new(*c, *r).label())
                .collect::<Vec<_>>()
                .join(",")
        } else {
            CellAddr::new(self.sel_col, self.sel_row).label()
        }
    }

    /// Collect numeric values from all selected cells
    fn selected_numeric_values(&self) -> Vec<f64> {
        let cells = self.selected_cells();
        let mut vals = Vec::new();
        for (c, r) in cells {
            if let CellValue::Number(n) = self.sheet.eval(c, r) {
                vals.push(n);
            }
        }
        vals
    }

    fn open_file(&mut self, path: PathBuf) {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let result = match ext {
            "csv" => Sheet::open_csv(path),
            "json" => Sheet::open_json(path),
            _ => Sheet::open_csv(path),
        };
        if let Ok(s) = result { self.sheet = s; }
    }

    fn save(&mut self) {
        if let Some(path) = self.sheet.path.clone() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("csv");
            match ext {
                "json" => { let _ = self.sheet.save_json(&path); }
                _ => { let _ = self.sheet.save_csv(&path); }
            }
        } else {
            self.fb_mode = FbMode::Save;
            self.save_filename = "untitled.csv".into();
            self.show_file_browser = true;
        }
    }

    fn handle_keys(&mut self, ctx: &Context) {
        // Detect and consume Tab key before egui processes it
        let tab_pressed = ctx.input_mut(|i| {
            let pressed = i.key_pressed(Key::Tab);
            if pressed {
                i.events.retain(|e| !matches!(e, egui::Event::Key { key: Key::Tab, .. }));
            }
            pressed
        });
        ctx.input(|i| {
            let cmd = i.modifiers.command;
            let shift = i.modifiers.shift;

            if cmd && i.key_pressed(Key::N) {
                self.sheet = Sheet::new();
                self.clear_selection();
            }
            if cmd && i.key_pressed(Key::O) {
                self.fb_mode = FbMode::Open;
                self.show_file_browser = true;
            }
            if cmd && i.key_pressed(Key::S) { self.save(); }

            // Copy selection values to clipboard (⌘C when not editing)
            if cmd && i.key_pressed(Key::C) && !self.editing {
                let cells = self.selected_cells();
                if !cells.is_empty() {
                    let mut lines: Vec<String> = Vec::new();
                    let mut current_row = cells[0].1;
                    let mut line_parts: Vec<String> = Vec::new();
                    for (c, r) in &cells {
                        if *r != current_row {
                            lines.push(line_parts.join("\t"));
                            line_parts.clear();
                            current_row = *r;
                        }
                        line_parts.push(self.sheet.eval(*c, *r).display());
                    }
                    lines.push(line_parts.join("\t"));
                    let text = lines.join("\n");
                    if let Ok(mut clip) = arboard::Clipboard::new() {
                        let _ = clip.set_text(text);
                    }
                }
            }

            // Delete selected cells (Delete/Backspace when not editing)
            if !self.editing && (i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
                if self.has_multi_selection() {
                    let cells = self.selected_cells();
                    for (c, r) in cells {
                        self.sheet.set_input(c, r, String::new());
                    }
                } else {
                    self.sheet.set_input(self.sel_col, self.sel_row, String::new());
                }
            }

            if !self.editing {
                if i.key_pressed(Key::Enter) { self.start_edit(); }

                // Arrow key navigation with shift for range selection
                let mut moved = false;
                let mut new_col = self.sel_col;
                let mut new_row = self.sel_row;

                if i.key_pressed(Key::ArrowUp) && self.sel_row > 0 {
                    new_row = self.sel_row - 1; moved = true;
                }
                if i.key_pressed(Key::ArrowDown) {
                    new_row = self.sel_row + 1; moved = true;
                }
                if i.key_pressed(Key::ArrowLeft) && self.sel_col > 0 {
                    new_col = self.sel_col - 1; moved = true;
                }
                if i.key_pressed(Key::ArrowRight) {
                    new_col = self.sel_col + 1; moved = true;
                }

                if moved {
                    if shift {
                        // Extend range selection
                        if self.range_anchor.is_none() {
                            self.range_anchor = Some((self.sel_col, self.sel_row));
                        }
                    } else {
                        self.clear_selection();
                    }
                    self.sel_col = new_col;
                    self.sel_row = new_row;
                }

                if tab_pressed {
                    self.clear_selection();
                    self.sel_col += 1;
                }

                // Start typing directly — enters edit mode
                for event in &i.events {
                    if let egui::Event::Text(t) = event {
                        if !cmd {
                            self.edit_buf.clear();
                            self.edit_buf.push_str(t);
                            self.editing = true;
                            self.clear_selection();
                        }
                    }
                }
            } else {
                // While editing: accumulate typed text
                for event in &i.events {
                    if let egui::Event::Text(t) = event {
                        if !cmd {
                            self.edit_buf.push_str(t);
                        }
                    }
                }
                if i.key_pressed(Key::Backspace) {
                    self.edit_buf.pop();
                }
                if i.key_pressed(Key::Enter) {
                    self.commit_edit();
                    self.sel_row += 1;
                    self.clear_selection();
                }
                if i.key_pressed(Key::Escape) { self.cancel_edit(); }
                if tab_pressed {
                    self.commit_edit();
                    self.sel_col += 1;
                    self.clear_selection();
                }
            }
        });
    }

    fn render_menu(&mut self, ui: &mut egui::Ui) {
        menu_bar(ui, |ui| {
            ui.menu_button("file", |ui| {
                if ui.button("new       ⌘n").clicked() {
                    self.sheet = Sheet::new();
                    self.clear_selection();
                    ui.close_menu();
                }
                if ui.button("open...   ⌘o").clicked() {
                    self.fb_mode = FbMode::Open; self.show_file_browser = true; ui.close_menu();
                }
                ui.separator();
                if ui.button("save      ⌘s").clicked() { self.save(); ui.close_menu(); }
                if ui.button("save as...").clicked() {
                    self.fb_mode = FbMode::Save;
                    self.save_filename = "untitled.csv".into();
                    self.show_file_browser = true;
                    ui.close_menu();
                }
            });
            ui.menu_button("edit", |ui| {
                let has_sel = self.has_multi_selection();
                if ui.button("delete selected cells").clicked() {
                    if has_sel {
                        let cells = self.selected_cells();
                        for (c, r) in cells {
                            self.sheet.set_input(c, r, String::new());
                        }
                    } else {
                        self.sheet.set_input(self.sel_col, self.sel_row, String::new());
                    }
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("insert =SUM()").clicked() {
                    let ref_str = self.selection_reference();
                    self.edit_buf = format!("=SUM({})", ref_str);
                    self.editing = true;
                    self.formula_bar_focus = true;
                    ui.close_menu();
                }
                if ui.button("insert =AVG()").clicked() {
                    let ref_str = self.selection_reference();
                    self.edit_buf = format!("=AVG({})", ref_str);
                    self.editing = true;
                    self.formula_bar_focus = true;
                    ui.close_menu();
                }
            });
            ui.menu_button("help", |ui| {
                if ui.button("about").clicked() { self.show_about = true; ui.close_menu(); }
            });
        });
    }

    fn render_formula_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Show selection range or active cell address
            let addr_str = if self.has_multi_selection() {
                self.selection_reference()
            } else {
                format!("{}{}", col_letter(self.sel_col), self.sel_row + 1)
            };
            ui.label(egui::RichText::new(addr_str).monospace().strong());
            ui.separator();
            if self.editing {
                let response = ui.text_edit_singleline(&mut self.edit_buf);
                if self.formula_bar_focus {
                    response.request_focus();
                    self.formula_bar_focus = false;
                }
            } else {
                let display = self.sheet.get_input(self.sel_col, self.sel_row);
                ui.label(egui::RichText::new(display).monospace());
            }
        });
    }

    /// Convert pixel position to grid (col, row), accounting for scroll and headers
    fn pos_to_cell(&self, pos: Pos2, grid_rect: Rect, row_header_w: f32, row_height: f32, col_w: f32) -> Option<(usize, usize)> {
        let rel_x = pos.x - grid_rect.min.x - row_header_w;
        let rel_y = pos.y - grid_rect.min.y - row_height; // skip column header row
        if rel_x >= 0.0 && rel_y >= 0.0 {
            let col = (rel_x / col_w) as usize + self.scroll_col;
            let row = (rel_y / row_height) as usize + self.scroll_row;
            Some((col, row))
        } else {
            None
        }
    }

    fn render_grid(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let painter = ui.painter_at(rect);

        let row_height = 22.0;
        let row_header_w = 40.0;
        let default_col_w = 80.0;

        // Background
        painter.rect_filled(rect, 0.0, SlowColors::WHITE);

        let visible_rows = ((rect.height() - row_height) / row_height) as usize;
        let visible_cols = {
            let mut w = row_header_w;
            let mut c = 0;
            while w < rect.width() && c + self.scroll_col < self.sheet.used_cols() + 4 {
                w += default_col_w;
                c += 1;
            }
            c
        };

        // Column headers
        let mut x = rect.min.x + row_header_w;
        for ci in 0..visible_cols {
            let col = ci + self.scroll_col;
            let w = default_col_w;
            let header_rect = Rect::from_min_size(
                egui::pos2(x, rect.min.y), Vec2::new(w, row_height),
            );
            painter.rect_filled(header_rect, 0.0, SlowColors::WHITE);
            painter.rect_stroke(header_rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));
            painter.text(
                header_rect.center(), egui::Align2::CENTER_CENTER,
                format!("{}", col_letter(col)),
                egui::FontId::proportional(12.0), SlowColors::BLACK,
            );
            x += w;
        }

        // Row headers + cells
        for ri in 0..visible_rows {
            let row = ri + self.scroll_row;
            let y = rect.min.y + row_height + ri as f32 * row_height;

            // Row header
            let rh_rect = Rect::from_min_size(
                egui::pos2(rect.min.x, y), Vec2::new(row_header_w, row_height),
            );
            painter.rect_filled(rh_rect, 0.0, SlowColors::WHITE);
            painter.rect_stroke(rh_rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));
            painter.text(
                rh_rect.center(), egui::Align2::CENTER_CENTER,
                format!("{}", row + 1),
                egui::FontId::proportional(11.0), SlowColors::BLACK,
            );

            // Cells
            let mut x = rect.min.x + row_header_w;
            for ci in 0..visible_cols {
                let col = ci + self.scroll_col;
                let w = default_col_w;
                let cell_rect = Rect::from_min_size(egui::pos2(x, y), Vec2::new(w, row_height));

                let is_active = col == self.sel_col && row == self.sel_row;
                let in_selection = self.is_selected(col, row);

                // Draw cell background + border
                if is_active {
                    painter.rect_filled(cell_rect, 0.0, SlowColors::WHITE);
                    // Double-outline for clear active cell visibility
                    painter.rect_stroke(cell_rect, 0.0, Stroke::new(3.0, SlowColors::BLACK));
                    let inner = cell_rect.shrink(3.0);
                    painter.rect_stroke(inner, 0.0, Stroke::new(1.0, SlowColors::BLACK));
                } else {
                    painter.rect_stroke(cell_rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));
                }

                // Draw cell text
                let text = if is_active && self.editing {
                    self.edit_buf.clone()
                } else {
                    self.sheet.eval(col, row).display()
                };

                if !text.is_empty() {
                    let text_pos = egui::pos2(cell_rect.min.x + 4.0, cell_rect.center().y);
                    painter.text(
                        text_pos, egui::Align2::LEFT_CENTER,
                        &text, egui::FontId::proportional(12.0), SlowColors::BLACK,
                    );
                }

                // Draw dithered selection overlay on top (not on active cell — it has thick border)
                if in_selection && !is_active {
                    draw_dither_selection(&painter, cell_rect);
                }

                x += w;
            }
        }

        // --- Mouse interaction ---
        let modifiers = ui.input(|i| i.modifiers);
        let cmd = modifiers.command;
        let shift = modifiers.shift;

        // Click handling
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some((col, row)) = self.pos_to_cell(pos, rect, row_header_w, row_height, default_col_w) {
                    // If clicking the same cell, don't disrupt editing at all
                    if col == self.sel_col && row == self.sel_row {
                        if !self.editing {
                            self.start_edit();
                        }
                    } else {
                        self.commit_edit();

                        if cmd {
                        // Cmd+Click: toggle cell in extra selection
                        if self.extra_cells.contains(&(col, row)) {
                            self.extra_cells.remove(&(col, row));
                        } else {
                            // If first cmd+click, add current active cell to extras first
                            if self.extra_cells.is_empty() && self.range_anchor.is_none() {
                                self.extra_cells.insert((self.sel_col, self.sel_row));
                            }
                            self.extra_cells.insert((col, row));
                        }
                        self.sel_col = col;
                        self.sel_row = row;
                        self.range_anchor = None;
                    } else if shift {
                        // Shift+Click: rectangular range from anchor to here
                        if self.range_anchor.is_none() {
                            self.range_anchor = Some((self.sel_col, self.sel_row));
                        }
                        self.extra_cells.clear();
                        self.sel_col = col;
                        self.sel_row = row;
                    } else {
                        // Plain click: select single cell
                        self.clear_selection();
                        self.sel_col = col;
                        self.sel_row = row;
                    }
                    }
                }
            }
        }

        // Drag handling for range selection
        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some((col, row)) = self.pos_to_cell(pos, rect, row_header_w, row_height, default_col_w) {
                    // Don't start drag on the active cell if editing
                    if self.editing && col == self.sel_col && row == self.sel_row {
                        // Do nothing — let the user continue editing
                    } else if !cmd {
                        self.commit_edit();
                        self.extra_cells.clear();
                        self.drag_start = Some((col, row));
                        self.range_anchor = Some((col, row));
                        self.sel_col = col;
                        self.sel_row = row;
                        self.dragging = true;
                    }
                }
            }
        }

        if self.dragging {
            if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                if let Some((col, row)) = self.pos_to_cell(pos, rect, row_header_w, row_height, default_col_w) {
                    self.sel_col = col;
                    self.sel_row = row;
                }
            }
        }

        if response.drag_stopped() {
            self.dragging = false;
            // If drag ended at same cell as start, it's a single click — clear range
            if let Some((sc, sr)) = self.drag_start {
                if sc == self.sel_col && sr == self.sel_row {
                    self.range_anchor = None;
                }
            }
            self.drag_start = None;
        }

        // Scroll
        ui.input(|i| {
            let scroll = i.raw_scroll_delta.y;
            if scroll < 0.0 && self.scroll_row < MAX_ROWS {
                self.scroll_row += 3;
            } else if scroll > 0.0 && self.scroll_row >= 3 {
                self.scroll_row -= 3;
            }
        });
    }

    fn render_file_browser(&mut self, ctx: &Context) {
        let title = if self.fb_mode == FbMode::Open { "open spreadsheet" } else { "save spreadsheet" };
        egui::Window::new(title).collapsible(false).resizable(false).default_width(380.0)
            .show(ctx, |ui| {
                ui.label(self.file_browser.current_dir.to_string_lossy().to_string());
                ui.separator();
                egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
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
                if self.fb_mode == FbMode::Save {
                    ui.separator();
                    ui.horizontal(|ui| { ui.label("filename:"); ui.text_edit_singleline(&mut self.save_filename); });
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("cancel").clicked() { self.show_file_browser = false; }
                    if ui.button(if self.fb_mode == FbMode::Open { "open" } else { "save" }).clicked() {
                        match self.fb_mode {
                            FbMode::Open => {
                                if let Some(e) = self.file_browser.selected_entry() {
                                    if !e.is_directory {
                                        let p = e.path.clone();
                                        self.open_file(p);
                                        self.show_file_browser = false;
                                    }
                                }
                            }
                            FbMode::Save => {
                                if !self.save_filename.is_empty() {
                                    let p = self.file_browser.save_directory().join(&self.save_filename);
                                    if self.save_filename.ends_with(".json") {
                                        let _ = self.sheet.save_json(&p);
                                    } else {
                                        let _ = self.sheet.save_csv(&p);
                                    }
                                    self.show_file_browser = false;
                                }
                            }
                        }
                    }
                });
            });
    }
}

const MAX_ROWS: usize = crate::sheet::MAX_ROWS;

/// Try to compact a set of cells into a range string like "A1:B3"
/// if they form a perfect rectangle; otherwise return None.
fn try_compact_range(cells: &[(usize, usize)]) -> Option<String> {
    if cells.is_empty() { return None; }
    let min_c = cells.iter().map(|c| c.0).min().unwrap();
    let max_c = cells.iter().map(|c| c.0).max().unwrap();
    let min_r = cells.iter().map(|c| c.1).min().unwrap();
    let max_r = cells.iter().map(|c| c.1).max().unwrap();

    let expected_count = (max_c - min_c + 1) * (max_r - min_r + 1);
    if cells.len() == expected_count {
        if min_c == max_c && min_r == max_r {
            Some(CellAddr::new(min_c, min_r).label())
        } else {
            Some(format!("{}:{}", CellAddr::new(min_c, min_r).label(), CellAddr::new(max_c, max_r).label()))
        }
    } else {
        None
    }
}

impl eframe::App for SlowSheetsApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        slowcore::theme::consume_special_keys(ctx);
        self.handle_keys(ctx);

        egui::TopBottomPanel::top("menu").show(ctx, |ui| self.render_menu(ui));
        egui::TopBottomPanel::top("title").show(ctx, |ui| {
            slowcore::theme::SlowTheme::title_bar_frame().show(ui, |ui| {
                ui.centered_and_justified(|ui| ui.label(self.sheet.display_title()));
            });
        });
        egui::TopBottomPanel::top("formula").show(ctx, |ui| self.render_formula_bar(ui));
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let status_text = if self.has_multi_selection() {
                let cells = self.selected_cells();
                let vals = self.selected_numeric_values();
                let count = cells.len();
                if vals.is_empty() {
                    format!("{} cells selected", count)
                } else {
                    let sum: f64 = vals.iter().sum();
                    let avg = sum / vals.len() as f64;
                    format!(
                        "{} cells  |  sum: {}  avg: {}  count: {}",
                        count,
                        format_number(sum),
                        format_number(avg),
                        vals.len()
                    )
                }
            } else {
                let val = self.sheet.eval(self.sel_col, self.sel_row).display();
                format!(
                    "{}{}: {}  |  {} cells used",
                    col_letter(self.sel_col), self.sel_row + 1, val,
                    self.sheet.cells.len()
                )
            };
            status_bar(ui, &status_text);
        });
        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| self.render_grid(ui));

        if self.show_file_browser { self.render_file_browser(ctx); }
        if self.show_about {
            egui::Window::new("about slowSheets")
                .collapsible(false)
                .resizable(false)
                .default_width(300.0)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("slowSheets");
                        ui.label("version 0.2.0");
                        ui.add_space(8.0);
                        ui.label("spreadsheet for slowOS");
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label("supported formats:");
                    ui.label("  CSV (.csv)");
                    ui.add_space(4.0);
                    ui.label("features:");
                    ui.label("  formulas (SUM, AVG, etc.)");
                    ui.label("  multi-cell selection, sorting");
                    ui.add_space(4.0);
                    ui.label("frameworks:");
                    ui.label("  egui/eframe (MIT), serde (MIT)");
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        if ui.button("ok").clicked() { self.show_about = false; }
                    });
                });
        }
    }
}

fn format_number(n: f64) -> String {
    if n == n.floor() && n.abs() < 1e12 {
        format!("{}", n as i64)
    } else {
        format!("{:.2}", n)
    }
}
