//! Drawing tools for SlowPaint — e-ink edition
//!
//! Black and white only. No colors. Dither patterns for fills.

use image::Rgba;

/// The two colors that exist on an e-ink display.
pub const BLACK: Rgba<u8> = Rgba([0, 0, 0, 255]);
pub const WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);

/// Available drawing tools
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Pencil,
    Brush,
    Eraser,
    Line,
    Rectangle,
    FilledRectangle,
    Ellipse,
    FilledEllipse,
    Fill,
    Marquee,
    Lasso,
    /// Selection move tool - appears when content is cut/copied
    Select,
}

impl Tool {
    pub fn name(&self) -> &'static str {
        match self {
            Tool::Pencil => "pencil",
            Tool::Brush => "brush",
            Tool::Eraser => "eraser",
            Tool::Line => "line",
            Tool::Rectangle => "rectangle",
            Tool::FilledRectangle => "filled rect",
            Tool::Ellipse => "ellipse",
            Tool::FilledEllipse => "filled ellipse",
            Tool::Fill => "fill",
            Tool::Marquee => "marquee",
            Tool::Lasso => "lasso",
            Tool::Select => "select",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Tool::Pencil => "pen",
            Tool::Brush => "brush",
            Tool::Eraser => "erase",
            Tool::Line => "line",
            Tool::Rectangle => "rect",
            Tool::FilledRectangle => "f.rect",
            Tool::Ellipse => "oval",
            Tool::FilledEllipse => "f.oval",
            Tool::Fill => "fill",
            Tool::Marquee => "marq",
            Tool::Lasso => "lasso",
            Tool::Select => "sel",
        }
    }

    /// All available tools in toolbar order
    pub fn all() -> &'static [Tool] {
        &[
            Tool::Marquee,
            Tool::Lasso,
            Tool::Pencil,
            Tool::Brush,
            Tool::Eraser,
            Tool::Line,
            Tool::Rectangle,
            Tool::FilledRectangle,
            Tool::Ellipse,
            Tool::FilledEllipse,
            Tool::Fill,
        ]
    }

    /// Does this tool draw continuously while dragging?
    pub fn is_continuous(&self) -> bool {
        matches!(self, Tool::Pencil | Tool::Brush | Tool::Eraser)
    }

    /// Does this tool need a drag to complete (start + end point)?
    pub fn is_shape(&self) -> bool {
        matches!(
            self,
            Tool::Line | Tool::Rectangle | Tool::FilledRectangle | Tool::Ellipse | Tool::FilledEllipse | Tool::Marquee
        )
    }

    /// Is this a selection tool?
    #[allow(dead_code)]
    pub fn is_selection(&self) -> bool {
        matches!(self, Tool::Marquee | Tool::Lasso | Tool::Select)
    }
}

/// Brush size options
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrushSize {
    Size1,
    Size2,
    Size4,
    Size8,
    Size16,
}

impl BrushSize {
    pub fn pixels(&self) -> u32 {
        match self {
            BrushSize::Size1 => 1,
            BrushSize::Size2 => 2,
            BrushSize::Size4 => 4,
            BrushSize::Size8 => 8,
            BrushSize::Size16 => 16,
        }
    }

    pub fn all() -> &'static [BrushSize] {
        &[
            BrushSize::Size1,
            BrushSize::Size2,
            BrushSize::Size4,
            BrushSize::Size8,
            BrushSize::Size16,
        ]
    }
}

/// Fill pattern options (classic MacPaint style)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    Solid,
    Checkerboard,
    HorizontalLines,
    VerticalLines,
    DiagonalRight,
    DiagonalLeft,
    Dots,
}

impl Pattern {
    pub fn all() -> &'static [Pattern] {
        &[
            Pattern::Solid,
            Pattern::Checkerboard,
            Pattern::HorizontalLines,
            Pattern::VerticalLines,
            Pattern::DiagonalRight,
            Pattern::DiagonalLeft,
            Pattern::Dots,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Pattern::Solid => "solid",
            Pattern::Checkerboard => "checker",
            Pattern::HorizontalLines => "h-lines",
            Pattern::VerticalLines => "v-lines",
            Pattern::DiagonalRight => "diag ╱",
            Pattern::DiagonalLeft => "diag ╲",
            Pattern::Dots => "dots",
        }
    }

    /// Check if a pixel should be filled based on pattern
    pub fn should_fill(&self, x: u32, y: u32) -> bool {
        match self {
            Pattern::Solid => true,
            Pattern::Checkerboard => (x + y) % 2 == 0,
            Pattern::HorizontalLines => y % 2 == 0,
            Pattern::VerticalLines => x % 2 == 0,
            Pattern::DiagonalRight => (x + y) % 4 < 2,
            Pattern::DiagonalLeft => ((x as i32 - y as i32).abs() as u32) % 4 < 2,
            Pattern::Dots => x % 2 == 0 && y % 2 == 0,
        }
    }
}
