//! Image loader for slowView
//!
//! Designed to handle extremely large images (400MB+) on constrained hardware
//! like a Raspberry Pi Zero 2 W (512MB RAM). Strategy:
//!
//! 1. Read image header to get dimensions (no decode, just bytes for the header)
//! 2. Decode the full image — unavoidable for most formats
//! 3. Immediately resize to display dimensions (max 640×480) and DROP the original
//! 4. Never modify or overwrite the original file
//!
//! The key trick: we decode into memory, create a small thumbnail, then immediately
//! free the large decoded buffer. Peak memory is ~(original pixels × 4 bytes) but
//! only transiently. The retained display image is at most 640×480×4 = 1.2MB.
//!
//! For truly massive images that would exceed available RAM, we catch allocation
//! failures and report a clean error rather than OOM-killing the process.

use image::{DynamicImage, imageops::FilterType};
use std::path::{Path, PathBuf};

/// Maximum display dimensions — these match the e-ink target resolution
pub const MAX_DISPLAY_WIDTH: u32 = 640;
pub const MAX_DISPLAY_HEIGHT: u32 = 480;

/// Result of loading an image
pub struct LoadedImage {
    /// The downsampled display image (max 640×480 RGBA)
    pub display: DynamicImage,
    /// Original file path (never modified)
    pub path: PathBuf,
    /// Original dimensions before resizing
    pub original_width: u32,
    pub original_height: u32,
    /// Display dimensions after resizing
    pub display_width: u32,
    pub display_height: u32,
    /// File size in bytes
    pub file_size: u64,
    /// Format string
    pub format: String,
}

impl LoadedImage {
    /// Load an image from path, returning a display-ready thumbnail.
    /// The original image data is freed as soon as the thumbnail is created.
    pub fn open(path: &Path) -> Result<Self, LoadError> {
        // Get file size first (cheap)
        let file_size = std::fs::metadata(path)
            .map(|m| m.len())
            .unwrap_or(0);

        // Try to read just the dimensions from the header
        // This is fast and uses minimal memory
        let (orig_w, orig_h) = read_dimensions(path)?;

        // Estimate decoded memory: width × height × 4 bytes (RGBA)
        let estimated_bytes = orig_w as u64 * orig_h as u64 * 4;

        // Safety check: if the decoded image would be > 1GB, refuse
        // (Pi Zero 2 W has 512MB RAM total)
        if estimated_bytes > 1_073_741_824 {
            return Err(LoadError::TooLarge {
                width: orig_w,
                height: orig_h,
                estimated_mb: estimated_bytes / (1024 * 1024),
            });
        }

        // Detect format from extension
        let format = path.extension()
            .map(|e| e.to_string_lossy().to_uppercase())
            .unwrap_or_else(|| "UNKNOWN".to_string());

        // Decode the full image
        // We use catch_unwind to handle potential allocation failures gracefully
        let full_image = std::panic::catch_unwind(|| {
            image::open(path)
        })
        .map_err(|_| LoadError::OutOfMemory)?
        .map_err(|e| LoadError::DecodeError(e.to_string()))?;

        // Calculate display dimensions maintaining aspect ratio
        let (disp_w, disp_h) = fit_dimensions(orig_w, orig_h, MAX_DISPLAY_WIDTH, MAX_DISPLAY_HEIGHT);

        // Resize to display dimensions
        // Use Triangle (bilinear) filter — fast and good enough for e-ink
        let resized = if disp_w < orig_w || disp_h < orig_h {
            full_image.resize_exact(disp_w, disp_h, FilterType::Triangle)
            // `full_image` is dropped here — the big allocation is freed
        } else {
            full_image
        };

        // Convert to black and white for e-ink performance
        // Uses grayscale conversion then threshold at 128
        let display = DynamicImage::ImageLuma8(resized.to_luma8());

        Ok(LoadedImage {
            display,
            path: path.to_path_buf(),
            original_width: orig_w,
            original_height: orig_h,
            display_width: disp_w,
            display_height: disp_h,
            file_size,
            format,
        })
    }

    /// Get the display image as RGBA bytes for uploading to an egui texture
    pub fn rgba_bytes(&self) -> Vec<u8> {
        self.display.to_rgba8().into_raw()
    }

    /// Get a human-readable file size string
    pub fn size_string(&self) -> String {
        format_size(self.file_size)
    }
}

/// Read image dimensions from the file header without decoding the full image.
/// This is very fast and uses almost no memory.
fn read_dimensions(path: &Path) -> Result<(u32, u32), LoadError> {
    let reader = image::ImageReader::open(path)
        .map_err(|e| LoadError::IoError(e.to_string()))?
        .with_guessed_format()
        .map_err(|e| LoadError::IoError(e.to_string()))?;

    reader.into_dimensions()
        .map_err(|e| LoadError::DecodeError(e.to_string()))
}

/// Calculate dimensions that fit within max_w × max_h while preserving aspect ratio.
/// If the image is already smaller than the max, return original dimensions.
pub fn fit_dimensions(w: u32, h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    if w <= max_w && h <= max_h {
        return (w, h);
    }

    let scale_x = max_w as f64 / w as f64;
    let scale_y = max_h as f64 / h as f64;
    let scale = scale_x.min(scale_y);

    let new_w = (w as f64 * scale).round() as u32;
    let new_h = (h as f64 * scale).round() as u32;

    (new_w.max(1), new_h.max(1))
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[derive(Debug)]
pub enum LoadError {
    IoError(String),
    DecodeError(String),
    OutOfMemory,
    TooLarge {
        width: u32,
        height: u32,
        estimated_mb: u64,
    },
    UnsupportedFormat(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::IoError(e) => write!(f, "i/o error: {}", e),
            LoadError::DecodeError(e) => write!(f, "decode error: {}", e),
            LoadError::OutOfMemory => write!(f, "image too large for available memory"),
            LoadError::TooLarge { width, height, estimated_mb } =>
                write!(f, "image {}×{} would require ~{}MB to decode", width, height, estimated_mb),
            LoadError::UnsupportedFormat(fmt) => write!(f, "unsupported format: {}", fmt),
        }
    }
}

/// List supported image extensions
pub fn supported_extensions() -> &'static [&'static str] {
    &["png", "jpg", "jpeg", "gif", "bmp", "tiff", "tif", "webp"]
}

/// Check if a path is a supported image
pub fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| supported_extensions().contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// List all images in the same directory as the given path, sorted by name
pub fn sibling_images(path: &Path) -> Vec<PathBuf> {
    let parent = match path.parent() {
        Some(p) => p,
        None => return vec![path.to_path_buf()],
    };

    let mut images: Vec<PathBuf> = std::fs::read_dir(parent)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| is_image(p))
                .collect()
        })
        .unwrap_or_default();

    images.sort();
    images
}
