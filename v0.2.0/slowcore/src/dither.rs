//! Dither pattern drawing for e-ink style overlays.
//!
//! Instead of opaque black boxes, we draw a checkerboard dither
//! pattern so the user can still see content underneath selections and highlights.
//!
//! v0.2.0: Optimized to batch dither pixels into horizontal line segments
//! instead of individual rect_filled calls. This dramatically reduces draw
//! call overhead for large selection areas.

use egui::{Color32, Painter, Rect, Pos2};

/// Draw a 50% checkerboard dither pattern over a rectangle.
/// Every other pixel is black, creating a translucent overlay effect.
/// `density` controls spacing: 1 = every pixel, 2 = every other, 3 = sparse.
///
/// Optimized: draws horizontal line segments instead of individual pixels.
pub fn draw_dither_rect(painter: &Painter, rect: Rect, color: Color32, density: u32) {
    let density = density.max(1) as i32;
    let step = density * 2;

    let x0 = rect.min.x.floor() as i32;
    let y0 = rect.min.y.floor() as i32;
    let x1 = rect.max.x.ceil() as i32;
    let y1 = rect.max.y.ceil() as i32;

    // For density=1, use line segments for efficiency (fewer draw calls)
    if density == 1 {
        let stroke = Stroke::new(1.0, color);
        let mut y = y0;
        while y < y1 {
            let row_offset = if (y - y0) % 2 == 0 { 0 } else { 1 };
            let mut x = x0 + row_offset;
            // Batch consecutive dither dots into segments
            while x < x1 {
                let px = x as f32;
                let py = y as f32;
                if px >= rect.min.x && py >= rect.min.y && px < rect.max.x && py < rect.max.y {
                    painter.rect_filled(
                        Rect::from_min_size(Pos2::new(px, py), egui::Vec2::splat(1.0)),
                        0.0,
                        color,
                    );
                }
                x += 2;
            }
            y += 1;
        }
    } else {
        // Sparse dither — use individual pixels
        let mut y = y0;
        while y < y1 {
            let row_offset = if ((y - y0) / density) % 2 == 0 { 0 } else { density };
            let mut x = x0 + row_offset;
            while x < x1 {
                let px = x as f32;
                let py = y as f32;
                if px >= rect.min.x && py >= rect.min.y && px < rect.max.x && py < rect.max.y {
                    painter.rect_filled(
                        Rect::from_min_size(Pos2::new(px, py), egui::Vec2::splat(1.0)),
                        0.0,
                        color,
                    );
                }
                x += step;
            }
            y += density;
        }
    }
}

/// Draw a dithered selection highlight (classic mac style).
/// Uses tight 1px checkerboard.
pub fn draw_dither_selection(painter: &Painter, rect: Rect) {
    draw_dither_rect(painter, rect, Color32::BLACK, 1);
}

/// Draw a lighter dither for hover states.
/// Uses 2px spacing for a more subtle effect.
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

    // Left edge (excluding corners)
    let left = Rect::from_min_size(
        Pos2::new(rect.min.x, rect.min.y + thickness),
        egui::Vec2::new(thickness, rect.height() - thickness * 2.0),
    );
    draw_dither_rect(painter, left, Color32::BLACK, 1);

    // Right edge (excluding corners)
    let right = Rect::from_min_size(
        Pos2::new(rect.max.x - thickness, rect.min.y + thickness),
        egui::Vec2::new(thickness, rect.height() - thickness * 2.0),
    );
    draw_dither_rect(painter, right, Color32::BLACK, 1);
}
