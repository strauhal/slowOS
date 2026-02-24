//! Inter-application drag-and-drop support
//!
//! Uses a temp file to communicate drag state between slowOS applications.
//! When one app starts dragging files, it writes their paths to a temp file.
//! Other apps can check for this file to accept drops.

use std::fs;
use std::path::PathBuf;

/// Get the path to the drag state file
fn drag_state_path() -> PathBuf {
    std::env::temp_dir().join("slowos_drag_state.txt")
}

/// Start a drag operation with the given file paths
/// Called by source app (e.g., Files) when drag begins
pub fn start_drag(paths: &[PathBuf]) {
    if paths.is_empty() {
        return;
    }
    let content: Vec<String> = paths.iter()
        .filter_map(|p| p.to_str())
        .map(|s| s.to_string())
        .collect();
    let _ = fs::write(drag_state_path(), content.join("\n"));
}

/// End/cancel a drag operation
/// Called when drag ends (drop or cancel)
pub fn end_drag() {
    let _ = fs::remove_file(drag_state_path());
}

/// Check if there's an active drag operation and get the paths
/// Returns None if no drag is active or paths couldn't be read
pub fn get_drag_paths() -> Option<Vec<PathBuf>> {
    let path = drag_state_path();
    if !path.exists() {
        return None;
    }

    // Only return paths if the file is recent (within last 30 seconds)
    // This prevents stale drag state from persisting
    if let Ok(meta) = fs::metadata(&path) {
        if let Ok(modified) = meta.modified() {
            if let Ok(elapsed) = modified.elapsed() {
                if elapsed.as_secs() > 30 {
                    let _ = fs::remove_file(&path);
                    return None;
                }
            }
        }
    }

    let content = fs::read_to_string(&path).ok()?;
    let paths: Vec<PathBuf> = content
        .lines()
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .collect();

    if paths.is_empty() {
        None
    } else {
        Some(paths)
    }
}

/// Check if a drag is currently in progress
pub fn is_drag_active() -> bool {
    get_drag_paths().is_some()
}
