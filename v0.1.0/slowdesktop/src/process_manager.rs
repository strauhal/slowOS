//! Process manager for SlowOS applications
//!
//! Manages child processes for each app. Tracks running state,
//! handles clean shutdown, and optionally restarts crashed apps.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command};

/// Information about a SlowOS application
#[derive(Debug, Clone)]
pub struct AppInfo {
    /// Binary name (e.g. "slowwrite")
    pub binary: String,
    /// Display name (e.g. "slowWrite")
    pub display_name: String,
    /// Short description
    pub description: String,
    /// Icon label (text glyph used on desktop)
    pub icon_label: String,
    /// Whether this app is currently running
    pub running: bool,
}

/// Manages running application processes
pub struct ProcessManager {
    /// Registry of all known applications
    apps: Vec<AppInfo>,
    /// Running child processes, keyed by binary name
    children: HashMap<String, Child>,
    /// Path to search for app binaries
    bin_paths: Vec<PathBuf>,
}

impl ProcessManager {
    pub fn new() -> Self {
        let mut pm = Self {
            apps: Vec::new(),
            children: HashMap::new(),
            bin_paths: vec![
                // Development: next to the desktop binary
                std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                    .unwrap_or_else(|| PathBuf::from(".")),
                // Buildroot: /usr/bin
                PathBuf::from("/usr/bin"),
                // Local builds
                PathBuf::from("./target/release"),
                PathBuf::from("./target/debug"),
            ],
        };
        pm.register_apps();
        pm
    }

    fn register_apps(&mut self) {
        self.apps = vec![
            AppInfo {
                binary: "slowwrite".into(),
                display_name: "slowWrite".into(),
                description: "word processor".into(),
                icon_label: "W".into(),
                running: false,
            },
            AppInfo {
                binary: "slowpaint".into(),
                display_name: "slowPaint".into(),
                description: "bitmap editor".into(),
                icon_label: "P".into(),
                running: false,
            },
            AppInfo {
                binary: "slowbooks".into(),
                display_name: "slowBooks".into(),
                description: "ebook reader".into(),
                icon_label: "B".into(),
                running: false,
            },
            AppInfo {
                binary: "slowsheets".into(),
                display_name: "slowSheets".into(),
                description: "spreadsheet".into(),
                icon_label: "S".into(),
                running: false,
            },
            AppInfo {
                binary: "slownotes".into(),
                display_name: "slowNotes".into(),
                description: "notes".into(),
                icon_label: "N".into(),
                running: false,
            },
            AppInfo {
                binary: "slowchess".into(),
                display_name: "slowChess".into(),
                description: "chess".into(),
                icon_label: "♟".into(),
                running: false,
            },
            AppInfo {
                binary: "files".into(),
                display_name: "files".into(),
                description: "file manager".into(),
                icon_label: "F".into(),
                running: false,
            },
            AppInfo {
                binary: "slowmusic".into(),
                display_name: "slowMusic".into(),
                description: "music player".into(),
                icon_label: "♪".into(),
                running: false,
            },
            AppInfo {
                binary: "slowslides".into(),
                display_name: "slowSlides".into(),
                description: "presentations".into(),
                icon_label: "▶".into(),
                running: false,
            },
            AppInfo {
                binary: "slowtex".into(),
                display_name: "slowTeX".into(),
                description: "LaTeX editor".into(),
                icon_label: "T".into(),
                running: false,
            },
            AppInfo {
                binary: "trash".into(),
                display_name: "trash".into(),
                description: "trash bin".into(),
                icon_label: "🗑".into(),
                running: false,
            },
            AppInfo {
                binary: "slowterm".into(),
                display_name: "slowTerm".into(),
                description: "terminal".into(),
                icon_label: ">_".into(),
                running: false,
            },
            AppInfo {
                binary: "slowpics".into(),
                display_name: "slowPics".into(),
                description: "image viewer".into(),
                icon_label: "◻".into(),
                running: false,
            },
        ];
    }

    /// Get all registered apps
    pub fn apps(&self) -> &[AppInfo] {
        &self.apps
    }

    /// Find the binary path for an app
    fn find_binary(&self, binary: &str) -> Option<PathBuf> {
        for base in &self.bin_paths {
            let path = base.join(binary);
            if path.exists() && path.is_file() {
                return Some(path);
            }
        }
        None
    }

    /// Launch an application. If already running, bring to focus (on X11/Wayland).
    /// Returns Ok(true) if launched, Ok(false) if already running, Err on failure.
    pub fn launch(&mut self, binary: &str) -> Result<bool, String> {
        // Check if already running
        if let Some(child) = self.children.get_mut(binary) {
            match child.try_wait() {
                Ok(Some(_status)) => {
                    // Process exited, remove it and allow relaunch
                    self.children.remove(binary);
                }
                Ok(None) => {
                    // Still running
                    return Ok(false);
                }
                Err(e) => {
                    // Error checking status, remove stale entry
                    eprintln!("[slowdesktop] error checking {}: {}", binary, e);
                    self.children.remove(binary);
                }
            }
        }

        // Find the binary
        let bin_path = self.find_binary(binary).ok_or_else(|| {
            format!(
                "binary '{}' not found in paths: {:?}",
                binary, self.bin_paths
            )
        })?;

        // Launch with panic isolation
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Command::new(&bin_path)
                .env("SLOWOS_MANAGED", "1")
                .spawn()
        }));

        match result {
            Ok(Ok(child)) => {
                self.children.insert(binary.to_string(), child);
                // Update running status
                if let Some(app) = self.apps.iter_mut().find(|a| a.binary == binary) {
                    app.running = true;
                }
                Ok(true)
            }
            Ok(Err(e)) => Err(format!("failed to spawn {}: {}", binary, e)),
            Err(_) => Err(format!("panic while spawning {}", binary)),
        }
    }

    /// Poll all running processes and update their status.
    /// Returns list of apps that have exited since last poll.
    pub fn poll(&mut self) -> Vec<String> {
        let mut exited = Vec::new();

        let binaries: Vec<String> = self.children.keys().cloned().collect();
        for binary in binaries {
            if let Some(child) = self.children.get_mut(&binary) {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        if !status.success() {
                            eprintln!(
                                "[slowdesktop] {} exited with status: {}",
                                binary, status
                            );
                        }
                        exited.push(binary.clone());
                    }
                    Ok(None) => {
                        // Still running
                    }
                    Err(e) => {
                        eprintln!(
                            "[slowdesktop] error polling {}: {}",
                            binary, e
                        );
                        exited.push(binary.clone());
                    }
                }
            }
        }

        // Clean up exited processes
        for binary in &exited {
            self.children.remove(binary);
            if let Some(app) = self.apps.iter_mut().find(|a| a.binary == *binary) {
                app.running = false;
            }
        }

        exited
    }

    /// Shut down all running applications gracefully
    pub fn shutdown_all(&mut self) {
        let binaries: Vec<String> = self.children.keys().cloned().collect();
        for binary in binaries {
            if let Some(mut child) = self.children.remove(&binary) {
                // Try graceful kill first (SIGTERM)
                let _ = child.kill();
                // Give it a moment
                let _ = child.wait();
            }
        }
        for app in &mut self.apps {
            app.running = false;
        }
    }

    /// Number of currently running apps
    pub fn running_count(&self) -> usize {
        self.children.len()
    }

    /// Check if a specific app is running
    pub fn is_running(&self, binary: &str) -> bool {
        self.children.contains_key(binary)
    }
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        self.shutdown_all();
    }
}
