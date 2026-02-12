//! Canvas - bitmap image representation and manipulation

use image::{ImageBuffer, Rgba, RgbaImage};
use std::path::PathBuf;

/// A bitmap canvas for editing
#[derive(Clone)]
pub struct Canvas {
    pub image: RgbaImage,
    pub path: Option<PathBuf>,
    pub modified: bool,
    undo_stack: Vec<RgbaImage>,
    redo_stack: Vec<RgbaImage>,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Self {
        let image = ImageBuffer::from_pixel(width, height, Rgba([255, 255, 255, 255]));
        Self {
            image,
            path: None,
            modified: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }
    
    pub fn open(path: PathBuf) -> Result<Self, image::ImageError> {
        let img = image::open(&path)?;
        // Convert to grayscale to reduce processing overhead
        let gray = img.to_luma8();
        // Convert grayscale back to RGBA (all channels same value)
        let (w, h) = gray.dimensions();
        let mut image = ImageBuffer::new(w, h);
        for (x, y, pixel) in gray.enumerate_pixels() {
            let v = pixel.0[0];
            image.put_pixel(x, y, Rgba([v, v, v, 255]));
        }
        Ok(Self {
            image,
            path: Some(path),
            modified: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        })
    }
    
    pub fn save(&mut self) -> Result<(), image::ImageError> {
        if let Some(ref path) = self.path {
            self.image.save(path)?;
            self.modified = false;
        }
        Ok(())
    }
    
    pub fn save_as(&mut self, path: PathBuf) -> Result<(), image::ImageError> {
        self.image.save(&path)?;
        self.path = Some(path);
        self.modified = false;
        Ok(())
    }

    pub fn width(&self) -> u32 { self.image.width() }
    pub fn height(&self) -> u32 { self.image.height() }

    /// Resize the canvas to new dimensions. Preserves content (crops if smaller, pads with white if larger).
    pub fn resize(&mut self, new_width: u32, new_height: u32) {
        self.save_undo_state();
        let mut new_image = ImageBuffer::from_pixel(new_width, new_height, Rgba([255, 255, 255, 255]));
        // Copy existing pixels
        let copy_width = self.width().min(new_width);
        let copy_height = self.height().min(new_height);
        for y in 0..copy_height {
            for x in 0..copy_width {
                new_image.put_pixel(x, y, *self.image.get_pixel(x, y));
            }
        }
        self.image = new_image;
        self.modified = true;
    }
    
    pub fn display_title(&self) -> String {
        let name = self.path.as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string());
        if self.modified { format!("{}*", name) } else { name }
    }
    
    pub fn save_undo_state(&mut self) {
        self.undo_stack.push(self.image.clone());
        self.redo_stack.clear();
        if self.undo_stack.len() > 20 { self.undo_stack.remove(0); }
    }
    
    pub fn undo(&mut self) -> bool {
        if let Some(state) = self.undo_stack.pop() {
            self.redo_stack.push(self.image.clone());
            self.image = state;
            self.modified = true;
            true
        } else { false }
    }
    
    pub fn redo(&mut self) -> bool {
        if let Some(state) = self.redo_stack.pop() {
            self.undo_stack.push(self.image.clone());
            self.image = state;
            self.modified = true;
            true
        } else { false }
    }
    
    pub fn set_pixel(&mut self, x: u32, y: u32, color: Rgba<u8>) {
        if x < self.width() && y < self.height() {
            self.image.put_pixel(x, y, color);
            self.modified = true;
        }
    }
    
    fn set_pixel_safe(&mut self, x: i32, y: i32, color: Rgba<u8>) {
        if x >= 0 && y >= 0 { self.set_pixel(x as u32, y as u32, color); }
    }
    
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba<u8>, thickness: u32) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let (mut x, mut y) = (x0, y0);

        loop {
            self.draw_circle_filled(x, y, thickness as i32 / 2, color);
            if x == x1 && y == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy { if x == x1 { break; } err += dy; x += sx; }
            if e2 <= dx { if y == y1 { break; } err += dx; y += sy; }
        }
        self.modified = true;
    }

    /// Draw a line with pattern support
    pub fn draw_line_pattern(
        &mut self, x0: i32, y0: i32, x1: i32, y1: i32,
        color: Rgba<u8>, thickness: u32, pattern: &crate::tools::Pattern,
    ) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let (mut x, mut y) = (x0, y0);

        loop {
            self.draw_circle_filled_pattern(x, y, thickness as i32 / 2, color, pattern);
            if x == x1 && y == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy { if x == x1 { break; } err += dy; x += sx; }
            if e2 <= dx { if y == y1 { break; } err += dx; y += sy; }
        }
        self.modified = true;
    }

    pub fn draw_circle_filled(&mut self, cx: i32, cy: i32, radius: i32, color: Rgba<u8>) {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy <= radius * radius {
                    self.set_pixel_safe(cx + dx, cy + dy, color);
                }
            }
        }
    }

    /// Draw a filled circle with a pattern
    pub fn draw_circle_filled_pattern(
        &mut self, cx: i32, cy: i32, radius: i32,
        color: Rgba<u8>, pattern: &crate::tools::Pattern,
    ) {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy <= radius * radius {
                    let px = cx + dx;
                    let py = cy + dy;
                    if px >= 0 && py >= 0 && pattern.should_fill(px as u32, py as u32) {
                        self.set_pixel_safe(px, py, color);
                    }
                }
            }
        }
    }

    pub fn draw_rect_outline(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba<u8>) {
        let (x0, x1) = if x0 < x1 { (x0, x1) } else { (x1, x0) };
        let (y0, y1) = if y0 < y1 { (y0, y1) } else { (y1, y0) };
        for x in x0..=x1 { self.set_pixel_safe(x, y0, color); self.set_pixel_safe(x, y1, color); }
        for y in y0..=y1 { self.set_pixel_safe(x0, y, color); self.set_pixel_safe(x1, y, color); }
        self.modified = true;
    }
    
    pub fn fill(&mut self, color: Rgba<u8>) {
        for pixel in self.image.pixels_mut() { *pixel = color; }
        self.modified = true;
    }
    
    pub fn clear(&mut self) { self.fill(Rgba([255, 255, 255, 255])); }
    
    pub fn invert(&mut self) {
        for pixel in self.image.pixels_mut() {
            pixel[0] = 255 - pixel[0];
            pixel[1] = 255 - pixel[1];
            pixel[2] = 255 - pixel[2];
        }
        self.modified = true;
    }
    
    pub fn flip_horizontal(&mut self) {
        let (w, h) = (self.width(), self.height());
        for y in 0..h {
            for x in 0..w / 2 {
                let left = *self.image.get_pixel(x, y);
                let right = *self.image.get_pixel(w - 1 - x, y);
                self.image.put_pixel(x, y, right);
                self.image.put_pixel(w - 1 - x, y, left);
            }
        }
        self.modified = true;
    }
    
    pub fn flip_vertical(&mut self) {
        let (w, h) = (self.width(), self.height());
        for y in 0..h / 2 {
            for x in 0..w {
                let top = *self.image.get_pixel(x, y);
                let bottom = *self.image.get_pixel(x, h - 1 - y);
                self.image.put_pixel(x, y, bottom);
                self.image.put_pixel(x, h - 1 - y, top);
            }
        }
        self.modified = true;
    }
    
    /// Convert to pure black and white (threshold at 128)
    pub fn threshold(&mut self) {
        for pixel in self.image.pixels_mut() {
            let gray = ((pixel[0] as u32 + pixel[1] as u32 + pixel[2] as u32) / 3) as u8;
            let bw = if gray > 128 { 255 } else { 0 };
            pixel[0] = bw; pixel[1] = bw; pixel[2] = bw;
        }
        self.modified = true;
    }

    /// Draw an ellipse outline using midpoint ellipse algorithm
    pub fn draw_ellipse_outline(&mut self, cx: i32, cy: i32, rx: i32, ry: i32, color: Rgba<u8>) {
        if rx <= 0 || ry <= 0 { return; }
        let (rx, ry) = (rx as i64, ry as i64);
        let (mut x, mut y) = (0i64, ry);
        let mut d1 = ry * ry - rx * rx * ry + rx * rx / 4;

        // Region 1
        while 2 * ry * ry * x <= 2 * rx * rx * y {
            self.set_pixel_safe(cx + x as i32, cy + y as i32, color);
            self.set_pixel_safe(cx - x as i32, cy + y as i32, color);
            self.set_pixel_safe(cx + x as i32, cy - y as i32, color);
            self.set_pixel_safe(cx - x as i32, cy - y as i32, color);
            x += 1;
            if d1 < 0 {
                d1 += 2 * ry * ry * x + ry * ry;
            } else {
                y -= 1;
                d1 += 2 * ry * ry * x - 2 * rx * rx * y + ry * ry;
            }
        }

        // Region 2
        let mut d2 = ry * ry * (x * 2 + 1) * (x * 2 + 1) / 4
            + rx * rx * (y - 1) * (y - 1) - rx * rx * ry * ry;
        while y >= 0 {
            self.set_pixel_safe(cx + x as i32, cy + y as i32, color);
            self.set_pixel_safe(cx - x as i32, cy + y as i32, color);
            self.set_pixel_safe(cx + x as i32, cy - y as i32, color);
            self.set_pixel_safe(cx - x as i32, cy - y as i32, color);
            y -= 1;
            if d2 > 0 {
                d2 += rx * rx - 2 * rx * rx * y;
            } else {
                x += 1;
                d2 += 2 * ry * ry * x - 2 * rx * rx * y + rx * rx;
            }
        }
        self.modified = true;
    }

    /// Draw a filled ellipse with a pattern
    pub fn draw_ellipse_filled_pattern(
        &mut self, cx: i32, cy: i32, rx: i32, ry: i32,
        color: Rgba<u8>, pattern: &crate::tools::Pattern,
    ) {
        if rx <= 0 || ry <= 0 { return; }
        let (rxf, ryf) = (rx as f64, ry as f64);
        for dy in -ry..=ry {
            for dx in -rx..=rx {
                let nx = dx as f64 / rxf;
                let ny = dy as f64 / ryf;
                if nx * nx + ny * ny <= 1.0 {
                    let px = cx + dx;
                    let py = cy + dy;
                    if px >= 0 && py >= 0 && pattern.should_fill(px as u32, py as u32) {
                        self.set_pixel(px as u32, py as u32, color);
                    }
                }
            }
        }
        self.modified = true;
    }

    /// Draw a filled rectangle with a pattern
    pub fn draw_rect_filled_pattern(
        &mut self, x0: i32, y0: i32, x1: i32, y1: i32,
        color: Rgba<u8>, pattern: &crate::tools::Pattern,
    ) {
        let (x0, x1) = if x0 < x1 { (x0, x1) } else { (x1, x0) };
        let (y0, y1) = if y0 < y1 { (y0, y1) } else { (y1, y0) };
        for y in y0..=y1 {
            for x in x0..=x1 {
                if x >= 0 && y >= 0 && pattern.should_fill(x as u32, y as u32) {
                    self.set_pixel_safe(x, y, color);
                }
            }
        }
        self.modified = true;
    }

    /// Pattern-aware flood fill
    pub fn pattern_fill(
        &mut self, start_x: u32, start_y: u32,
        fill_color: Rgba<u8>, pattern: &crate::tools::Pattern,
    ) {
        if start_x >= self.width() || start_y >= self.height() { return; }
        let target_color = *self.image.get_pixel(start_x, start_y);
        if target_color == fill_color { return; }

        let mut stack = vec![(start_x, start_y)];
        let mut visited = std::collections::HashSet::new();

        while let Some((x, y)) = stack.pop() {
            if x >= self.width() || y >= self.height() { continue; }
            if !visited.insert((x, y)) { continue; }
            if *self.image.get_pixel(x, y) != target_color { continue; }

            if pattern.should_fill(x, y) {
                self.image.put_pixel(x, y, fill_color);
            }
            // Non-pattern pixels: visited but unfilled, flood continues past them

            if x > 0 { stack.push((x - 1, y)); }
            if x < self.width() - 1 { stack.push((x + 1, y)); }
            if y > 0 { stack.push((x, y - 1)); }
            if y < self.height() - 1 { stack.push((x, y + 1)); }
        }
        self.modified = true;
    }
    
    pub fn to_texture_data(&self) -> egui::ColorImage {
        let size = [self.width() as usize, self.height() as usize];
        let pixels: Vec<egui::Color32> = self.image.pixels()
            .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
            .collect();
        egui::ColorImage { size, pixels }
    }
}
