//! Chess game logic - board state, move generation, validation

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color { White, Black }
impl Color {
    pub fn opposite(&self) -> Self { match self { Color::White => Color::Black, Color::Black => Color::White } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PieceKind { King, Queen, Rook, Bishop, Knight, Pawn }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Piece {
    pub kind: PieceKind,
    pub color: Color,
}

impl Piece {
    pub fn new(kind: PieceKind, color: Color) -> Self { Self { kind, color } }

    pub fn symbol(&self) -> &'static str {
        match (self.color, self.kind) {
            (Color::White, PieceKind::King) => "♔",
            (Color::White, PieceKind::Queen) => "♕",
            (Color::White, PieceKind::Rook) => "♖",
            (Color::White, PieceKind::Bishop) => "♗",
            (Color::White, PieceKind::Knight) => "♘",
            (Color::White, PieceKind::Pawn) => "♙",
            (Color::Black, PieceKind::King) => "♚",
            (Color::Black, PieceKind::Queen) => "♛",
            (Color::Black, PieceKind::Rook) => "♜",
            (Color::Black, PieceKind::Bishop) => "♝",
            (Color::Black, PieceKind::Knight) => "♞",
            (Color::Black, PieceKind::Pawn) => "♟",
        }
    }
}

pub type Square = Option<Piece>;
pub type Pos = (usize, usize); // (row, col) where row 0 = rank 8 (top)

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct Move {
    pub from: Pos,
    pub to: Pos,
    pub promotion: Option<PieceKind>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameState { Playing, Check, Checkmate, Stalemate }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Board {
    pub squares: [[Square; 8]; 8],
    pub turn: Color,
    pub state: GameState,
    pub move_history: Vec<String>,
    pub castling: CastlingRights,
    pub en_passant: Option<Pos>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CastlingRights {
    pub white_king: bool,
    pub white_queen: bool,
    pub black_king: bool,
    pub black_queen: bool,
}

impl Default for CastlingRights {
    fn default() -> Self {
        Self { white_king: true, white_queen: true, black_king: true, black_queen: true }
    }
}

impl Board {
    pub fn new() -> Self {
        let mut b = Self {
            squares: [[None; 8]; 8],
            turn: Color::White,
            state: GameState::Playing,
            move_history: Vec::new(),
            castling: CastlingRights::default(),
            en_passant: None,
        };

        // Black pieces (row 0 = rank 8)
        let back = [PieceKind::Rook, PieceKind::Knight, PieceKind::Bishop, PieceKind::Queen,
                     PieceKind::King, PieceKind::Bishop, PieceKind::Knight, PieceKind::Rook];
        for c in 0..8 {
            b.squares[0][c] = Some(Piece::new(back[c], Color::Black));
            b.squares[1][c] = Some(Piece::new(PieceKind::Pawn, Color::Black));
            b.squares[6][c] = Some(Piece::new(PieceKind::Pawn, Color::White));
            b.squares[7][c] = Some(Piece::new(back[c], Color::White));
        }
        b
    }

    pub fn get(&self, pos: Pos) -> Square { self.squares[pos.0][pos.1] }

    #[allow(dead_code)]
    fn set(&mut self, pos: Pos, piece: Square) { self.squares[pos.0][pos.1] = piece; }

    pub fn in_bounds(r: i32, c: i32) -> bool { r >= 0 && r < 8 && c >= 0 && c < 8 }

    /// Generate all pseudo-legal moves for a piece (doesn't check for leaving king in check)
    pub fn piece_moves(&self, pos: Pos) -> Vec<Pos> {
        let piece = match self.get(pos) {
            Some(p) => p,
            None => return vec![],
        };

        let (r, c) = (pos.0 as i32, pos.1 as i32);
        let mut moves = Vec::new();

        match piece.kind {
            PieceKind::Pawn => {
                let dir: i32 = if piece.color == Color::White { -1 } else { 1 };
                let start_row = if piece.color == Color::White { 6 } else { 1 };

                // Forward
                let nr = r + dir;
                if Self::in_bounds(nr, c) && self.squares[nr as usize][c as usize].is_none() {
                    moves.push((nr as usize, c as usize));
                    // Double move from start
                    let nr2 = r + dir * 2;
                    if pos.0 == start_row && self.squares[nr2 as usize][c as usize].is_none() {
                        moves.push((nr2 as usize, c as usize));
                    }
                }
                // Captures
                for dc in [-1i32, 1] {
                    let nc = c + dc;
                    if Self::in_bounds(nr, nc) {
                        let target = self.squares[nr as usize][nc as usize];
                        if target.map(|p| p.color != piece.color).unwrap_or(false) {
                            moves.push((nr as usize, nc as usize));
                        }
                        // En passant
                        if self.en_passant == Some((nr as usize, nc as usize)) {
                            moves.push((nr as usize, nc as usize));
                        }
                    }
                }
            }
            PieceKind::Knight => {
                for (dr, dc) in [(-2,-1),(-2,1),(-1,-2),(-1,2),(1,-2),(1,2),(2,-1),(2,1)] {
                    let (nr, nc) = (r + dr, c + dc);
                    if Self::in_bounds(nr, nc) {
                        let target = self.squares[nr as usize][nc as usize];
                        if target.map(|p| p.color != piece.color).unwrap_or(true) {
                            moves.push((nr as usize, nc as usize));
                        }
                    }
                }
            }
            PieceKind::King => {
                for dr in -1..=1 {
                    for dc in -1..=1 {
                        if dr == 0 && dc == 0 { continue; }
                        let (nr, nc) = (r + dr, c + dc);
                        if Self::in_bounds(nr, nc) {
                            let target = self.squares[nr as usize][nc as usize];
                            if target.map(|p| p.color != piece.color).unwrap_or(true) {
                                moves.push((nr as usize, nc as usize));
                            }
                        }
                    }
                }
                // Castling
                let row = if piece.color == Color::White { 7 } else { 0 };
                if pos == (row, 4) {
                    let (ks, qs) = match piece.color {
                        Color::White => (self.castling.white_king, self.castling.white_queen),
                        Color::Black => (self.castling.black_king, self.castling.black_queen),
                    };
                    if ks && self.squares[row][5].is_none() && self.squares[row][6].is_none() {
                        if !self.is_attacked((row, 4), piece.color.opposite())
                            && !self.is_attacked((row, 5), piece.color.opposite()) {
                            moves.push((row, 6));
                        }
                    }
                    if qs && self.squares[row][3].is_none() && self.squares[row][2].is_none() && self.squares[row][1].is_none() {
                        if !self.is_attacked((row, 4), piece.color.opposite())
                            && !self.is_attacked((row, 3), piece.color.opposite()) {
                            moves.push((row, 2));
                        }
                    }
                }
            }
            PieceKind::Rook => self.slide_moves(pos, &[(0,1),(0,-1),(1,0),(-1,0)], piece.color, &mut moves),
            PieceKind::Bishop => self.slide_moves(pos, &[(1,1),(1,-1),(-1,1),(-1,-1)], piece.color, &mut moves),
            PieceKind::Queen => self.slide_moves(pos, &[(0,1),(0,-1),(1,0),(-1,0),(1,1),(1,-1),(-1,1),(-1,-1)], piece.color, &mut moves),
        }

        moves
    }

    fn slide_moves(&self, pos: Pos, dirs: &[(i32,i32)], color: Color, moves: &mut Vec<Pos>) {
        let (r, c) = (pos.0 as i32, pos.1 as i32);
        for &(dr, dc) in dirs {
            let (mut nr, mut nc) = (r + dr, c + dc);
            while Self::in_bounds(nr, nc) {
                let target = self.squares[nr as usize][nc as usize];
                if let Some(p) = target {
                    if p.color != color { moves.push((nr as usize, nc as usize)); }
                    break;
                }
                moves.push((nr as usize, nc as usize));
                nr += dr;
                nc += dc;
            }
        }
    }

    /// Check if a square is attacked by the given color
    pub fn is_attacked(&self, pos: Pos, by_color: Color) -> bool {
        for r in 0..8 {
            for c in 0..8 {
                if let Some(p) = self.squares[r][c] {
                    if p.color == by_color {
                        // For pawns, only check diagonal attacks, not forward moves
                        if p.kind == PieceKind::Pawn {
                            // Pawns only attack diagonally
                            let dir: i32 = if p.color == Color::White { -1 } else { 1 };
                            let nr = r as i32 + dir;
                            for dc in [-1i32, 1] {
                                let nc = c as i32 + dc;
                                if nr == pos.0 as i32 && nc == pos.1 as i32 {
                                    return true;
                                }
                            }
                        } else if p.kind == PieceKind::King {
                            // King attacks adjacent squares only (not castling)
                            // Handle separately to avoid infinite recursion with piece_moves
                            for dr in -1i32..=1 {
                                for dc in -1i32..=1 {
                                    if dr == 0 && dc == 0 { continue; }
                                    let nr = r as i32 + dr;
                                    let nc = c as i32 + dc;
                                    if nr == pos.0 as i32 && nc == pos.1 as i32 {
                                        return true;
                                    }
                                }
                            }
                        } else {
                            let moves = self.piece_moves((r, c));
                            if moves.contains(&pos) { return true; }
                        }
                    }
                }
            }
        }
        false
    }

    fn find_king(&self, color: Color) -> Option<Pos> {
        for r in 0..8 {
            for c in 0..8 {
                if let Some(p) = self.squares[r][c] {
                    if p.kind == PieceKind::King && p.color == color {
                        return Some((r, c));
                    }
                }
            }
        }
        None
    }

    pub fn in_check(&self, color: Color) -> bool {
        self.find_king(color).map(|k| self.is_attacked(k, color.opposite())).unwrap_or(false)
    }

    /// Get legal moves (filtered for not leaving own king in check)
    pub fn legal_moves(&self, pos: Pos) -> Vec<Pos> {
        let piece = match self.get(pos) {
            Some(p) if p.color == self.turn => p,
            _ => return vec![],
        };

        self.piece_moves(pos).into_iter().filter(|&to| {
            let mut test = self.clone();
            test.raw_move(pos, to);
            !test.in_check(piece.color)
        }).collect()
    }

    fn raw_move(&mut self, from: Pos, to: Pos) {
        let piece = self.squares[from.0][from.1].take();
        self.squares[to.0][to.1] = piece;
    }

    /// Make a move, update game state
    pub fn make_move(&mut self, from: Pos, to: Pos) -> bool {
        let piece = match self.get(from) {
            Some(p) if p.color == self.turn => p,
            _ => return false,
        };

        let legal = self.legal_moves(from);
        if !legal.contains(&to) { return false; }

        let notation = self.to_notation(from, to);

        // En passant capture
        if piece.kind == PieceKind::Pawn && Some(to) == self.en_passant {
            let captured_row = from.0;
            self.squares[captured_row][to.1] = None;
        }

        // Set en passant
        self.en_passant = None;
        if piece.kind == PieceKind::Pawn && (from.0 as i32 - to.0 as i32).abs() == 2 {
            self.en_passant = Some(((from.0 + to.0) / 2, from.1));
        }

        // Castling - move rook
        if piece.kind == PieceKind::King && (from.1 as i32 - to.1 as i32).abs() == 2 {
            if to.1 == 6 { // Kingside
                let rook = self.squares[from.0][7].take();
                self.squares[from.0][5] = rook;
            } else if to.1 == 2 { // Queenside
                let rook = self.squares[from.0][0].take();
                self.squares[from.0][3] = rook;
            }
        }

        // Update castling rights
        if piece.kind == PieceKind::King {
            match piece.color {
                Color::White => { self.castling.white_king = false; self.castling.white_queen = false; }
                Color::Black => { self.castling.black_king = false; self.castling.black_queen = false; }
            }
        }
        if piece.kind == PieceKind::Rook {
            match (piece.color, from) {
                (Color::White, (7, 7)) => self.castling.white_king = false,
                (Color::White, (7, 0)) => self.castling.white_queen = false,
                (Color::Black, (0, 7)) => self.castling.black_king = false,
                (Color::Black, (0, 0)) => self.castling.black_queen = false,
                _ => {}
            }
        }

        // Move piece
        self.raw_move(from, to);

        // Pawn promotion (auto-queen)
        if piece.kind == PieceKind::Pawn && (to.0 == 0 || to.0 == 7) {
            self.squares[to.0][to.1] = Some(Piece::new(PieceKind::Queen, piece.color));
        }

        self.move_history.push(notation);
        self.turn = self.turn.opposite();
        self.update_state();
        true
    }

    fn update_state(&mut self) {
        let has_moves = (0..8).any(|r| (0..8).any(|c| {
            self.get((r, c)).map(|p| p.color == self.turn).unwrap_or(false)
                && !self.legal_moves((r, c)).is_empty()
        }));

        if self.in_check(self.turn) {
            self.state = if has_moves { GameState::Check } else { GameState::Checkmate };
        } else {
            self.state = if has_moves { GameState::Playing } else { GameState::Stalemate };
        }
    }

    fn to_notation(&self, from: Pos, to: Pos) -> String {
        let piece = match self.get(from) {
            Some(p) => p,
            None => return format!("{}{}→{}{}",
                (b'a' + from.1 as u8) as char, 8 - from.0,
                (b'a' + to.1 as u8) as char, 8 - to.0),
        };
        let piece_char = match piece.kind {
            PieceKind::King => "K",
            PieceKind::Queen => "Q",
            PieceKind::Rook => "R",
            PieceKind::Bishop => "B",
            PieceKind::Knight => "N",
            PieceKind::Pawn => "",
        };
        let capture = if self.get(to).is_some() { "x" } else { "" };
        let file = (b'a' + to.1 as u8) as char;
        let rank = 8 - to.0;
        format!("{}{}{}{}", piece_char, capture, file, rank)
    }
}
