//! App launcher utilities
//!
//! Handles binary resolution and launch configuration.

use std::path::PathBuf;

/// Resolve the directory containing SlowOS app binaries.
///
/// Search order:
/// 1. Same directory as the running desktop binary
/// 2. /usr/bin (Buildroot production)
/// 3. ./target/release (local Cargo build)
/// 4. ./target/debug (local Cargo debug build)
pub fn resolve_bin_dir() -> PathBuf {
    // 1. Same directory as current executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let test_path = dir.join("slowwrite");
            if test_path.exists() {
                return dir.to_path_buf();
            }
            // Also check without extension on Windows
            let test_path_exe = dir.join("slowwrite.exe");
            if test_path_exe.exists() {
                return dir.to_path_buf();
            }
        }
    }

    // 2. /usr/bin (production)
    let usr_bin = PathBuf::from("/usr/bin");
    if usr_bin.join("slowwrite").exists() {
        return usr_bin;
    }

    // 3. ./target/release
    let release = PathBuf::from("./target/release");
    if release.join("slowwrite").exists() {
        return release;
    }

    // 4. ./target/debug
    let debug = PathBuf::from("./target/debug");
    if debug.join("slowwrite").exists() {
        return debug;
    }

    // Fallback: current directory
    PathBuf::from(".")
}

/// Check which apps have binaries available
pub fn available_apps(bin_dir: &PathBuf) -> Vec<String> {
    let all_apps = vec![
        "slowwrite",
        "slowpaint",
        "slowreader",
        "slowsheets",
        "slownotes",
        "slowchess",
        "files",
        "slowmusic",
        "slowslides",
        "slowtex",
        "trash",
        "slowterm",
        "slowpics",
    ];

    all_apps
        .into_iter()
        .filter(|name| {
            let path = bin_dir.join(name);
            path.exists() || bin_dir.join(format!("{}.exe", name)).exists()
        })
        .map(|s| s.to_string())
        .collect()
}
