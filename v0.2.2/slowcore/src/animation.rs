//! Animation system for SlowOS
//!
//! Provides smooth, e-ink friendly animations for window operations.
//! Animations use expanding/contracting rectangle outlines to create
//! a classic "zoom" effect like early Macintosh computers.

use egui::{Color32, Painter, Pos2, Rect, Stroke};

/// Duration of window open/close animations in seconds.
/// Tuned for e-ink: ~4 frames at 250ms intervals.
pub const ANIMATION_DURATION: f32 = 0.8;

/// Number of rectangle outlines to draw during animation
pub const ANIMATION_STEPS: usize = 4;

/// Types of animations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationType {
    /// Window opening - rectangles expand from icon to window
    WindowOpen,
    /// Window closing - rectangles contract from window to icon
    WindowClose,
}

/// State of an active animation
#[derive(Debug, Clone)]
pub struct Animation {
    /// Type of animation
    pub anim_type: AnimationType,
    /// Starting rectangle (icon position)
    pub start_rect: Rect,
    /// Ending rectangle (window position)
    pub end_rect: Rect,
    /// Animation progress (0.0 to 1.0)
    pub progress: f32,
    /// Associated app binary name (for tracking)
    pub app_binary: String,
    /// Whether animation is complete
    pub completed: bool,
}

impl Animation {
    /// Create a new window open animation
    pub fn window_open(icon_rect: Rect, window_rect: Rect, app_binary: String) -> Self {
        Self {
            anim_type: AnimationType::WindowOpen,
            start_rect: icon_rect,
            end_rect: window_rect,
            progress: 0.0,
            app_binary,
            completed: false,
        }
    }

    /// Create a new window close animation
    pub fn window_close(window_rect: Rect, icon_rect: Rect, app_binary: String) -> Self {
        Self {
            anim_type: AnimationType::WindowClose,
            start_rect: window_rect,
            end_rect: icon_rect,
            progress: 0.0,
            app_binary,
            completed: false,
        }
    }

    /// Update animation progress based on delta time
    pub fn update(&mut self, dt: f32) {
        self.progress += dt / ANIMATION_DURATION;
        if self.progress >= 1.0 {
            self.progress = 1.0;
            self.completed = true;
        }
    }

    /// Get the current interpolated rectangle for a given step
    fn get_step_rect(&self, step: usize) -> Rect {
        // Each step has a slightly different timing offset
        let step_offset = (step as f32) / (ANIMATION_STEPS as f32) * 0.15;
        let t = (self.progress - step_offset).clamp(0.0, 1.0);

        // Use easing function for smooth animation
        let eased_t = ease_out_quad(t);

        lerp_rect(self.start_rect, self.end_rect, eased_t)
    }

    /// Draw the animation (multiple expanding/contracting rectangle outlines)
    pub fn draw(&self, painter: &Painter) {
        for step in 0..ANIMATION_STEPS {
            let rect = self.get_step_rect(step);

            // Draw rectangle outline with 1px black stroke
            painter.rect_stroke(
                rect,
                0.0,
                Stroke::new(1.0, Color32::BLACK),
            );
        }
    }
}

/// Animation manager - tracks all active animations
#[derive(Debug, Default)]
pub struct AnimationManager {
    /// Currently running animations
    animations: Vec<Animation>,
    /// Pending app launches (waiting for animation to complete)
    pending_launches: Vec<String>,
}

impl AnimationManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a window open animation from icon rect to window rect
    pub fn start_open_to(&mut self, icon_rect: Rect, window_rect: Rect, app_binary: String) {
        self.animations.push(Animation::window_open(icon_rect, window_rect, app_binary.clone()));
        self.pending_launches.push(app_binary);
    }

    /// Start a window close animation
    pub fn start_close(&mut self, window_rect: Rect, icon_rect: Rect, app_binary: String) {
        self.animations.push(Animation::window_close(window_rect, icon_rect, app_binary));
    }

    /// Update all animations, returns list of apps that should now be launched
    pub fn update(&mut self, dt: f32) -> Vec<String> {
        let mut to_launch = Vec::new();

        for anim in &mut self.animations {
            anim.update(dt);

            // When open animation completes, the app should be launched
            if anim.completed && anim.anim_type == AnimationType::WindowOpen {
                if let Some(pos) = self.pending_launches.iter().position(|b| b == &anim.app_binary) {
                    to_launch.push(self.pending_launches.remove(pos));
                }
            }
        }

        // Remove completed animations
        self.animations.retain(|a| !a.completed);

        to_launch
    }

    /// Draw all active animations
    pub fn draw(&self, painter: &Painter) {
        for anim in &self.animations {
            anim.draw(painter);
        }
    }

    /// Check if any animations are currently running
    pub fn is_animating(&self) -> bool {
        !self.animations.is_empty()
    }

    /// Check if a specific app is currently animating
    pub fn is_app_animating(&self, binary: &str) -> bool {
        self.animations.iter().any(|a| a.app_binary == binary)
    }

    /// Get count of active animations
    pub fn animation_count(&self) -> usize {
        self.animations.len()
    }
}

/// Linear interpolation between two rectangles
fn lerp_rect(a: Rect, b: Rect, t: f32) -> Rect {
    Rect::from_min_max(
        Pos2::new(
            lerp(a.min.x, b.min.x, t),
            lerp(a.min.y, b.min.y, t),
        ),
        Pos2::new(
            lerp(a.max.x, b.max.x, t),
            lerp(a.max.y, b.max.y, t),
        ),
    )
}

/// Linear interpolation between two values
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Quadratic ease-out function for smooth deceleration
fn ease_out_quad(t: f32) -> f32 {
    1.0 - (1.0 - t) * (1.0 - t)
}
