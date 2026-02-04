//! Dither pattern drawing for e-ink style overlays and image processing.
//!
//! UI dithering: Instead of opaque black boxes, we draw a checkerboard dither
//! pattern so the user can still see content underneath selections and highlights.
//!
//! Image dithering: Floyd-Steinberg error-diffusion converts greyscale images
//! to pure 1-bit black and white for a classic Macintosh aesthetic.

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

/// Draw dithered text: white text on a dithered black background.
/// Returns the rect used so callers can position text on top.
pub fn draw_dither_label_bg(painter: &Painter, rect: Rect) {
    draw_dither_rect(painter, rect, Color32::BLACK, 1);
}

// ---------------------------------------------------------------------------
// Image dithering (Floyd-Steinberg error diffusion)
// ---------------------------------------------------------------------------

/// Apply Floyd-Steinberg dithering to convert a DynamicImage to 1-bit black & white.
///
/// The image is first converted to greyscale, then error-diffusion dithering
/// produces a pure black/white result that looks great on e-ink displays and
/// matches the classic Macintosh aesthetic.
///
/// Returns a `DynamicImage::ImageLuma8` where every pixel is 0 or 255.
pub fn floyd_steinberg_dither(img: &image::DynamicImage) -> image::DynamicImage {
    let gray = img.to_luma8();
    let (w, h) = gray.dimensions();
    if w == 0 || h == 0 {
        return image::DynamicImage::ImageLuma8(gray);
    }

    let w_usize = w as usize;
    // Work in f32 to accumulate error without clamping issues
    let mut buf: Vec<f32> = gray.pixels().map(|p| p.0[0] as f32).collect();

    for y in 0..h as usize {
        for x in 0..w_usize {
            let idx = y * w_usize + x;
            let old = buf[idx];
            let new = if old > 127.0 { 255.0 } else { 0.0 };
            buf[idx] = new;
            let err = old - new;

            if x + 1 < w_usize {
                buf[idx + 1] += err * 7.0 / 16.0;
            }
            if y + 1 < h as usize {
                let below = (y + 1) * w_usize;
                if x > 0 {
                    buf[below + x - 1] += err * 3.0 / 16.0;
                }
                buf[below + x] += err * 5.0 / 16.0;
                if x + 1 < w_usize {
                    buf[below + x + 1] += err * 1.0 / 16.0;
                }
            }
        }
    }

    let mut output = image::GrayImage::new(w, h);
    for (i, pixel) in output.pixels_mut().enumerate() {
        pixel.0[0] = buf[i].clamp(0.0, 255.0) as u8;
    }
    image::DynamicImage::ImageLuma8(output)
}
