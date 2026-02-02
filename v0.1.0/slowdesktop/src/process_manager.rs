//! Process manager for SlowOS applications
//!
//! Manages child processes for each app. Tracks running state,
//! handles clean shutdown, and provides robust error handling.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

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

/// Process state for tracking
#[derive(Debug)]
struct ProcessState {
    child: Child,
    started_at: Instant,
}

/// Manages running application processes
pub struct ProcessManager {
    /// Registry of all known applications
    apps: Vec<AppInfo>,
    /// Running child processes, keyed by binary name
    children: HashMap<String, ProcessState>,
    /// Path to search for app binaries
    bin_paths: Vec<PathBuf>,
    /// Apps that failed to launch (with error message)
    failed_launches: HashMap<String, String>,
}

impl ProcessManager {
    pub fn new() -> Self {
        let mut pm = Self {
            apps: Vec::new(),
            children: HashMap::new(),
            bin_paths: Self::build_bin_paths(),
            failed_launches: HashMap::new(),
        };
        pm.register_apps();
        pm
    }

    /// Build the list of paths to search for binaries
    fn build_bin_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // 1. Same directory as current executable (most reliable for development)
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                paths.push(dir.to_path_buf());
            }
        }

        // 2. Buildroot: /usr/bin
        paths.push(PathBuf::from("/usr/bin"));

        // 3. Absolute path to workspace builds (works regardless of cwd)
        // Look for the workspace root by finding Cargo.toml
        if let Ok(exe) = std::env::current_exe() {
            let mut search_dir = exe.parent().map(|p| p.to_path_buf());
            while let Some(dir) = search_dir {
                if dir.join("Cargo.toml").exists() {
                    paths.push(dir.join("target/debug"));
                    paths.push(dir.join("target/release"));
                    break;
                }
                search_dir = dir.parent().map(|p| p.to_path_buf());
            }
        }

        // 4. Local workspace builds (relative to cwd)
        if let Ok(cwd) = std::env::current_dir() {
            paths.push(cwd.join("target/release"));
            paths.push(cwd.join("target/debug"));
        }

        // 5. Fallback relative paths
        paths.push(PathBuf::from("./target/release"));
        paths.push(PathBuf::from("./target/debug"));

        paths
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
                icon_label: "c".into(),
                running: false,
            },
            AppInfo {
                binary: "slowfiles".into(),
                display_name: "slowFiles".into(),
                description: "file manager".into(),
                icon_label: "F".into(),
                running: false,
            },
            AppInfo {
                binary: "slowmusic".into(),
                display_name: "slowMusic".into(),
                description: "music player".into(),
                icon_label: "M".into(),
                running: false,
            },
            AppInfo {
                binary: "slowslides".into(),
                display_name: "slowSlides".into(),
                description: "presentations".into(),
                icon_label: "L".into(),
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
                icon_label: "X".into(),
                running: false,
            },
            AppInfo {
                binary: "slowterm".into(),
                display_name: "slowTerm".into(),
                description: "terminal".into(),
                icon_label: ">".into(),
                running: false,
            },
            AppInfo {
                binary: "slowpics".into(),
                display_name: "slowPics".into(),
                description: "image viewer".into(),
                icon_label: "I".into(),
                running: false,
            },
            AppInfo {
                binary: "credits".into(),
                display_name: "credits".into(),
                description: "open source credits".into(),
                icon_label: "C".into(),
                running: false,
            },
            AppInfo {
                binary: "slowmidi".into(),
                display_name: "slowMidi".into(),
                description: "MIDI sequencer".into(),
                icon_label: "m".into(),
                running: false,
            },
            AppInfo {
                binary: "slowbreath".into(),
                display_name: "slowBreath".into(),
                description: "breathing timer".into(),
                icon_label: "~".into(),
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
                // Verify it's executable (on Unix)
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(meta) = path.metadata() {
                        if meta.permissions().mode() & 0o111 != 0 {
                            return Some(path);
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    return Some(path);
                }
            }
            // Try with .exe extension on Windows
            #[cfg(windows)]
            {
                let path_exe = base.join(format!("{}.exe", binary));
                if path_exe.exists() && path_exe.is_file() {
                    return Some(path_exe);
                }
            }
        }
        None
    }

    /// Launch an application. If already running, return false.
    /// Returns Ok(true) if launched, Ok(false) if already running, Err on failure.
    pub fn launch(&mut self, binary: &str) -> Result<bool, String> {
        // Clear any previous failure
        self.failed_launches.remove(binary);

        // Check if already running
        if let Some(state) = self.children.get_mut(binary) {
            match state.child.try_wait() {
                Ok(Some(_status)) => {
                    // Process exited, remove it and allow relaunch
                    self.children.remove(binary);
                    self.update_running_status(binary, false);
                }
                Ok(None) => {
                    // Still running
                    return Ok(false);
                }
                Err(e) => {
                    // Error checking status, remove stale entry
                    eprintln!("[slowdesktop] error checking {}: {}", binary, e);
                    self.children.remove(binary);
                    self.update_running_status(binary, false);
                }
            }
        }

        // Find the binary
        let bin_path = self.find_binary(binary).ok_or_else(|| {
            let err = format!("'{}' not found", binary);
            self.failed_launches.insert(binary.to_string(), err.clone());
            err
        })?;

        // Launch the process with proper stdio handling
        let result = Command::new(&bin_path)
            .env("SLOWOS_MANAGED", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn();

        match result {
            Ok(child) => {
                self.children.insert(
                    binary.to_string(),
                    ProcessState {
                        child,
                        started_at: Instant::now(),
                    },
                );
                self.update_running_status(binary, true);
                Ok(true)
            }
            Err(e) => {
                let err = format!("failed to start: {}", e);
                self.failed_launches.insert(binary.to_string(), err.clone());
                Err(err)
            }
        }
    }

    /// Update the running status for an app
    fn update_running_status(&mut self, binary: &str, running: bool) {
        if let Some(app) = self.apps.iter_mut().find(|a| a.binary == binary) {
            app.running = running;
        }
    }

    /// Poll all running processes and update their status.
    /// Returns list of apps that have exited since last poll.
    pub fn poll(&mut self) -> Vec<String> {
        let mut exited = Vec::new();

        let binaries: Vec<String> = self.children.keys().cloned().collect();
        for binary in binaries {
            if let Some(state) = self.children.get_mut(&binary) {
                match state.child.try_wait() {
                    Ok(Some(status)) => {
                        if !status.success() {
                            let runtime = state.started_at.elapsed();
                            eprintln!(
                                "[slowdesktop] {} exited with {} after {:.1}s",
                                binary,
                                status,
                                runtime.as_secs_f32()
                            );
                        }
                        exited.push(binary.clone());
                    }
                    Ok(None) => {
                        // Still running
                    }
                    Err(e) => {
                        eprintln!("[slowdesktop] error polling {}: {}", binary, e);
                        exited.push(binary.clone());
                    }
                }
            }
        }

        // Clean up exited processes
        for binary in &exited {
            self.children.remove(binary);
            self.update_running_status(binary, false);
        }

        exited
    }

    /// Shut down all running applications gracefully
    pub fn shutdown_all(&mut self) {
        let binaries: Vec<String> = self.children.keys().cloned().collect();

        for binary in &binaries {
            if let Some(mut state) = self.children.remove(binary) {
                // Send termination signal
                if let Err(e) = state.child.kill() {
                    eprintln!("[slowdesktop] error killing {}: {}", binary, e);
                }

                // Wait with timeout
                let start = Instant::now();
                let timeout = Duration::from_secs(3);

                loop {
                    match state.child.try_wait() {
                        Ok(Some(_)) => break,
                        Ok(None) => {
                            if start.elapsed() > timeout {
                                eprintln!("[slowdesktop] {} did not exit in time", binary);
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(50));
                        }
                        Err(e) => {
                            eprintln!("[slowdesktop] error waiting for {}: {}", binary, e);
                            break;
                        }
                    }
                }
            }
        }

        // Reset all running states
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

    /// Get the last error for an app, if any
    #[allow(dead_code)]
    pub fn last_error(&self, binary: &str) -> Option<&str> {
        self.failed_launches.get(binary).map(|s| s.as_str())
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ProcessManager {
    fn drop(&mut self) {
        self.shutdown_all();
    }
}
