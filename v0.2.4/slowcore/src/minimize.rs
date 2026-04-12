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
    /// Process ID (0 if the desktop has auto-suspended this app —
    /// clicking its taskbar entry will re-launch the binary fresh)
    pub pid: u32,
    /// Unix seconds when the app was minimized. Used by the desktop
    /// to auto-suspend long-idle processes to free RAM on low-memory
    /// hardware (Pi Zero 2W).
    #[serde(default)]
    pub minimized_at: u64,
}

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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
        minimized_at: now_unix_secs(),
    };
    let path = minimized_dir().join(format!("{}_{}.json", binary, state.pid));
    if let Ok(json) = serde_json::to_string(&state) {
        let _ = std::fs::write(path, json);
    }
}

/// Write a "suspended" entry for a binary whose process has been killed.
/// The taskbar keeps showing it so the user can relaunch. Uses a special
/// `_suspended_{binary}.json` filename so the file isn't tied to a live PID.
pub fn write_suspended(binary: &str, title: &str) {
    let state = MinimizedApp {
        binary: binary.to_string(),
        title: title.to_string(),
        pid: 0,
        minimized_at: now_unix_secs(),
    };
    let path = minimized_dir().join(format!("_suspended_{}.json", binary));
    if let Ok(json) = serde_json::to_string(&state) {
        let _ = std::fs::write(path, json);
    }
}

/// Remove the suspended marker for a binary (called when the desktop
/// re-launches a suspended app).
pub fn clear_suspended(binary: &str) {
    let path = minimized_dir().join(format!("_suspended_{}.json", binary));
    let _ = std::fs::remove_file(path);
}

/// Clear minimized state for this process
pub fn clear_minimized(binary: &str) {
    let pid = std::process::id();
    let path = minimized_dir().join(format!("{}_{}.json", binary, pid));
    let _ = std::fs::remove_file(path);
}

/// Read all minimized apps (used by the desktop). Entries with pid=0
/// are "suspended" — the process was auto-killed to free RAM and will
/// be re-launched when the user clicks the taskbar entry.
pub fn read_all_minimized() -> Vec<MinimizedApp> {
    let dir = minimized_dir();
    let mut results = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(json) = std::fs::read_to_string(&path) {
                    if let Ok(state) = serde_json::from_str::<MinimizedApp>(&json) {
                        if state.pid == 0 {
                            // Suspended entry — keep as-is
                            results.push(state);
                        } else if is_process_alive(state.pid) {
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

/// How long a minimized app can sit idle before the desktop auto-kills
/// it. Defaults to 10 minutes but can be overridden by setting the
/// `SLOWOS_IDLE_KILL_SECS` environment variable (0 disables the feature).
pub fn idle_kill_secs() -> u64 {
    std::env::var("SLOWOS_IDLE_KILL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(600)
}

/// Remove a specific minimized entry (used by the desktop when restoring).
/// Also writes a restore signal file so the app can unminimize itself.
pub fn remove_minimized(binary: &str, pid: u32) {
    let path = minimized_dir().join(format!("{}_{}.json", binary, pid));
    let _ = std::fs::remove_file(path);
    // Signal the app to restore itself by writing a restore file
    let restore_path = minimized_dir().join(format!("restore_{}_{}", binary, pid));
    let _ = std::fs::write(restore_path, "1");
}

/// Check if this process has been asked to restore, and clear the signal.
/// Apps should call this every frame and issue `Minimized(false)` if true.
pub fn check_restore_signal(binary: &str) -> bool {
    let pid = std::process::id();
    let restore_path = minimized_dir().join(format!("restore_{}_{}", binary, pid));
    if restore_path.exists() {
        let _ = std::fs::remove_file(restore_path);
        true
    } else {
        false
    }
}

/// Check if a process is still running
fn is_process_alive(pid: u32) -> bool {
    // Check /proc/{pid} on Linux
    std::path::Path::new(&format!("/proc/{}", pid)).exists()
}
