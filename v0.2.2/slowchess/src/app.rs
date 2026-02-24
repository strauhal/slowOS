//! SlowChess application

use crate::chess::*;
use egui::{ColorImage, Context, Rect, Sense, Stroke, TextureHandle, TextureOptions, Vec2};
use serde::{Deserialize, Serialize};
use slowcore::repaint::RepaintController;
use slowcore::theme::{menu_bar, SlowColors};
use slowcore::widgets::{status_bar, window_control_buttons, WindowAction};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Saved game state for persistence
#[derive(Serialize, Deserialize)]
struct SavedState {
    board: Board,
    vs_computer: bool,
    computer_color: Color,
    ai_difficulty: u8,
    last_move: Option<(Pos, Pos)>,
}

fn save_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("slowchess_save.json")
}

pub struct SlowChessApp {
    board: Board,
    selected: Option<Pos>,
    legal_highlights: Vec<Pos>,
    vs_computer: bool,
    computer_color: Color,
    show_about: bool,
    last_move: Option<(Pos, Pos)>,
    /// AI difficulty: 1 = easy (random), 5 = hardest (best moves)
    ai_difficulty: u8,
    /// AI thinking state
    ai_thinking: bool,
    ai_think_start: Option<Instant>,
    ai_pending_move: Option<(Pos, Pos)>,
    /// Chess piece icon textures (keyed by "white_king", "black_pawn", etc.)
    piece_icons: HashMap<String, TextureHandle>,
    icons_loaded: bool,
    repaint: RepaintController,
}

impl SlowChessApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Try to load saved game
        if let Some(saved) = Self::load_saved_state() {
            return Self {
                board: saved.board,
                selected: None,
                legal_highlights: Vec::new(),
                vs_computer: saved.vs_computer,
                computer_color: saved.computer_color,
                show_about: false,
                last_move: saved.last_move,
                ai_difficulty: saved.ai_difficulty,
                ai_thinking: false,
                ai_think_start: None,
                ai_pending_move: None,
                piece_icons: HashMap::new(),
                icons_loaded: false,
                repaint: RepaintController::new(),
            };
        }

        Self {
            board: Board::new(),
            selected: None,
            legal_highlights: Vec::new(),
            vs_computer: true,
            computer_color: Color::Black,
            show_about: false,
            last_move: None,
            ai_difficulty: 3, // Default: medium
            ai_thinking: false,
            ai_think_start: None,
            ai_pending_move: None,
            piece_icons: HashMap::new(),
            icons_loaded: false,
            repaint: RepaintController::new(),
        }
    }

    /// Load chess piece icons from separate white and black folders
    fn ensure_piece_icons(&mut self, ctx: &Context) {
        if self.icons_loaded {
            return;
        }
        self.icons_loaded = true;

        // White piece icons
        let white_icons: &[(&str, &[u8])] = &[
            ("white_king", include_bytes!("../../icons/chess_icons/white/icons_king.png")),
            ("white_queen", include_bytes!("../../icons/chess_icons/white/icons_queen.png")),
            ("white_rook", include_bytes!("../../icons/chess_icons/white/icons_rook.png")),
            ("white_bishop", include_bytes!("../../icons/chess_icons/white/icons_bishop.png")),
            ("white_knight", include_bytes!("../../icons/chess_icons/white/icons_knight.png")),
            ("white_pawn", include_bytes!("../../icons/chess_icons/white/icons_pawn.png")),
        ];

        // Black piece icons
        let black_icons: &[(&str, &[u8])] = &[
            ("black_king", include_bytes!("../../icons/chess_icons/black/icons_king.png")),
            ("black_queen", include_bytes!("../../icons/chess_icons/black/icons_queen.png")),
            ("black_rook", include_bytes!("../../icons/chess_icons/black/icons_rook.png")),
            ("black_bishop", include_bytes!("../../icons/chess_icons/black/icons_bishop.png")),
            ("black_knight", include_bytes!("../../icons/chess_icons/black/icons_knight.png")),
            ("black_pawn", include_bytes!("../../icons/chess_icons/black/icons_pawn.png")),
        ];

        // Load all icons
        for (key, png_bytes) in white_icons.iter().chain(black_icons.iter()) {
            if let Ok(img) = image::load_from_memory(png_bytes) {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let color_image = ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    rgba.as_raw(),
                );
                let tex = ctx.load_texture(
                    format!("chess_{}", key),
                    color_image,
                    TextureOptions::NEAREST,
                );
                self.piece_icons.insert(key.to_string(), tex);
            }
        }
    }

    /// Get the texture key for a piece
    fn piece_texture_key(piece: &Piece) -> String {
        let color = match piece.color {
            Color::White => "white",
            Color::Black => "black",
        };
        let kind = match piece.kind {
            PieceKind::King => "king",
            PieceKind::Queen => "queen",
            PieceKind::Rook => "rook",
            PieceKind::Bishop => "bishop",
            PieceKind::Knight => "knight",
            PieceKind::Pawn => "pawn",
        };
        format!("{}_{}", color, kind)
    }

    fn load_saved_state() -> Option<SavedState> {
        let path = save_path();
        let data = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&data).ok()
    }

    fn save_state(&self) {
        let saved = SavedState {
            board: self.board.clone(),
            vs_computer: self.vs_computer,
            computer_color: self.computer_color,
            ai_difficulty: self.ai_difficulty,
            last_move: self.last_move,
        };
        if let Ok(json) = serde_json::to_string_pretty(&saved) {
            let _ = std::fs::write(save_path(), json);
        }
    }

    fn new_game(&mut self) {
        self.board = Board::new();
        self.selected = None;
        self.legal_highlights.clear();
        self.last_move = None;
        self.ai_thinking = false;
        self.ai_think_start = None;
        self.ai_pending_move = None;
    }

    /// Get think duration based on difficulty (higher = thinks longer)
    fn think_duration(&self) -> Duration {
        match self.ai_difficulty {
            1 => Duration::from_millis(400),
            2 => Duration::from_millis(700),
            3 => Duration::from_millis(1000),
            4 => Duration::from_millis(1500),
            _ => Duration::from_millis(2000),
        }
    }

    /// Get AI thinking progress (0.0 to 1.0)
    fn ai_progress(&self) -> f32 {
        if let Some(start) = self.ai_think_start {
            let elapsed = start.elapsed().as_secs_f32();
            let total = self.think_duration().as_secs_f32();
            (elapsed / total).min(1.0)
        } else {
            0.0
        }
    }

    /// Start AI thinking process
    fn start_computer_think(&mut self) {
        if self.board.turn != self.computer_color { return; }
        if self.board.state == GameState::Checkmate || self.board.state == GameState::Stalemate { return; }
        if self.ai_thinking { return; }

        // Start the thinking animation BEFORE calculating (so progress bar shows immediately)
        self.ai_thinking = true;
        self.ai_think_start = Some(Instant::now());
        self.ai_pending_move = None; // Will be calculated on next frame
    }

    /// Calculate AI move if not yet calculated (called during update)
    fn ensure_ai_move_calculated(&mut self) {
        if !self.ai_thinking { return; }
        if self.ai_pending_move.is_some() { return; } // Already calculated

        // Calculate the move
        if let Some(mv) = self.calculate_best_move() {
            self.ai_pending_move = Some(mv);
        } else {
            // No valid moves - stop thinking
            self.ai_thinking = false;
            self.ai_think_start = None;
        }
    }

    /// Calculate best move using minimax with alpha-beta pruning
    fn calculate_best_move(&self) -> Option<(Pos, Pos)> {
        // Search depth based on difficulty
        let depth = match self.ai_difficulty {
            1 => 1,  // Easy: only look 1 move ahead
            2 => 2,  // Beginner: 2 moves ahead
            3 => 3,  // Medium: 3 moves ahead
            4 => 4,  // Hard: 4 moves ahead
            _ => 5,  // Expert: 5 moves ahead (very strong)
        };

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

        if all_moves.is_empty() { return None; }

        // Order moves to improve alpha-beta pruning (captures first)
        all_moves.sort_by(|a, b| {
            let score_a = self.move_order_score(&self.board, *a);
            let score_b = self.move_order_score(&self.board, *b);
            score_b.cmp(&score_a)
        });

        let mut best_move = all_moves[0];
        let mut best_score = i32::MIN;

        for mv in &all_moves {
            let mut test_board = self.board.clone();
            test_board.make_move(mv.0, mv.1);

            let score = -self.minimax(&test_board, depth - 1, i32::MIN + 1, i32::MAX, self.computer_color.opposite());

            if score > best_score {
                best_score = score;
                best_move = *mv;
            }
        }

        // On lower difficulties, occasionally make suboptimal moves
        if self.ai_difficulty < 4 {
            let random_chance = match self.ai_difficulty {
                1 => 40,  // 40% chance of random move
                2 => 20,  // 20% chance
                3 => 8,   // 8% chance
                _ => 0,
            };
            if (rand::random::<u8>() % 100) < random_chance && all_moves.len() > 1 {
                let idx = rand::random::<usize>() % all_moves.len();
                return Some(all_moves[idx]);
            }
        }

        Some(best_move)
    }

    /// Minimax with alpha-beta pruning
    fn minimax(&self, board: &Board, depth: i32, mut alpha: i32, beta: i32, color: Color) -> i32 {
        // Terminal conditions
        if depth == 0 || board.state == GameState::Checkmate || board.state == GameState::Stalemate {
            return self.evaluate_board(board, color);
        }

        // Collect all legal moves for current color
        let mut moves: Vec<(Pos, Pos)> = Vec::new();
        for r in 0..8 {
            for c in 0..8 {
                if let Some(p) = board.get((r, c)) {
                    if p.color == color {
                        // Need to check legal moves with board's turn temporarily set
                        let mut temp_board = board.clone();
                        temp_board.turn = color;
                        let piece_moves = temp_board.legal_moves((r, c));
                        for to in piece_moves {
                            moves.push(((r, c), to));
                        }
                    }
                }
            }
        }

        if moves.is_empty() {
            // No moves: checkmate or stalemate
            let in_check = board.in_check(color);
            if in_check {
                return -100000 + (5 - depth); // Prefer faster checkmates
            } else {
                return 0; // Stalemate
            }
        }

        // Order moves to improve pruning
        moves.sort_by(|a, b| {
            let score_a = self.move_order_score(board, *a);
            let score_b = self.move_order_score(board, *b);
            score_b.cmp(&score_a)
        });

        let mut best_score = i32::MIN;

        for mv in moves {
            let mut test_board = board.clone();
            test_board.turn = color; // Ensure correct turn
            test_board.make_move(mv.0, mv.1);

            let score = -self.minimax(&test_board, depth - 1, -beta, -alpha, color.opposite());

            best_score = best_score.max(score);
            alpha = alpha.max(score);

            if alpha >= beta {
                break; // Beta cutoff
            }
        }

        best_score
    }

    /// Move ordering heuristic: prioritize captures, checks, center moves
    fn move_order_score(&self, board: &Board, mv: (Pos, Pos)) -> i32 {
        let (_, to) = mv;
        let mut score = 0;

        // Captures are very valuable to search first
        if let Some(captured) = board.get(to) {
            score += match captured.kind {
                PieceKind::Queen => 900,
                PieceKind::Rook => 500,
                PieceKind::Bishop | PieceKind::Knight => 300,
                PieceKind::Pawn => 100,
                PieceKind::King => 10000,
            };
        }

        // Center control
        if (to.0 == 3 || to.0 == 4) && (to.1 == 3 || to.1 == 4) {
            score += 20;
        }

        score
    }

    /// Evaluate board position from the perspective of the given color
    fn evaluate_board(&self, board: &Board, perspective: Color) -> i32 {
        let mut score = 0i32;

        // Piece values
        const PAWN_VALUE: i32 = 100;
        const KNIGHT_VALUE: i32 = 320;
        const BISHOP_VALUE: i32 = 330;
        const ROOK_VALUE: i32 = 500;
        const QUEEN_VALUE: i32 = 900;
        const KING_VALUE: i32 = 20000;

        // Piece-square tables for positional evaluation
        const PAWN_TABLE: [[i32; 8]; 8] = [
            [0,  0,  0,  0,  0,  0,  0,  0],
            [50, 50, 50, 50, 50, 50, 50, 50],
            [10, 10, 20, 30, 30, 20, 10, 10],
            [5,  5, 10, 25, 25, 10,  5,  5],
            [0,  0,  0, 20, 20,  0,  0,  0],
            [5, -5,-10,  0,  0,-10, -5,  5],
            [5, 10, 10,-20,-20, 10, 10,  5],
            [0,  0,  0,  0,  0,  0,  0,  0]
        ];

        const KNIGHT_TABLE: [[i32; 8]; 8] = [
            [-50,-40,-30,-30,-30,-30,-40,-50],
            [-40,-20,  0,  0,  0,  0,-20,-40],
            [-30,  0, 10, 15, 15, 10,  0,-30],
            [-30,  5, 15, 20, 20, 15,  5,-30],
            [-30,  0, 15, 20, 20, 15,  0,-30],
            [-30,  5, 10, 15, 15, 10,  5,-30],
            [-40,-20,  0,  5,  5,  0,-20,-40],
            [-50,-40,-30,-30,-30,-30,-40,-50]
        ];

        const BISHOP_TABLE: [[i32; 8]; 8] = [
            [-20,-10,-10,-10,-10,-10,-10,-20],
            [-10,  0,  0,  0,  0,  0,  0,-10],
            [-10,  0,  5, 10, 10,  5,  0,-10],
            [-10,  5,  5, 10, 10,  5,  5,-10],
            [-10,  0, 10, 10, 10, 10,  0,-10],
            [-10, 10, 10, 10, 10, 10, 10,-10],
            [-10,  5,  0,  0,  0,  0,  5,-10],
            [-20,-10,-10,-10,-10,-10,-10,-20]
        ];

        const ROOK_TABLE: [[i32; 8]; 8] = [
            [0,  0,  0,  0,  0,  0,  0,  0],
            [5, 10, 10, 10, 10, 10, 10,  5],
            [-5,  0,  0,  0,  0,  0,  0, -5],
            [-5,  0,  0,  0,  0,  0,  0, -5],
            [-5,  0,  0,  0,  0,  0,  0, -5],
            [-5,  0,  0,  0,  0,  0,  0, -5],
            [-5,  0,  0,  0,  0,  0,  0, -5],
            [0,  0,  0,  5,  5,  0,  0,  0]
        ];

        const QUEEN_TABLE: [[i32; 8]; 8] = [
            [-20,-10,-10, -5, -5,-10,-10,-20],
            [-10,  0,  0,  0,  0,  0,  0,-10],
            [-10,  0,  5,  5,  5,  5,  0,-10],
            [-5,  0,  5,  5,  5,  5,  0, -5],
            [0,  0,  5,  5,  5,  5,  0, -5],
            [-10,  5,  5,  5,  5,  5,  0,-10],
            [-10,  0,  5,  0,  0,  0,  0,-10],
            [-20,-10,-10, -5, -5,-10,-10,-20]
        ];

        const KING_MIDDLE_TABLE: [[i32; 8]; 8] = [
            [-30,-40,-40,-50,-50,-40,-40,-30],
            [-30,-40,-40,-50,-50,-40,-40,-30],
            [-30,-40,-40,-50,-50,-40,-40,-30],
            [-30,-40,-40,-50,-50,-40,-40,-30],
            [-20,-30,-30,-40,-40,-30,-30,-20],
            [-10,-20,-20,-20,-20,-20,-20,-10],
            [20, 20,  0,  0,  0,  0, 20, 20],
            [20, 30, 10,  0,  0, 10, 30, 20]
        ];

        for r in 0..8 {
            for c in 0..8 {
                if let Some(piece) = board.get((r, c)) {
                    let row = if piece.color == Color::White { r } else { 7 - r };
                    let (piece_value, position_value) = match piece.kind {
                        PieceKind::Pawn => (PAWN_VALUE, PAWN_TABLE[row][c]),
                        PieceKind::Knight => (KNIGHT_VALUE, KNIGHT_TABLE[row][c]),
                        PieceKind::Bishop => (BISHOP_VALUE, BISHOP_TABLE[row][c]),
                        PieceKind::Rook => (ROOK_VALUE, ROOK_TABLE[row][c]),
                        PieceKind::Queen => (QUEEN_VALUE, QUEEN_TABLE[row][c]),
                        PieceKind::King => (KING_VALUE, KING_MIDDLE_TABLE[row][c]),
                    };

                    let total = piece_value + position_value;
                    if piece.color == perspective {
                        score += total;
                    } else {
                        score -= total;
                    }
                }
            }
        }

        // Check and checkmate bonuses
        if board.state == GameState::Checkmate {
            // If it's the opponent's turn and checkmate, we won
            if board.turn != perspective {
                score += 100000;
            } else {
                score -= 100000;
            }
        } else if board.state == GameState::Check {
            if board.turn != perspective {
                score += 50; // Good to have opponent in check
            }
        }

        // Mobility bonus: more legal moves is better
        let mut our_mobility = 0;
        let mut their_mobility = 0;
        for r in 0..8 {
            for c in 0..8 {
                if let Some(p) = board.get((r, c)) {
                    let mut temp = board.clone();
                    temp.turn = p.color;
                    let moves = temp.legal_moves((r, c)).len() as i32;
                    if p.color == perspective {
                        our_mobility += moves;
                    } else {
                        their_mobility += moves;
                    }
                }
            }
        }
        score += (our_mobility - their_mobility) * 2;

        score
    }

    /// Check if AI is done thinking and execute the move
    fn update_ai_thinking(&mut self) {
        if !self.ai_thinking { return; }

        // Calculate move if not yet done (deferred from start_computer_think)
        self.ensure_ai_move_calculated();

        if let Some(start) = self.ai_think_start {
            // Only execute after both: move is calculated AND minimum think time elapsed
            if self.ai_pending_move.is_some() && start.elapsed() >= self.think_duration() {
                // Execute the pending move
                if let Some((from, to)) = self.ai_pending_move {
                    self.last_move = Some((from, to));
                    self.board.make_move(from, to);
                }
                self.ai_thinking = false;
                self.ai_think_start = None;
                self.ai_pending_move = None;
            }
        }
    }

    fn handle_click(&mut self, pos: Pos) {
        if self.board.state == GameState::Checkmate || self.board.state == GameState::Stalemate {
            return;
        }

        // Don't allow moves while AI is thinking
        if self.ai_thinking {
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

                // Computer starts thinking
                if self.vs_computer {
                    self.start_computer_think();
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
        let board_size = available.width().min(available.height() - 40.0).min(560.0);
        let sq_size = board_size / 8.0;

        // AI thinking progress bar at top
        let progress_height = 8.0;
        let progress_rect = Rect::from_min_size(
            egui::pos2(
                available.center().x - board_size / 2.0,
                available.min.y + 2.0,
            ),
            Vec2::new(board_size, progress_height),
        );

        let painter = ui.painter();
        if self.ai_thinking {
            // Draw progress bar background
            painter.rect_filled(progress_rect, 0.0, SlowColors::WHITE);
            painter.rect_stroke(progress_rect, 0.0, Stroke::new(1.0, SlowColors::BLACK));
            // Draw progress fill
            let progress = self.ai_progress();
            let fill_width = progress_rect.width() * progress;
            let fill_rect = Rect::from_min_size(progress_rect.min, Vec2::new(fill_width, progress_height));
            painter.rect_filled(fill_rect, 0.0, SlowColors::BLACK);
        }

        let board_rect = Rect::from_min_size(
            egui::pos2(
                available.center().x - board_size / 2.0,
                available.min.y + progress_height + 8.0,
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

                // Draw piece using icon textures
                if let Some(piece) = self.board.get((r, c)) {
                    let key = Self::piece_texture_key(&piece);
                    if let Some(tex) = self.piece_icons.get(&key) {
                        // Center the piece icon in the square with some padding
                        let icon_size = sq_size * 0.75;
                        let icon_rect = Rect::from_center_size(
                            sq_rect.center(),
                            Vec2::splat(icon_size),
                        );
                        painter.image(
                            tex.id(),
                            icon_rect,
                            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    } else {
                        // Fallback to text if icon not loaded
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
        self.repaint.begin_frame(ctx);

        // Load piece icons if not loaded yet
        self.ensure_piece_icons(ctx);

        // Update AI thinking state
        self.update_ai_thinking();

        // Enable continuous repaint while AI is thinking (for smooth progress bar)
        self.repaint.set_continuous(self.ai_thinking);

        slowcore::theme::consume_special_keys(ctx);
        let mut win_action = WindowAction::None;
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            menu_bar(ui, |ui| {
                win_action = window_control_buttons(ui);
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
        match win_action {
            WindowAction::Close => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            WindowAction::Minimize => {
                slowcore::minimize::write_minimized("slowchess", "chess");
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            WindowAction::None => {}
        }

        // Toolbar with restart button and AI difficulty slider
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("restart").clicked() {
                    self.new_game();
                }

                ui.separator();

                if self.vs_computer {
                    ui.label("AI:");
                    // Custom difficulty bar (like volume slider in slowMusic)
                    let desired = egui::vec2(100.0, 18.0);
                    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click_and_drag());
                    if ui.is_rect_visible(rect) {
                        let painter = ui.painter();
                        // Background
                        painter.rect_filled(rect, 0.0, SlowColors::WHITE);
                        painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.0, SlowColors::BLACK));
                        // Filled portion based on difficulty (1-5 mapped to 0.0-1.0)
                        let fill_pct = (self.ai_difficulty as f32 - 1.0) / 4.0;
                        let fill_w = rect.width() * fill_pct;
                        let fill_rect = egui::Rect::from_min_size(rect.min, egui::vec2(fill_w, rect.height()));
                        painter.rect_filled(fill_rect, 0.0, SlowColors::BLACK);
                    }
                    // Handle click/drag to set difficulty
                    if response.clicked() || response.dragged() {
                        if let Some(pos) = response.interact_pointer_pos() {
                            let rel = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                            // Map 0.0-1.0 to 1-5
                            self.ai_difficulty = ((rel * 4.0).round() as u8 + 1).clamp(1, 5);
                        }
                    }
                    // Label to the side
                    ui.label(match self.ai_difficulty {
                        1 => "easy",
                        2 => "beginner",
                        3 => "medium",
                        4 => "hard",
                        _ => "expert",
                    });
                } else {
                    ui.label("two player mode");
                }
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            let state_text = match self.board.state {
                GameState::Playing => format!("{}'s turn", if self.board.turn == Color::White { "white" } else { "black" }),
                GameState::Check => format!("{} is in check!", if self.board.turn == Color::White { "white" } else { "black" }),
                GameState::Checkmate => format!("checkmate! {} wins!", if self.board.turn == Color::White { "black" } else { "white" }),
                GameState::Stalemate => "stalemate — draw! (no legal moves)".into(),
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
            let screen = ctx.screen_rect();
            let max_h = (screen.height() - 60.0).max(120.0);
            let resp = egui::Window::new("about slowChess")
                .collapsible(false)
                .resizable(false)
                .default_width(300.0)
                .max_height(max_h)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().max_height(max_h - 50.0).show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.heading("slowChess");
                            ui.label("version 0.2.2");
                            ui.add_space(8.0);
                            ui.label("chess game for slowOS");
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
                });
            if let Some(r) = &resp { slowcore::dither::draw_window_shadow_large(ctx, r.response.rect); }
        }

        self.repaint.end_frame(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Save the game state when exiting
        self.save_state();
    }
}
