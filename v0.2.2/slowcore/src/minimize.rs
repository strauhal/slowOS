//! Minimize IPC — file-based communication between apps and the desktop
//!
//! When an app is minimized, it writes a state file to ~/.config/slowos/minimized/.
//! The desktop polls this directory to show minimized apps in the status bar.
//! When the user clicks a minimized app in the status bar, the desktop restores it.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// State of a minimized application
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MinimizedApp {
    /// Binary name (e.g. "slowwrite")
    pub binary: String,
    /// Display title (e.g. "letter.txt — slowWrite" or "calculator")
    pub title: String,
    /// Process ID
    pub pid: u32,
}

/// Directory for minimized state files
fn minimized_dir() -> PathBuf {
    let dir = directories::ProjectDirs::from("", "", "slowos")
        .map(|p| p.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/tmp/slowos"))
        .join("minimized");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Write minimized state for this process
pub fn write_minimized(binary: &str, title: &str) {
    let state = MinimizedApp {
        binary: binary.to_string(),
        title: title.to_string(),
        pid: std::process::id(),
    };
    let path = minimized_dir().join(format!("{}_{}.json", binary, state.pid));
    if let Ok(json) = serde_json::to_string(&state) {
        let _ = std::fs::write(path, json);
    }
}

/// Clear minimized state for this process
pub fn clear_minimized(binary: &str) {
    let pid = std::process::id();
    let path = minimized_dir().join(format!("{}_{}.json", binary, pid));
    let _ = std::fs::remove_file(path);
}

/// Read all minimized apps (used by the desktop)
pub fn read_all_minimized() -> Vec<MinimizedApp> {
    let dir = minimized_dir();
    let mut results = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(json) = std::fs::read_to_string(&path) {
                    if let Ok(state) = serde_json::from_str::<MinimizedApp>(&json) {
                        // Verify the process is still alive
                        if is_process_alive(state.pid) {
                            results.push(state);
                        } else {
                            // Stale file — process died without cleaning up
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                }
            }
        }
    }
    results
}

/// Remove a specific minimized entry (used by the desktop when restoring)
pub fn remove_minimized(binary: &str, pid: u32) {
    let path = minimized_dir().join(format!("{}_{}.json", binary, pid));
    let _ = std::fs::remove_file(path);
}

/// Check if a process is still running
fn is_process_alive(pid: u32) -> bool {
    // Check /proc/{pid} on Linux
    std::path::Path::new(&format!("/proc/{}", pid)).exists()
}
