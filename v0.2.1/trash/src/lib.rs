//! Trash library — provides `move_to_trash` for other slow computer apps.

mod app;

pub use app::move_to_trash;
pub use app::trash_dir;
pub use app::restore_from_trash;
