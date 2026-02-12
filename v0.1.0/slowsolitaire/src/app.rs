use egui::{
    Align2, ColorImage, Context, FontId, Pos2, Rect, Sense, Stroke,
    TextureHandle, TextureOptions, Vec2,
};
use rand::seq::SliceRandom;
use slowcore::theme::SlowColors;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Card model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Suit {
    Spades,
    Clubs,
    Hearts,
    Diamonds,
}

impl Suit {
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            Suit::Spades => "S",
            Suit::Clubs => "C",
            Suit::Hearts => "H",
            Suit::Diamonds => "D",
        }
    }

    pub fn is_red(self) -> bool {
        matches!(self, Suit::Hearts | Suit::Diamonds)
    }

    pub fn all() -> [Suit; 4] {
        [Suit::Spades, Suit::Clubs, Suit::Hearts, Suit::Diamonds]
    }
}

/// Draw a suit symbol at a given center position and size using the painter.
fn draw_suit(painter: &egui::Painter, suit: Suit, center: Pos2, size: f32, color: egui::Color32) {
    let s = size * 0.5;
    match suit {
        Suit::Spades => {
            // Spade: upward triangle + two side bumps + stem
            let top = Pos2::new(center.x, center.y - s);
            let bl = Pos2::new(center.x - s * 0.7, center.y + s * 0.2);
            let br = Pos2::new(center.x + s * 0.7, center.y + s * 0.2);
            painter.add(egui::Shape::convex_polygon(
                vec![top, br, bl],
                color,
                Stroke::NONE,
            ));
            let bump = s * 0.3;
            painter.circle_filled(Pos2::new(center.x - s * 0.35, center.y + s * 0.1), bump, color);
            painter.circle_filled(Pos2::new(center.x + s * 0.35, center.y + s * 0.1), bump, color);
            // stem
            painter.rect_filled(
                Rect::from_center_size(
                    Pos2::new(center.x, center.y + s * 0.6),
                    Vec2::new(s * 0.2, s * 0.6),
                ),
                0.0,
                color,
            );
        }
        Suit::Hearts => {
            // Heart: two circles on top + triangle pointing down
            let r = s * 0.35;
            painter.circle_filled(Pos2::new(center.x - r * 0.85, center.y - s * 0.15), r, color);
            painter.circle_filled(Pos2::new(center.x + r * 0.85, center.y - s * 0.15), r, color);
            let left = Pos2::new(center.x - s * 0.7, center.y - s * 0.05);
            let right = Pos2::new(center.x + s * 0.7, center.y - s * 0.05);
            let bottom = Pos2::new(center.x, center.y + s * 0.8);
            painter.add(egui::Shape::convex_polygon(
                vec![left, right, bottom],
                color,
                Stroke::NONE,
            ));
        }
        Suit::Diamonds => {
            // Diamond: four-point shape
            let top = Pos2::new(center.x, center.y - s * 0.9);
            let right = Pos2::new(center.x + s * 0.55, center.y);
            let bottom = Pos2::new(center.x, center.y + s * 0.9);
            let left = Pos2::new(center.x - s * 0.55, center.y);
            painter.add(egui::Shape::convex_polygon(
                vec![top, right, bottom, left],
                color,
                Stroke::NONE,
            ));
        }
        Suit::Clubs => {
            // Club: three circles + stem
            let r = s * 0.3;
            painter.circle_filled(Pos2::new(center.x, center.y - s * 0.4), r, color);
            painter.circle_filled(Pos2::new(center.x - s * 0.35, center.y + s * 0.05), r, color);
            painter.circle_filled(Pos2::new(center.x + s * 0.35, center.y + s * 0.05), r, color);
            // stem
            painter.rect_filled(
                Rect::from_center_size(
                    Pos2::new(center.x, center.y + s * 0.6),
                    Vec2::new(s * 0.2, s * 0.6),
                ),
                0.0,
                color,
            );
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Card {
    pub suit: Suit,
    pub rank: u8, // 1=Ace .. 13=King
    pub face_up: bool,
}

impl Card {
    pub fn new(suit: Suit, rank: u8) -> Self {
        Self { suit, rank, face_up: false }
    }

    pub fn rank_label(self) -> &'static str {
        match self.rank {
            1 => "A",
            2 => "2",
            3 => "3",
            4 => "4",
            5 => "5",
            6 => "6",
            7 => "7",
            8 => "8",
            9 => "9",
            10 => "10",
            11 => "J",
            12 => "Q",
            13 => "K",
            _ => "?",
        }
    }

    pub fn is_face_card(self) -> bool {
        self.rank >= 11
    }

    /// Icon key for face cards: "king", "queen", "joker" (joker = jack)
    pub fn face_icon_key(self) -> Option<&'static str> {
        match self.rank {
            11 => Some("joker"),
            12 => Some("queen"),
            13 => Some("king"),
            _ => None,
        }
    }

    /// Can this card be placed on top of `other` in the tableau?
    /// (descending rank, alternating colour)
    pub fn can_stack_on_tableau(self, other: Card) -> bool {
        self.rank + 1 == other.rank && self.suit.is_red() != other.suit.is_red()
    }

    /// Can this card be placed on a foundation pile that currently has `top`?
    pub fn can_stack_on_foundation(self, top: Option<Card>) -> bool {
        match top {
            None => self.rank == 1,
            Some(t) => self.suit == t.suit && self.rank == t.rank + 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Game state
// ---------------------------------------------------------------------------

/// Where a card or group of cards is being dragged from.
#[derive(Clone, Debug)]
enum DragSource {
    Waste,
    Tableau(usize, usize), // (column, card_index)
    Foundation(usize),
}

pub struct SolitaireGame {
    /// Stock pile (face-down, draw from here)
    pub stock: Vec<Card>,
    /// Waste pile (face-up, drawn from stock)
    pub waste: Vec<Card>,
    /// Four foundation piles (one per suit, build A..K)
    pub foundations: [Vec<Card>; 4],
    /// Seven tableau columns
    pub tableau: [Vec<Card>; 7],
    /// Number of cards to draw (1 or 3)
    pub draw_count: u8,
    /// Move counter
    pub moves: u32,
}

impl SolitaireGame {
    pub fn new() -> Self {
        let mut deck = Vec::with_capacity(52);
        for &suit in &Suit::all() {
            for rank in 1..=13u8 {
                deck.push(Card::new(suit, rank));
            }
        }

        let mut rng = rand::thread_rng();
        deck.shuffle(&mut rng);

        let mut tableau: [Vec<Card>; 7] = Default::default();
        let mut idx = 0;
        for col in 0..7 {
            for row in 0..=col {
                let mut card = deck[idx];
                card.face_up = row == col; // only top card face-up
                tableau[col].push(card);
                idx += 1;
            }
        }

        let stock: Vec<Card> = deck[idx..].to_vec();

        Self {
            stock,
            waste: Vec::new(),
            foundations: Default::default(),
            tableau,
            draw_count: 1,
            moves: 0,
        }
    }

    /// Draw from stock to waste.
    pub fn draw_from_stock(&mut self) {
        if self.stock.is_empty() {
            // Recycle waste back into stock (reversed)
            while let Some(mut c) = self.waste.pop() {
                c.face_up = false;
                self.stock.push(c);
            }
        } else {
            let n = (self.draw_count as usize).min(self.stock.len());
            for _ in 0..n {
                if let Some(mut c) = self.stock.pop() {
                    c.face_up = true;
                    self.waste.push(c);
                }
            }
            self.moves += 1;
        }
    }

    /// Try to move the top waste card to a foundation. Returns true on success.
    pub fn waste_to_foundation(&mut self) -> bool {
        if let Some(&card) = self.waste.last() {
            for f in 0..4 {
                if card.can_stack_on_foundation(self.foundations[f].last().copied()) {
                    let mut c = self.waste.pop().unwrap();
                    c.face_up = true;
                    self.foundations[f].push(c);
                    self.moves += 1;
                    return true;
                }
            }
        }
        false
    }

    /// Try to move waste card to a specific tableau column. Returns true on success.
    pub fn waste_to_tableau(&mut self, col: usize) -> bool {
        if let Some(&card) = self.waste.last() {
            if self.can_place_on_tableau(card, col) {
                let mut c = self.waste.pop().unwrap();
                c.face_up = true;
                self.tableau[col].push(c);
                self.moves += 1;
                return true;
            }
        }
        false
    }

    /// Try to move a tableau card to a foundation. Returns true on success.
    pub fn tableau_to_foundation(&mut self, col: usize) -> bool {
        if let Some(&card) = self.tableau[col].last() {
            if !card.face_up {
                return false;
            }
            for f in 0..4 {
                if card.can_stack_on_foundation(self.foundations[f].last().copied()) {
                    let mut c = self.tableau[col].pop().unwrap();
                    c.face_up = true;
                    self.foundations[f].push(c);
                    self.flip_top(col);
                    self.moves += 1;
                    return true;
                }
            }
        }
        false
    }

    /// Move a run of cards from one tableau column to another.
    pub fn tableau_to_tableau(&mut self, from_col: usize, card_idx: usize, to_col: usize) -> bool {
        if from_col == to_col || card_idx >= self.tableau[from_col].len() {
            return false;
        }
        let card = self.tableau[from_col][card_idx];
        if !card.face_up {
            return false;
        }
        if !self.can_place_on_tableau(card, to_col) {
            return false;
        }
        let cards: Vec<Card> = self.tableau[from_col].drain(card_idx..).collect();
        self.tableau[to_col].extend(cards);
        self.flip_top(from_col);
        self.moves += 1;
        true
    }

    /// Move a foundation card back to a tableau column.
    pub fn foundation_to_tableau(&mut self, found_idx: usize, to_col: usize) -> bool {
        if let Some(&card) = self.foundations[found_idx].last() {
            if self.can_place_on_tableau(card, to_col) {
                let c = self.foundations[found_idx].pop().unwrap();
                self.tableau[to_col].push(c);
                self.moves += 1;
                return true;
            }
        }
        false
    }

    fn can_place_on_tableau(&self, card: Card, col: usize) -> bool {
        if let Some(&top) = self.tableau[col].last() {
            card.can_stack_on_tableau(top)
        } else {
            card.rank == 13 // only Kings on empty columns
        }
    }

    fn flip_top(&mut self, col: usize) {
        if let Some(c) = self.tableau[col].last_mut() {
            c.face_up = true;
        }
    }

    pub fn is_won(&self) -> bool {
        self.foundations.iter().all(|f| f.len() == 13)
    }

    /// Auto-finish: move all available cards to foundations.
    /// Returns true if any card was moved.
    pub fn auto_finish_step(&mut self) -> bool {
        // Try waste
        if self.waste_to_foundation() {
            return true;
        }
        // Try tableau
        for col in 0..7 {
            if self.tableau_to_foundation(col) {
                return true;
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

pub struct SlowSolitaireApp {
    game: SolitaireGame,
    /// Face card icon textures
    face_icons: HashMap<String, TextureHandle>,
    icons_loaded: bool,
    show_about: bool,
    /// Currently selected source for a move (click-to-select, click-to-place)
    selected: Option<DragSource>,
    /// Win state detected
    won: bool,
    /// Auto-finish in progress
    auto_finishing: bool,
}

impl SlowSolitaireApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            game: SolitaireGame::new(),
            face_icons: HashMap::new(),
            icons_loaded: false,
            show_about: false,
            selected: None,
            won: false,
            auto_finishing: false,
        }
    }

    fn new_game(&mut self) {
        self.game = SolitaireGame::new();
        self.selected = None;
        self.won = false;
        self.auto_finishing = false;
    }

    fn ensure_icons(&mut self, ctx: &Context) {
        if self.icons_loaded {
            return;
        }
        self.icons_loaded = true;

        let icons: &[(&str, &[u8])] = &[
            ("king", include_bytes!("../../icons/solitaire_icons/solitaire_king.png")),
            ("queen", include_bytes!("../../icons/solitaire_icons/solitaire_queen.png")),
            ("joker", include_bytes!("../../icons/solitaire_icons/solitaire_joker.png")),
        ];

        for (key, png_bytes) in icons {
            if let Ok(img) = image::load_from_memory(png_bytes) {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let color_image = ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    rgba.as_raw(),
                );
                let tex = ctx.load_texture(
                    format!("solitaire_{}", key),
                    color_image,
                    TextureOptions::NEAREST,
                );
                self.face_icons.insert(key.to_string(), tex);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Drawing helpers
    // -----------------------------------------------------------------------

    /// Card visual dimensions — sized so face card icons (64x90) render
    /// at their native pixel resolution with a small border.
    const CARD_W: f32 = 80.0;
    const CARD_H: f32 = 112.0;
    const CARD_RADIUS: f32 = 3.0;
    const TABLEAU_FACE_DOWN_OFFSET: f32 = 10.0;
    const TABLEAU_FACE_UP_OFFSET: f32 = 22.0;
    const PADDING: f32 = 8.0;

    /// Draw a face-down card (card back with line grid pattern).
    fn draw_card_back(&self, painter: &egui::Painter, rect: Rect) {
        // White fill with black border
        painter.rect_filled(rect, Self::CARD_RADIUS, SlowColors::WHITE);
        painter.rect_stroke(rect, Self::CARD_RADIUS, Stroke::new(1.0, SlowColors::BLACK));

        // Inner border for the pattern area
        let inner = rect.shrink(4.0);
        painter.rect_stroke(inner, 2.0, Stroke::new(1.0, SlowColors::BLACK));

        // Grid line pattern (no dither — clean lines)
        let pattern = inner.shrink(2.0);
        let step = 4.0;
        let mut x = pattern.min.x;
        while x <= pattern.max.x {
            painter.line_segment(
                [Pos2::new(x, pattern.min.y), Pos2::new(x, pattern.max.y)],
                Stroke::new(1.0, SlowColors::BLACK),
            );
            x += step;
        }
        let mut y = pattern.min.y;
        while y <= pattern.max.y {
            painter.line_segment(
                [Pos2::new(pattern.min.x, y), Pos2::new(pattern.max.x, y)],
                Stroke::new(1.0, SlowColors::BLACK),
            );
            y += step;
        }
    }

    /// Draw a face-up card.
    fn draw_card_face(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        card: Card,
        highlighted: bool,
    ) {
        // Base card
        painter.rect_filled(rect, Self::CARD_RADIUS, SlowColors::WHITE);

        if highlighted {
            painter.rect_stroke(rect, Self::CARD_RADIUS, Stroke::new(3.0, SlowColors::BLACK));
        } else {
            painter.rect_stroke(rect, Self::CARD_RADIUS, Stroke::new(1.0, SlowColors::BLACK));
        }

        let rank_str = card.rank_label();

        if card.is_face_card() {
            // Face cards: draw icon FIRST, then overlay corners on top
            if let Some(key) = card.face_icon_key() {
                if let Some(tex) = self.face_icons.get(key) {
                    // Scale down from native 64x90 keeping aspect ratio
                    let icon_w = 52.0;
                    let icon_h = icon_w * 90.0 / 64.0; // = 73.125
                    let icon_rect = Rect::from_center_size(
                        Pos2::new(rect.center().x, rect.center().y + 2.0),
                        Vec2::new(icon_w, icon_h),
                    );
                    painter.image(
                        tex.id(),
                        icon_rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
            }

            // White background boxes behind corner labels so they're readable
            let corner_w = 20.0;
            let corner_h = 32.0;
            painter.rect_filled(
                Rect::from_min_size(rect.min + Vec2::new(1.0, 1.0), Vec2::new(corner_w, corner_h)),
                0.0, SlowColors::WHITE,
            );
            painter.rect_filled(
                Rect::from_min_size(
                    Pos2::new(rect.max.x - corner_w - 1.0, rect.max.y - corner_h - 1.0),
                    Vec2::new(corner_w, corner_h),
                ),
                0.0, SlowColors::WHITE,
            );

            // Top-left rank + suit
            painter.text(
                Pos2::new(rect.min.x + 5.0, rect.min.y + 3.0),
                Align2::LEFT_TOP,
                rank_str,
                FontId::proportional(13.0),
                SlowColors::BLACK,
            );
            draw_suit(
                painter,
                card.suit,
                Pos2::new(rect.min.x + 11.0, rect.min.y + 22.0),
                11.0,
                SlowColors::BLACK,
            );

            // Bottom-right rank + suit (upside-down corner)
            painter.text(
                Pos2::new(rect.max.x - 5.0, rect.max.y - 3.0),
                Align2::RIGHT_BOTTOM,
                rank_str,
                FontId::proportional(13.0),
                SlowColors::BLACK,
            );
            draw_suit(
                painter,
                card.suit,
                Pos2::new(rect.max.x - 11.0, rect.max.y - 22.0),
                11.0,
                SlowColors::BLACK,
            );
        } else {
            // Number cards: draw corners first, then pips
            // Top-left rank + suit symbol
            painter.text(
                Pos2::new(rect.min.x + 5.0, rect.min.y + 3.0),
                Align2::LEFT_TOP,
                rank_str,
                FontId::proportional(13.0),
                SlowColors::BLACK,
            );
            draw_suit(
                painter,
                card.suit,
                Pos2::new(rect.min.x + 12.0, rect.min.y + 22.0),
                12.0,
                SlowColors::BLACK,
            );

            // Bottom-right rank + suit symbol
            painter.text(
                Pos2::new(rect.max.x - 5.0, rect.max.y - 3.0),
                Align2::RIGHT_BOTTOM,
                rank_str,
                FontId::proportional(13.0),
                SlowColors::BLACK,
            );
            draw_suit(
                painter,
                card.suit,
                Pos2::new(rect.max.x - 12.0, rect.max.y - 22.0),
                12.0,
                SlowColors::BLACK,
            );

            // Number cards: draw suit symbols in a pattern
            self.draw_pip_layout(painter, rect, card);
        }
    }

    /// Draw suit symbol pips in the centre of a number card.
    fn draw_pip_layout(&self, painter: &egui::Painter, rect: Rect, card: Card) {
        let pip_size = 16.0;
        let cx = rect.center().x;
        let top = rect.min.y + 32.0;
        let bottom = rect.max.y - 32.0;
        let height = bottom - top;

        // Column x positions
        let left_x = cx - 12.0;
        let right_x = cx + 12.0;

        let positions: Vec<Pos2> = match card.rank {
            1 => {
                // Ace: one large symbol in center
                draw_suit(painter, card.suit, rect.center(), 36.0, SlowColors::BLACK);
                return;
            }
            2 => vec![
                Pos2::new(cx, top),
                Pos2::new(cx, bottom),
            ],
            3 => vec![
                Pos2::new(cx, top),
                Pos2::new(cx, top + height / 2.0),
                Pos2::new(cx, bottom),
            ],
            4 => vec![
                Pos2::new(left_x, top),
                Pos2::new(right_x, top),
                Pos2::new(left_x, bottom),
                Pos2::new(right_x, bottom),
            ],
            5 => vec![
                Pos2::new(left_x, top),
                Pos2::new(right_x, top),
                Pos2::new(cx, top + height / 2.0),
                Pos2::new(left_x, bottom),
                Pos2::new(right_x, bottom),
            ],
            6 => vec![
                Pos2::new(left_x, top),
                Pos2::new(right_x, top),
                Pos2::new(left_x, top + height / 2.0),
                Pos2::new(right_x, top + height / 2.0),
                Pos2::new(left_x, bottom),
                Pos2::new(right_x, bottom),
            ],
            7 => vec![
                Pos2::new(left_x, top),
                Pos2::new(right_x, top),
                Pos2::new(cx, top + height / 3.0),
                Pos2::new(left_x, top + height / 2.0),
                Pos2::new(right_x, top + height / 2.0),
                Pos2::new(left_x, bottom),
                Pos2::new(right_x, bottom),
            ],
            8 => vec![
                Pos2::new(left_x, top),
                Pos2::new(right_x, top),
                Pos2::new(cx, top + height / 3.0),
                Pos2::new(left_x, top + height / 2.0),
                Pos2::new(right_x, top + height / 2.0),
                Pos2::new(cx, top + height * 2.0 / 3.0),
                Pos2::new(left_x, bottom),
                Pos2::new(right_x, bottom),
            ],
            9 => vec![
                Pos2::new(left_x, top),
                Pos2::new(right_x, top),
                Pos2::new(left_x, top + height / 3.0),
                Pos2::new(right_x, top + height / 3.0),
                Pos2::new(cx, top + height / 2.0),
                Pos2::new(left_x, top + height * 2.0 / 3.0),
                Pos2::new(right_x, top + height * 2.0 / 3.0),
                Pos2::new(left_x, bottom),
                Pos2::new(right_x, bottom),
            ],
            10 => vec![
                Pos2::new(left_x, top),
                Pos2::new(right_x, top),
                Pos2::new(cx, top + height / 6.0),
                Pos2::new(left_x, top + height / 3.0),
                Pos2::new(right_x, top + height / 3.0),
                Pos2::new(left_x, top + height * 2.0 / 3.0),
                Pos2::new(right_x, top + height * 2.0 / 3.0),
                Pos2::new(cx, top + height * 5.0 / 6.0),
                Pos2::new(left_x, bottom),
                Pos2::new(right_x, bottom),
            ],
            _ => return,
        };

        for pos in positions {
            draw_suit(painter, card.suit, pos, pip_size, SlowColors::BLACK);
        }
    }

    /// Draw an empty card slot (dashed outline).
    fn draw_empty_slot(&self, painter: &egui::Painter, rect: Rect) {
        painter.rect_stroke(
            rect,
            Self::CARD_RADIUS,
            Stroke::new(1.0, egui::Color32::from_rgb(180, 180, 180)),
        );
    }

    /// Draw an empty foundation slot with suit hint.
    fn draw_foundation_slot(&self, painter: &egui::Painter, rect: Rect, suit_idx: usize) {
        self.draw_empty_slot(painter, rect);
        let suit = Suit::all()[suit_idx];
        draw_suit(
            painter,
            suit,
            rect.center(),
            28.0,
            egui::Color32::from_rgb(200, 200, 200),
        );
    }

    // -----------------------------------------------------------------------
    // Layout calculations
    // -----------------------------------------------------------------------

    fn stock_rect(&self, area: Rect) -> Rect {
        Rect::from_min_size(
            Pos2::new(area.min.x + Self::PADDING, area.min.y + Self::PADDING),
            Vec2::new(Self::CARD_W, Self::CARD_H),
        )
    }

    fn waste_rect(&self, area: Rect) -> Rect {
        let stock = self.stock_rect(area);
        Rect::from_min_size(
            Pos2::new(stock.max.x + Self::PADDING, area.min.y + Self::PADDING),
            Vec2::new(Self::CARD_W, Self::CARD_H),
        )
    }

    fn foundation_rect(&self, area: Rect, idx: usize) -> Rect {
        let base_x = area.min.x + Self::PADDING + (Self::CARD_W + Self::PADDING) * 3.0;
        Rect::from_min_size(
            Pos2::new(
                base_x + idx as f32 * (Self::CARD_W + Self::PADDING),
                area.min.y + Self::PADDING,
            ),
            Vec2::new(Self::CARD_W, Self::CARD_H),
        )
    }

    fn tableau_base_pos(&self, area: Rect, col: usize) -> Pos2 {
        Pos2::new(
            area.min.x + Self::PADDING + col as f32 * (Self::CARD_W + Self::PADDING),
            area.min.y + Self::CARD_H + Self::PADDING * 3.0,
        )
    }

    fn tableau_card_rect(&self, area: Rect, col: usize, card_idx: usize) -> Rect {
        let base = self.tableau_base_pos(area, col);
        let mut y_off = 0.0;
        for i in 0..card_idx {
            if i < self.game.tableau[col].len() && self.game.tableau[col][i].face_up {
                y_off += Self::TABLEAU_FACE_UP_OFFSET;
            } else {
                y_off += Self::TABLEAU_FACE_DOWN_OFFSET;
            }
        }
        Rect::from_min_size(
            Pos2::new(base.x, base.y + y_off),
            Vec2::new(Self::CARD_W, Self::CARD_H),
        )
    }

    // -----------------------------------------------------------------------
    // Interaction
    // -----------------------------------------------------------------------

    fn handle_click(&mut self, area: Rect, pos: Pos2) {
        // Check stock click
        let stock_r = self.stock_rect(area);
        if stock_r.contains(pos) {
            self.selected = None;
            self.game.draw_from_stock();
            return;
        }

        // Check waste click
        let waste_r = self.waste_rect(area);
        if waste_r.contains(pos) && !self.game.waste.is_empty() {
            if self.selected.is_some() {
                self.selected = None;
            } else {
                self.selected = Some(DragSource::Waste);
            }
            return;
        }

        // Check foundation clicks
        for f in 0..4 {
            let fr = self.foundation_rect(area, f);
            if fr.contains(pos) {
                if let Some(ref src) = self.selected.clone() {
                    // Try to place the selected card here
                    match src {
                        DragSource::Waste => {
                            if self.game.waste_to_foundation() {
                                self.selected = None;
                                return;
                            }
                        }
                        DragSource::Tableau(col, _) => {
                            if self.game.tableau_to_foundation(*col) {
                                self.selected = None;
                                return;
                            }
                        }
                        DragSource::Foundation(_) => {}
                    }
                    self.selected = None;
                } else if !self.game.foundations[f].is_empty() {
                    self.selected = Some(DragSource::Foundation(f));
                }
                return;
            }
        }

        // Check tableau clicks (iterate cards top-to-bottom so topmost card wins)
        for col in 0..7 {
            let len = self.game.tableau[col].len();
            if len == 0 {
                // Click on empty column
                let base_rect = Rect::from_min_size(
                    self.tableau_base_pos(area, col),
                    Vec2::new(Self::CARD_W, Self::CARD_H),
                );
                if base_rect.contains(pos) {
                    if let Some(ref src) = self.selected.clone() {
                        match src {
                            DragSource::Waste => { self.game.waste_to_tableau(col); }
                            DragSource::Tableau(from_col, card_idx) => {
                                self.game.tableau_to_tableau(*from_col, *card_idx, col);
                            }
                            DragSource::Foundation(fi) => {
                                self.game.foundation_to_tableau(*fi, col);
                            }
                        }
                        self.selected = None;
                    }
                    return;
                }
                continue;
            }

            // Check from top card down
            for i in (0..len).rev() {
                let cr = self.tableau_card_rect(area, col, i);
                // For non-top cards, only the exposed strip is clickable
                let clickable = if i < len - 1 {
                    let next_r = self.tableau_card_rect(area, col, i + 1);
                    Rect::from_min_max(cr.min, Pos2::new(cr.max.x, next_r.min.y))
                } else {
                    cr
                };

                if clickable.contains(pos) {
                    let card = self.game.tableau[col][i];
                    if !card.face_up {
                        self.selected = None;
                        return;
                    }

                    if let Some(ref src) = self.selected.clone() {
                        // Try to place on this column
                        match src {
                            DragSource::Waste => { self.game.waste_to_tableau(col); }
                            DragSource::Tableau(from_col, card_idx) => {
                                self.game.tableau_to_tableau(*from_col, *card_idx, col);
                            }
                            DragSource::Foundation(fi) => {
                                self.game.foundation_to_tableau(*fi, col);
                            }
                        }
                        self.selected = None;
                    } else {
                        self.selected = Some(DragSource::Tableau(col, i));
                    }
                    return;
                }
            }
        }

        // Clicked elsewhere — deselect
        self.selected = None;
    }

    fn handle_double_click(&mut self, area: Rect, pos: Pos2) {
        // Double-click on waste -> try foundation
        let waste_r = self.waste_rect(area);
        if waste_r.contains(pos) && !self.game.waste.is_empty() {
            self.game.waste_to_foundation();
            self.selected = None;
            return;
        }

        // Double-click on tableau top card -> try foundation
        for col in 0..7 {
            let len = self.game.tableau[col].len();
            if len == 0 {
                continue;
            }
            let cr = self.tableau_card_rect(area, col, len - 1);
            if cr.contains(pos) {
                self.game.tableau_to_foundation(col);
                self.selected = None;
                return;
            }
        }
    }

    /// Check if a card at a given source is currently selected.
    fn is_selected_waste(&self) -> bool {
        matches!(&self.selected, Some(DragSource::Waste))
    }

    fn is_selected_tableau(&self, col: usize, idx: usize) -> bool {
        match &self.selected {
            Some(DragSource::Tableau(c, i)) => *c == col && idx >= *i,
            _ => false,
        }
    }

    fn is_selected_foundation(&self, f: usize) -> bool {
        matches!(&self.selected, Some(DragSource::Foundation(fi)) if *fi == f)
    }

    // -----------------------------------------------------------------------
    // Rendering
    // -----------------------------------------------------------------------

    fn render_game(&self, ui: &mut egui::Ui) -> Option<(Pos2, bool)> {
        let area = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(area, Sense::click());
        let painter = ui.painter_at(area);

        // Background
        painter.rect_filled(area, 0.0, SlowColors::WHITE);

        // Stock pile
        let stock_r = self.stock_rect(area);
        if self.game.stock.is_empty() {
            // Draw recycle indicator
            self.draw_empty_slot(&painter, stock_r);
            painter.text(
                stock_r.center(),
                Align2::CENTER_CENTER,
                "O",
                FontId::proportional(24.0),
                egui::Color32::from_rgb(150, 150, 150),
            );
        } else {
            self.draw_card_back(&painter, stock_r);
            // Show count
            painter.text(
                Pos2::new(stock_r.center().x, stock_r.max.y + 4.0),
                Align2::CENTER_TOP,
                &format!("{}", self.game.stock.len()),
                FontId::proportional(10.0),
                SlowColors::BLACK,
            );
        }

        // Waste pile
        let waste_r = self.waste_rect(area);
        if let Some(&card) = self.game.waste.last() {
            self.draw_card_face(&painter, waste_r, card, self.is_selected_waste());
        } else {
            self.draw_empty_slot(&painter, waste_r);
        }

        // Foundations
        for f in 0..4 {
            let fr = self.foundation_rect(area, f);
            if let Some(&card) = self.game.foundations[f].last() {
                self.draw_card_face(&painter, fr, card, self.is_selected_foundation(f));
            } else {
                self.draw_foundation_slot(&painter, fr, f);
            }
        }

        // Tableau
        for col in 0..7 {
            let base = self.tableau_base_pos(area, col);
            if self.game.tableau[col].is_empty() {
                let empty_rect = Rect::from_min_size(base, Vec2::new(Self::CARD_W, Self::CARD_H));
                self.draw_empty_slot(&painter, empty_rect);
                // Show K hint for empty columns
                painter.text(
                    empty_rect.center(),
                    Align2::CENTER_CENTER,
                    "K",
                    FontId::proportional(20.0),
                    egui::Color32::from_rgb(200, 200, 200),
                );
                continue;
            }

            for (i, &card) in self.game.tableau[col].iter().enumerate() {
                let cr = self.tableau_card_rect(area, col, i);
                if card.face_up {
                    let highlighted = self.is_selected_tableau(col, i);
                    self.draw_card_face(&painter, cr, card, highlighted);
                } else {
                    self.draw_card_back(&painter, cr);
                }
            }
        }

        // Determine click type
        let clicked = response.clicked();
        let double_clicked = response.double_clicked();
        let click_pos = response.interact_pointer_pos();

        if double_clicked {
            if let Some(pos) = click_pos {
                return Some((pos, true));
            }
        } else if clicked {
            if let Some(pos) = click_pos {
                return Some((pos, false));
            }
        }

        None
    }

    fn draw_menu(&mut self, ctx: &Context) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            slowcore::theme::menu_bar(ui, |ui| {
                ui.menu_button("game", |ui| {
                    if ui.button("new game").clicked() {
                        self.new_game();
                        ui.close_menu();
                    }
                    ui.separator();
                    let label = if self.game.draw_count == 1 {
                        "draw 3"
                    } else {
                        "draw 1"
                    };
                    if ui.button(label).clicked() {
                        self.game.draw_count = if self.game.draw_count == 1 { 3 } else { 1 };
                        self.new_game();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("auto finish").clicked() {
                        self.auto_finishing = true;
                        ui.close_menu();
                    }
                });
                ui.menu_button("help", |ui| {
                    if ui.button("about").clicked() {
                        self.show_about = true;
                        ui.close_menu();
                    }
                });
            });
        });
    }

    fn draw_status(&self, ctx: &Context) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("moves: {}", self.game.moves))
                        .font(FontId::proportional(11.0)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let draw_mode = if self.game.draw_count == 1 {
                        "draw 1"
                    } else {
                        "draw 3"
                    };
                    let foundation_count: usize =
                        self.game.foundations.iter().map(|f| f.len()).sum();
                    ui.label(
                        egui::RichText::new(format!(
                            "{}  |  {}/52",
                            draw_mode, foundation_count
                        ))
                        .font(FontId::proportional(11.0)),
                    );
                });
            });
        });
    }

    fn draw_about(&mut self, ctx: &Context) {
        if !self.show_about {
            return;
        }
        egui::Window::new("about solitaire")
            .collapsible(false)
            .resizable(false)
            .default_width(280.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    ui.heading("solitaire");
                    ui.add_space(4.0);
                    ui.label("klondike solitaire");
                    ui.add_space(8.0);
                    ui.label("click a card to select it,");
                    ui.label("then click where to place it.");
                    ui.label("double-click to send to foundation.");
                    ui.add_space(8.0);
                    ui.label("click the stock pile to draw.");
                    ui.add_space(12.0);
                    if ui.button("ok").clicked() {
                        self.show_about = false;
                    }
                    ui.add_space(4.0);
                });
            });
    }

    fn draw_win(&mut self, ctx: &Context) {
        if !self.won {
            return;
        }
        egui::Window::new("you win!")
            .collapsible(false)
            .resizable(false)
            .default_width(240.0)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    ui.heading("congratulations!");
                    ui.add_space(4.0);
                    ui.label(format!("completed in {} moves", self.game.moves));
                    ui.add_space(12.0);
                    if ui.button("new game").clicked() {
                        self.new_game();
                    }
                    ui.add_space(4.0);
                });
            });
    }
}

impl eframe::App for SlowSolitaireApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.ensure_icons(ctx);
        slowcore::theme::consume_special_keys(ctx);

        // Auto-finish animation
        if self.auto_finishing {
            if !self.game.auto_finish_step() {
                self.auto_finishing = false;
            }
            ctx.request_repaint();
        }

        // Check win
        if !self.won && self.game.is_won() {
            self.won = true;
        }

        self.draw_menu(ctx);
        self.draw_status(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(SlowColors::WHITE))
            .show(ctx, |ui| {
                let area = ui.available_rect_before_wrap();
                let click = self.render_game(ui);
                if let Some((pos, is_double)) = click {
                    if is_double {
                        self.handle_double_click(area, pos);
                    } else {
                        self.handle_click(area, pos);
                    }
                }
            });

        self.draw_about(ctx);
        self.draw_win(ctx);
    }
}
