//! Dither pattern drawing for e-ink style overlays.
//!
//! Instead of opaque black boxes, we draw a checkerboard dither
//! pattern so the user can still see content underneath selections and highlights.

use egui::{Color32, Painter, Rect, Pos2};

/// Draw a 50% checkerboard dither pattern over a rectangle.
/// Every other pixel is black, creating a translucent overlay effect.
/// `density` controls spacing: 1 = every pixel, 2 = every other, 3 = sparse.
pub fn draw_dither_rect(painter: &Painter, rect: Rect, color: Color32, density: u32) {
    let density = density.max(1);
    let pixel = 1.0;

    let x0 = rect.min.x as i32;
    let y0 = rect.min.y as i32;
    let x1 = rect.max.x as i32;
    let y1 = rect.max.y as i32;

    // Build small horizontal line segments for efficiency
    // instead of painting individual pixels
    let mut y = y0;
    while y < y1 {
        let mut x = x0 + (if (y - y0) % (density as i32 * 2) < density as i32 { 0 } else { density as i32 });
        while x < x1 {
            let px = x as f32;
            let py = y as f32;
            if px >= rect.min.x && py >= rect.min.y && px < rect.max.x && py < rect.max.y {
                painter.rect_filled(
                    Rect::from_min_size(Pos2::new(px, py), egui::Vec2::splat(pixel)),
                    0.0,
                    color,
                );
            }
            x += density as i32 * 2;
        }
        y += density as i32;
    }
}

/// Draw a dithered selection highlight (classic mac style).
/// Uses 2px spacing checkerboard for a lighter look.
pub fn draw_dither_selection(painter: &Painter, rect: Rect) {
    draw_dither_rect(painter, rect, Color32::BLACK, 1);
}

/// Draw a lighter dither for hover states.
/// Uses 3px spacing for a more subtle effect.
pub fn draw_dither_hover(painter: &Painter, rect: Rect) {
    draw_dither_rect(painter, rect, Color32::BLACK, 2);
}

/// Draw a dithered selection outline (frame) around a rectangle.
/// Only draws the border, not filling the interior.
/// `thickness` is the border width in pixels.
pub fn draw_dither_outline(painter: &Painter, rect: Rect, thickness: f32) {
    // Top edge
    let top = Rect::from_min_size(rect.min, egui::Vec2::new(rect.width(), thickness));
    draw_dither_rect(painter, top, Color32::BLACK, 1);

    // Bottom edge
    let bottom = Rect::from_min_size(
        Pos2::new(rect.min.x, rect.max.y - thickness),
        egui::Vec2::new(rect.width(), thickness),
    );
    draw_dither_rect(painter, bottom, Color32::BLACK, 1);

    // Left edge (excluding corners already drawn)
    let left = Rect::from_min_size(
        Pos2::new(rect.min.x, rect.min.y + thickness),
        egui::Vec2::new(thickness, rect.height() - thickness * 2.0),
    );
    draw_dither_rect(painter, left, Color32::BLACK, 1);

    // Right edge (excluding corners already drawn)
    let right = Rect::from_min_size(
        Pos2::new(rect.max.x - thickness, rect.min.y + thickness),
        egui::Vec2::new(thickness, rect.height() - thickness * 2.0),
    );
    draw_dither_rect(painter, right, Color32::BLACK, 1);
}

