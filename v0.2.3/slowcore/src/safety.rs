//! Safety utilities for crash-proof SlowOS applications.
//!
//! These helpers eliminate common panic sources: string slicing on
//! non-UTF-8 boundaries and unhandled panics in per-frame rendering.

use std::path::Path;

/// Snap a byte position to the nearest valid UTF-8 character boundary.
/// If `byte_pos` is already on a boundary, returns it unchanged.
/// Otherwise walks backward (up to 3 bytes) to find the boundary.
pub fn snap_to_char_boundary(s: &str, byte_pos: usize) -> usize {
    let len = s.len();
    if byte_pos >= len {
        return len;
    }
    if s.is_char_boundary(byte_pos) {
        return byte_pos;
    }
    // Walk backward up to 3 bytes (max UTF-8 char width)
    for offset in 1..=3 {
        let pos = byte_pos.saturating_sub(offset);
        if s.is_char_boundary(pos) {
            return pos;
        }
    }
    0
}

/// Safe string slice from start to `byte_pos`.
/// Returns `&s[..snapped]` where snapped is on a valid char boundary.
pub fn safe_slice_to(s: &str, byte_pos: usize) -> &str {
    let pos = snap_to_char_boundary(s, byte_pos);
    &s[..pos]
}

/// Safe string slice from `byte_pos` to end.
/// Returns `&s[snapped..]` where snapped is on a valid char boundary.
pub fn safe_slice_from(s: &str, byte_pos: usize) -> &str {
    let pos = snap_to_char_boundary(s, byte_pos);
    &s[pos..]
}

/// Run a closure, catching any panic. Returns the closure result on success,
/// or `fallback` on panic. Useful for per-frame rendering isolation.
pub fn catch_or<T>(fallback: T, f: impl FnOnce() -> T) -> T {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(val) => val,
        Err(_) => {
            eprintln!("[slowos] caught panic in frame — recovered");
            fallback
        }
    }
}

/// System folder names that live directly under the home directory.
const SYSTEM_FOLDERS: &[&str] = &[
    "Documents", "documents",
    "Pictures", "pictures",
    "Music", "music",
    "Books", "books",
    "MIDI", "midi",
    "Downloads", "downloads",
];

/// Bundled content folders that ship with slowOS and must not be deleted.
const BUNDLED_CONTENT: &[&str] = &[
    "slowLibrary",
    "slowMuseum",
    "compositions",
    "Kimiko Ishizaka - J.S. Bach- -Open- Goldberg Variations- BWV 988 (Piano)",
    "computerdrawing.club",
    "icons_process",
];

/// Check whether a path is a protected system folder or bundled content.
///
/// Protected paths:
/// - `~/Documents`, `~/Pictures`, `~/Music`, `~/Books`, `~/MIDI`, `~/Downloads`
/// - Bundled content inside those folders (slowLibrary, slowMuseum, compositions, etc.)
///
/// Returns `true` if the path should NOT be deleted.
pub fn is_system_path(path: &Path) -> bool {
    let home = match directories::BaseDirs::new() {
        Some(d) => d.home_dir().to_path_buf(),
        None => return false,
    };

    // Direct children of home that are system folders
    if path.parent() == Some(&home) {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if SYSTEM_FOLDERS.contains(&name) {
                return true;
            }
        }
    }

    // Bundled content anywhere under home
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if BUNDLED_CONTENT.contains(&name) && path.starts_with(&home) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snap_ascii() {
        let s = "hello";
        assert_eq!(snap_to_char_boundary(s, 0), 0);
        assert_eq!(snap_to_char_boundary(s, 3), 3);
        assert_eq!(snap_to_char_boundary(s, 5), 5);
        assert_eq!(snap_to_char_boundary(s, 100), 5);
    }

    #[test]
    fn test_snap_cjk() {
        // '中' is 3 bytes in UTF-8
        let s = "中文";
        assert_eq!(snap_to_char_boundary(s, 0), 0);
        assert_eq!(snap_to_char_boundary(s, 1), 0); // mid-char → snap back
        assert_eq!(snap_to_char_boundary(s, 2), 0);
        assert_eq!(snap_to_char_boundary(s, 3), 3); // boundary of '文'
        assert_eq!(snap_to_char_boundary(s, 4), 3);
        assert_eq!(snap_to_char_boundary(s, 6), 6);
    }

    #[test]
    fn test_snap_emoji() {
        // '😀' is 4 bytes in UTF-8
        let s = "a😀b";
        assert_eq!(snap_to_char_boundary(s, 0), 0); // 'a'
        assert_eq!(snap_to_char_boundary(s, 1), 1); // start of emoji
        assert_eq!(snap_to_char_boundary(s, 2), 1); // mid-emoji
        assert_eq!(snap_to_char_boundary(s, 3), 1); // mid-emoji
        assert_eq!(snap_to_char_boundary(s, 4), 1); // mid-emoji
        assert_eq!(snap_to_char_boundary(s, 5), 5); // 'b'
    }

    #[test]
    fn test_safe_slice() {
        let s = "café"; // 'é' is 2 bytes
        assert_eq!(safe_slice_to(s, 3), "caf");
        assert_eq!(safe_slice_to(s, 4), "caf"); // mid-char, snaps back
        assert_eq!(safe_slice_to(s, 5), "café");
        assert_eq!(safe_slice_from(s, 3), "fé");
    }

    #[test]
    fn test_empty_string() {
        let s = "";
        assert_eq!(snap_to_char_boundary(s, 0), 0);
        assert_eq!(snap_to_char_boundary(s, 5), 0);
        assert_eq!(safe_slice_to(s, 0), "");
        assert_eq!(safe_slice_from(s, 0), "");
    }
}
