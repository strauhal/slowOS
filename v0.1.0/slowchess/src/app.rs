//! SlowChess application

use crate::chess::*;
use egui::{Context, Rect, Sense, Stroke, Vec2};
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::status_bar;
use rand::seq::SliceRandom;

pub struct SlowChessApp {
    board: Board,
    selected: Option<Pos>,
    legal_highlights: Vec<Pos>,
    vs_computer: bool,
    computer_color: Color,
    show_about: bool,
    last_move: Option<(Pos, Pos)>,
}

impl SlowChessApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            board: Board::new(),
            selected: None,
            legal_highlights: Vec::new(),
            vs_computer: true,
            computer_color: Color::Black,
            show_about: false,
            last_move: None,
        }
    }

    fn new_game(&mut self) {
        self.board = Board::new();
        self.selected = None;
        self.legal_highlights.clear();
        self.last_move = None;
    }

    fn computer_move(&mut self) {
        if self.board.turn != self.computer_color { return; }
        if self.board.state == GameState::Checkmate || self.board.state == GameState::Stalemate { return; }

        // Collect all legal moves
        let mut all_moves: Vec<(Pos, Pos)> = Vec::new();
        for r in 0..8 {
            for c in 0..8 {
                if let Some(p) = self.board.get((r, c)) {
                    if p.color == self.computer_color {
                        let moves = self.board.legal_moves((r, c));
                        for to in moves {
                            all_moves.push(((r, c), to));
                        }
                    }
                }
            }
        }

        if all_moves.is_empty() { return; }

        // Simple evaluation: prefer captures, checks
        let mut scored: Vec<(i32, (Pos, Pos))> = all_moves.iter().map(|&(from, to)| {
            let mut score = 0i32;
            // Capture value
            if let Some(captured) = self.board.get(to) {
                score += match captured.kind {
                    PieceKind::Queen => 900,
                    PieceKind::Rook => 500,
                    PieceKind::Bishop | PieceKind::Knight => 300,
                    PieceKind::Pawn => 100,
                    PieceKind::King => 0,
                };
            }
            // Center control
            if (to.0 == 3 || to.0 == 4) && (to.1 == 3 || to.1 == 4) { score += 20; }
            // Random factor
            score += (rand::random::<u8>() % 30) as i32;
            (score, (from, to))
        }).collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        let (from, to) = scored[0].1;
        self.last_move = Some((from, to));
        self.board.make_move(from, to);
    }

    fn handle_click(&mut self, pos: Pos) {
        if self.board.state == GameState::Checkmate || self.board.state == GameState::Stalemate {
            return;
        }

        if self.vs_computer && self.board.turn == self.computer_color {
            return;
        }

        if let Some(from) = self.selected {
            if self.legal_highlights.contains(&pos) {
                self.last_move = Some((from, pos));
                self.board.make_move(from, pos);
                self.selected = None;
                self.legal_highlights.clear();

                // Computer responds
                if self.vs_computer {
                    self.computer_move();
                }
            } else {
                // Select new piece
                self.selected = None;
                self.legal_highlights.clear();
                if let Some(p) = self.board.get(pos) {
                    if p.color == self.board.turn {
                        let moves = self.board.legal_moves(pos);
                        if !moves.is_empty() {
                            self.selected = Some(pos);
                            self.legal_highlights = moves;
                        }
                    }
                }
            }
        } else {
            if let Some(p) = self.board.get(pos) {
                if p.color == self.board.turn {
                    let moves = self.board.legal_moves(pos);
                    if !moves.is_empty() {
                        self.selected = Some(pos);
                        self.legal_highlights = moves;
                    }
                }
            }
        }
    }

    fn render_board(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_rect_before_wrap();
        let board_size = available.width().min(available.height() - 20.0).min(560.0);
        let sq_size = board_size / 8.0;

        let board_rect = Rect::from_min_size(
            egui::pos2(
                available.center().x - board_size / 2.0,
                available.min.y + 10.0,
            ),
            Vec2::splat(board_size),
        );

        let response = ui.allocate_rect(board_rect, Sense::click());
        let painter = ui.painter_at(board_rect);

        // Draw squares
        for r in 0..8 {
            for c in 0..8 {
                let sq_rect = Rect::from_min_size(
                    egui::pos2(board_rect.min.x + c as f32 * sq_size, board_rect.min.y + r as f32 * sq_size),
                    Vec2::splat(sq_size),
                );

                let is_light = (r + c) % 2 == 0;

                // white base
                painter.rect_filled(sq_rect, 0.0, SlowColors::WHITE);

                // dark squares get dither
                if !is_light {
                    slowcore::dither::draw_dither_rect(&painter, sq_rect, SlowColors::BLACK, 2);
                }

                // highlight selected with dense dither
                if self.selected == Some((r, c)) {
                    slowcore::dither::draw_dither_selection(&painter, sq_rect);
                }
                // highlight last move with light dither
                if let Some((from, to)) = self.last_move {
                    if (r, c) == from || (r, c) == to {
                        slowcore::dither::draw_dither_hover(&painter, sq_rect);
                    }
                }

                painter.rect_stroke(sq_rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));

                // Legal move dots
                if self.legal_highlights.contains(&(r, c)) {
                    if self.board.get((r, c)).is_some() {
                        // Capture: ring
                        painter.circle_stroke(sq_rect.center(), sq_size * 0.4, Stroke::new(3.0, SlowColors::BLACK));
                    } else {
                        // Move: dot
                        painter.circle_filled(sq_rect.center(), sq_size * 0.15, SlowColors::BLACK);
                    }
                }

                // Draw piece
                if let Some(piece) = self.board.get((r, c)) {
                    painter.text(
                        sq_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        piece.symbol(),
                        egui::FontId::proportional(sq_size * 0.7),
                        SlowColors::BLACK,
                    );
                }
            }
        }

        // Border
        painter.rect_stroke(board_rect, 0.0, Stroke::new(2.0, SlowColors::BLACK));

        // File/rank labels
        for i in 0..8 {
            let file = (b'a' + i as u8) as char;
            painter.text(
                egui::pos2(board_rect.min.x + i as f32 * sq_size + sq_size / 2.0, board_rect.max.y + 10.0),
                egui::Align2::CENTER_TOP,
                format!("{}", file),
                egui::FontId::proportional(11.0),
                SlowColors::BLACK,
            );
            painter.text(
                egui::pos2(board_rect.min.x - 12.0, board_rect.min.y + i as f32 * sq_size + sq_size / 2.0),
                egui::Align2::CENTER_CENTER,
                format!("{}", 8 - i),
                egui::FontId::proportional(11.0),
                SlowColors::BLACK,
            );
        }

        // Handle clicks
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let col = ((pos.x - board_rect.min.x) / sq_size) as usize;
                let row = ((pos.y - board_rect.min.y) / sq_size) as usize;
                if row < 8 && col < 8 {
                    self.handle_click((row, col));
                }
            }
        }
    }
}

impl eframe::App for SlowChessApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Consume Tab to prevent menu hover
        ctx.input_mut(|i| {
            if i.key_pressed(egui::Key::Tab) {
                i.events.retain(|e| !matches!(e, egui::Event::Key { key: egui::Key::Tab, .. }));
            }
        });
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            menu_bar(ui, |ui| {
                ui.menu_button("game", |ui| {
                    if ui.button("new game").clicked() { self.new_game(); ui.close_menu(); }
                    ui.separator();
                    if ui.button(if self.vs_computer { "✓ vs Computer" } else { "  vs Computer" }).clicked() {
                        self.vs_computer = true; self.new_game(); ui.close_menu();
                    }
                    if ui.button(if !self.vs_computer { "✓ Two Player" } else { "  Two Player" }).clicked() {
                        self.vs_computer = false; self.new_game(); ui.close_menu();
                    }
                });
                ui.menu_button("help", |ui| {
                    if ui.button("about").clicked() { self.show_about = true; ui.close_menu(); }
                });
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let state_text = match self.board.state {
                GameState::Playing => format!("{}'s turn", if self.board.turn == Color::White { "white" } else { "black" }),
                GameState::Check => format!("{} is in check!", if self.board.turn == Color::White { "white" } else { "black" }),
                GameState::Checkmate => format!("checkmate! {} wins!", if self.board.turn == Color::White { "black" } else { "white" }),
                GameState::Stalemate => "stalemate — draw!".into(),
            };
            let move_count = self.board.move_history.len();
            status_bar(ui, &format!("{}  |  Move {}", state_text, move_count));
        });

        egui::CentralPanel::default().frame(
            egui::Frame::none().fill(SlowColors::WHITE).inner_margin(egui::Margin::same(20.0))
        ).show(ctx, |ui| {
            self.render_board(ui);
        });

        if self.show_about {
            egui::Window::new("about slowChess")
                .collapsible(false)
                .resizable(false)
                .default_width(300.0)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("slowChess");
                        ui.label("version 0.1.0");
                        ui.add_space(8.0);
                        ui.label("chess game for e-ink");
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label("features:");
                    ui.label("  play against AI opponent");
                    ui.label("  legal move highlighting");
                    ui.label("  undo moves");
                    ui.add_space(4.0);
                    ui.label("frameworks:");
                    ui.label("  egui/eframe (MIT)");
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        if ui.button("ok").clicked() { self.show_about = false; }
                    });
                });
        }
    }
}
