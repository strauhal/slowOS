//! slowcore — shared library for slow computer applications

pub mod animation;
pub mod dither;
pub mod safety;
pub mod storage;
pub mod theme;
pub mod widgets;

pub use animation::AnimationManager;
pub use theme::SlowTheme;
