//! Dither pattern drawing for e-ink style overlays.
//!
//! Instead of opaque black boxes, we draw a checkerboard dither
//! pattern so the user can still see content underneath selections and highlights.
//!
//! v0.2.1: Streamlined inner loop — bounds are clamped once up front so no
//! per-pixel check is needed inside the loop.

use egui::{Color32, Painter, Pos2, Rect};

/// Draw a checkerboard dither pattern over a rectangle.
/// Every other pixel is colored, creating a translucent overlay effect.
/// `density` controls spacing: 1 = every pixel, 2 = every other, 3 = sparse.
///
/// Bounds are clamped once before iteration so the inner loop needs no
/// per-pixel bounds check.
pub fn draw_dither_rect(painter: &Painter, rect: Rect, color: Color32, density: u32) {
    let density = density.max(1) as i32;

    // Clamp iteration bounds inward so every (x, y) in the loop is guaranteed
    // to lie inside `rect`. Use ceil for the start edge and floor for the end
    // edge — this is the key difference from the old code which used floor/ceil
    // and then re-checked every pixel.
    let x0 = rect.min.x.ceil() as i32;
    let y0 = rect.min.y.ceil() as i32;
    let x1 = rect.max.x.floor() as i32;
    let y1 = rect.max.y.floor() as i32;

    if x0 >= x1 || y0 >= y1 {
        return;
    }

    let y_step = if density == 1 { 1 } else { density };
    let x_step = if density == 1 { 2 } else { density * 2 };

    let pixel = egui::Vec2::splat(1.0);

    let mut y = y0;
    while y < y1 {
        let row_offset = if density == 1 {
            if (y - y0) % 2 == 0 { 0 } else { 1 }
        } else {
            if ((y - y0) / density) % 2 == 0 { 0 } else { density }
        };

        let mut x = x0 + row_offset;
        while x < x1 {
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(x as f32, y as f32), pixel),
                0.0,
                color,
            );
            x += x_step;
        }
        y += y_step;
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

/// Draw a dithered drop shadow for a window.
/// Call after egui::Window::show() with the window rect.
/// Uses Order::PanelResizeLine so the shadow renders between panels and windows.
pub fn draw_window_shadow(ctx: &egui::Context, window_rect: Rect) {
    let shadow_rect = Rect::from_min_max(
        Pos2::new(window_rect.min.x + 4.0, window_rect.min.y + 4.0),
        Pos2::new(window_rect.max.x + 4.0, window_rect.max.y + 4.0),
    );
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::PanelResizeLine,
        egui::Id::new("dither_shadows"),
    ));
    draw_dither_rect(&painter, shadow_rect, Color32::BLACK, 2);
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
