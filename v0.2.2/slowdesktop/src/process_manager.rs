//! Process manager for SlowOS applications
//!
//! Manages child processes for each app. Tracks running state,
//! handles clean shutdown, and provides robust error handling.

use std::collections::HashMap;
use std::sync::{atomic::{AtomicU64, Ordering}, Arc};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use std::env;

#[cfg(target_os = "linux")]
use x11rb::connection::Connection;
#[cfg(target_os = "linux")]
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt, InputFocus, Window};

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

/// Apps that allow multiple simultaneous instances
const MULTI_INSTANCE_APPS: &[&str] = &[
    "slowfiles",
    "slowpaint",
    "slowwrite",
    "slowmidi",
    "slowview",
    "slowdesign",
];

/// Manages running application processes
pub struct ProcessManager {
    /// Registry of all known applications
    apps: Vec<AppInfo>,
    /// Running child processes, keyed by binary name (or binary_N for multi-instance)
    children: HashMap<String, ProcessState>,
    /// Path to search for app binaries
    bin_paths: Vec<PathBuf>,
    /// Apps that failed to launch (with error message)
    failed_launches: HashMap<String, String>,
    /// Counter for multi-instance apps
    instance_counter: HashMap<String, u32>,
    /// Cascade offset for window staggering (cycles 0-9)
    cascade_offset: u32,
    /// Focus request sequence used to make focus handoff deterministic.
    focus_request_id: Arc<AtomicU64>,
    /// Bumps when any app `running` flag changes so search UI can invalidate caches.
    app_state_epoch: u64,
    /// Resolved binary path per app name; avoids repeated PATH `exists`/`metadata` probes.
    binary_path_cache: HashMap<String, Option<PathBuf>>,
}

impl ProcessManager {
    fn focus_diagnostics_enabled() -> bool {
        match env::var("SLOWOS_FOCUS_DIAG_TOOLS") {
            Ok(v) => {
                let value = v.to_lowercase();
                !(value.is_empty() || value == "0" || value == "false" || value == "off" || value == "no")
            }
            Err(_) => false,
        }
    }

    fn focus_sequence_log(&self, seq: u64, stage: &str, detail: &str) {
        eprintln!("[slowdesktop][focus] seq={seq} stage={stage} detail={detail}");
    }
    pub fn new() -> Self {
        let mut pm = Self {
            apps: Vec::new(),
            children: HashMap::new(),
            bin_paths: Self::build_bin_paths(),
            failed_launches: HashMap::new(),
            instance_counter: HashMap::new(),
            cascade_offset: 0,
            focus_request_id: Arc::new(AtomicU64::new(0)),
            app_state_epoch: 0,
            binary_path_cache: HashMap::new(),
        };
        pm.register_apps();
        pm
    }

    /// Check if an app allows multiple instances
    fn allows_multi_instance(binary: &str) -> bool {
        MULTI_INSTANCE_APPS.contains(&binary)
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
        // (binary, display_name, description, icon_label)
        const APP_DEFS: &[(&str, &str, &str, &str)] = &[
            ("slowwrite",     "slowWrite",  "word processor",      "W"),
            ("slowpaint",     "slowPaint",  "bitmap editor",       "P"),
            ("slowdesign",    "slowDesign", "document design",     "D"),
            ("slowreader",    "slowReader", "ebook reader",        "R"),
            ("slownotes",     "slowNotes",  "notes",               "N"),
            ("slowchess",     "chess",      "chess",               "c"),
            ("slowfiles",     "slowFiles",  "file manager",        "F"),
            ("slowmusic",     "slowMusic",  "music player",        "M"),
            ("slowclock",     "slowClock",  "clock",               "⏱"),
            ("trash",         "trash",      "trash bin",           "X"),
            ("slowterm",      "terminal",   "terminal emulator",   ">"),
            ("slowview",      "slowView",   "image & PDF viewer",  "V"),
            ("credits",       "credits",    "open source credits", "C"),
            ("slowmidi",      "slowMidi",   "MIDI sequencer",      "m"),
            ("slowbreath",    "slowBreath", "breathing timer",     "~"),
            ("settings",      "settings",   "system settings",     "*"),
            ("slowcalc",      "calculator", "calculator",          "="),
            ("slowsolitaire", "solitaire",  "solitaire",           "\u{2660}"),
        ];

        self.apps = APP_DEFS.iter().map(|&(bin, name, desc, icon)| AppInfo {
            binary: bin.into(),
            display_name: name.into(),
            description: desc.into(),
            icon_label: icon.into(),
            running: false,
        }).collect();
    }

    /// Monotonic counter bumped when any registered app's `running` flag changes.
    pub fn app_state_epoch(&self) -> u64 {
        self.app_state_epoch
    }

    /// Get all registered apps
    pub fn apps(&self) -> &[AppInfo] {
        &self.apps
    }

    /// Check if a binary exists and is executable
    pub fn binary_exists(&mut self, binary: &str) -> bool {
        self.find_binary(binary).is_some()
    }

    /// Find the binary path for an app (cached by binary name).
    pub fn find_binary(&mut self, binary: &str) -> Option<PathBuf> {
        if let Some(cached) = self.binary_path_cache.get(binary) {
            return cached.clone();
        }
        let resolved = Self::resolve_binary_path(&self.bin_paths, binary);
        self.binary_path_cache
            .insert(binary.to_string(), resolved.clone());
        resolved
    }

    fn resolve_binary_path(bin_paths: &[PathBuf], binary: &str) -> Option<PathBuf> {
        for base in bin_paths {
            let path = base.join(binary);
            if path.exists() && path.is_file() {
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

    /// Launch an application with extra arguments.
    pub fn launch_with_args(&mut self, binary: &str, args: &[&str]) -> Result<bool, String> {
        self.launch_inner(binary, args)
    }

    /// Launch an application. If already running, bring window to front.
    /// Returns Ok(true) if launched, Ok(false) if already running, Err on failure.
    pub fn launch(&mut self, binary: &str) -> Result<bool, String> {
        self.launch_inner(binary, &[])
    }

    fn launch_inner(&mut self, binary: &str, args: &[&str]) -> Result<bool, String> {
        // Clear any previous failure
        self.failed_launches.remove(binary);

        let multi_instance = Self::allows_multi_instance(binary);

        // For single-instance apps, check if already running
        if !multi_instance {
            if let Some(state) = self.children.get_mut(binary) {
                match state.child.try_wait() {
                    Ok(Some(_status)) => {
                        // Process exited, remove it and allow relaunch
                        self.children.remove(binary);
                        self.update_running_status(binary, false);
                    }
                    Ok(None) => {
                        // Still running - bring window to front
                        self.bring_to_front(binary);
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
        }

        // Find the binary
        let bin_path = self.find_binary(binary).ok_or_else(|| {
            let err = format!("'{}' not found", binary);
            self.failed_launches.insert(binary.to_string(), err.clone());
            err
        })?;

        // Calculate cascade offset (cycles 0-9, 30 pixels per step)
        let cascade = self.cascade_offset;
        self.cascade_offset = (self.cascade_offset + 1) % 10;

        // Launch the process with proper stdio handling
        let mut cmd = Command::new(&bin_path);
        cmd.env("SLOWOS_MANAGED", "1")
            .env("SLOWOS_CASCADE", cascade.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        if !args.is_empty() {
            cmd.args(args);
        }
        let result = cmd.spawn();

        match result {
            Ok(child) => {

                // Generate unique key for multi-instance apps
                let key = if multi_instance {
                    let counter = self.instance_counter.entry(binary.to_string()).or_insert(0);
                    *counter += 1;
                    format!("{}_{}", binary, counter)
                } else {
                    binary.to_string()
                };
                self.children.insert(
                    key,
                    ProcessState {
                        child,
                        started_at: Instant::now(),
                    },
                );
                self.update_running_status(binary, true);
                // Keep launch focus path aligned with already-running apps.
                self.focus_app(binary);
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
            if app.running != running {
                app.running = running;
                self.app_state_epoch = self.app_state_epoch.wrapping_add(1);
            }
        }
    }

    /// Bring an already-running app's window to the front
    fn bring_to_front(&self, binary: &str) {
        // Get the display name for the window title
        let window_title = self
            .apps
            .iter()
            .find(|a| a.binary == binary)
            .map(|a| a.display_name.as_str())
            .unwrap_or(binary);
        let focus_request_id = self
            .focus_request_id
            .fetch_add(1, Ordering::SeqCst)
            .saturating_add(1);

        #[cfg(target_os = "linux")]
        for pid in self.running_pids_for_binary(binary) {
            Self::request_focus_for_pid_async(pid, focus_request_id, Arc::clone(&self.focus_request_id));
        }

        let seq = focus_request_id;
        if Self::focus_diagnostics_enabled() {
            self.focus_sequence_log(seq, "diagnostic_fallback", "requesting wmctrl/xdotool by title");
            // Keep external tools as fallback when x11rb-based PID focus is not available or unsupported.
            Self::spawn_focus_command("wmctrl", &["-a", window_title]);
            Self::spawn_focus_command(
                "xdotool",
                &["search", "--name", window_title, "windowactivate"],
            );
        } else {
            self.focus_sequence_log(seq, "diagnostic_fallback", "disabled");
        }
    }

    /// Request deterministic focus handoff for an already-running app.
    pub fn focus_app(&self, binary: &str) {
        self.bring_to_front(binary);
    }

    #[cfg(target_os = "linux")]
    fn spawn_focus_command(program: &str, args: &[&str]) -> bool {
        Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
    }

    #[cfg(not(target_os = "linux"))]
    fn spawn_focus_command(_program: &str, _args: &[&str]) -> bool {
        false
    }

    /// Collect all running PIDs for a tracked binary (handles multi-instance keys).
    fn running_pids_for_binary(&self, binary: &str) -> Vec<u32> {
        self.children
            .iter()
            .filter_map(|(key, state)| {
                let key_matches = key == binary
                    || key.split_once('_').is_some_and(|(name, _)| name == binary);
                if key_matches {
                    Some(state.child.id())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Trigger a focused launch recovery pass for the newly spawned process.
    #[cfg(target_os = "linux")]
    fn request_focus_for_pid_async(pid: u32, focus_request_id: u64, focus_sequence: Arc<AtomicU64>) {
        let _ = std::thread::spawn(move || {
            let mut attempt = 0;
            let max_attempts = 12;
            eprintln!("[slowdesktop][focus] seq={focus_request_id} stage=request pid={pid} max_attempts={max_attempts}");
            while attempt < max_attempts {
                if focus_sequence.load(Ordering::Acquire) != focus_request_id {
                    eprintln!("[slowdesktop][focus] seq={focus_request_id} stage=abort pid={pid} stale_sequence");
                    return;
                }
                if Self::focus_window_for_pid(pid) {
                    eprintln!("[slowdesktop][focus] seq={focus_request_id} stage=acquired_native pid={pid} attempt={attempt}");
                    return;
                }
                if Self::focus_diagnostics_enabled() && Self::focus_window_with_pid_xdotool(pid) {
                    eprintln!("[slowdesktop][focus] seq={focus_request_id} stage=acquired_xdotool pid={pid} attempt={attempt}");
                    return;
                }
                if Self::focus_diagnostics_enabled() && Self::focus_window_with_pid_wmctrl(pid) {
                    eprintln!("[slowdesktop][focus] seq={focus_request_id} stage=acquired_wmctrl pid={pid} attempt={attempt}");
                    return;
                }
                attempt += 1;
                eprintln!("[slowdesktop][focus] seq={focus_request_id} stage=retry pid={pid} attempt={attempt}");
                std::thread::sleep(Duration::from_millis(60));
            }
            eprintln!("[slowdesktop][focus] seq={focus_request_id} stage=timeout pid={pid}");
        });
    }

    #[cfg(not(target_os = "linux"))]
    fn request_focus_for_pid_async(
        _pid: u32,
        _focus_request_id: u64,
        _focus_sequence: Arc<AtomicU64>,
    ) {
    }

    /// Best-effort async focus recovery for a known running PID (Linux only).
    #[cfg(target_os = "linux")]
    fn focus_window_for_pid(pid: u32) -> bool {
        let display = match std::env::var("DISPLAY").ok().filter(|v| !v.is_empty()) {
            Some(value) if value != "none" => value,
            _ => return false,
        };

        let Ok((conn, screen_num)) = x11rb::connect(Some(&display)) else {
            return false;
        };

        let atom_reply = match conn.intern_atom(false, b"_NET_WM_PID") {
            Ok(cookie) => match cookie.reply() {
                Ok(reply) => reply,
                Err(_) => return false,
            },
            Err(_) => return false,
        };

        let root_window = conn.setup().roots[screen_num].root;
        let tree_reply = match Self::find_window_by_pid_recursive(
            &conn,
            atom_reply.atom,
            root_window,
            pid,
        ) {
            Some(window) => window,
            None => return false,
        };

        if conn
            .set_input_focus(InputFocus::PARENT, tree_reply, x11rb::CURRENT_TIME)
            .is_err()
        {
            return false;
        }

        if conn.flush().is_err() {
            return false;
        }

        let focus_reply = match conn.get_input_focus() {
            Ok(cookie) => match cookie.reply() {
                Ok(reply) => reply,
                Err(_) => return false,
            },
            Err(_) => return false,
        };
        if focus_reply.focus == x11rb::NONE {
            return false;
        }
        let focused_pid = Self::window_pid_matches(&conn, atom_reply.atom, focus_reply.focus);
        if focused_pid != Some(pid) {
            return false;
        }

        true
    }

    #[cfg(target_os = "linux")]
    fn focus_window_with_pid_xdotool(pid: u32) -> bool {
        let pid_arg = pid.to_string();
        Command::new("xdotool")
            .args(["search", "--pid", pid_arg.as_str(), "windowactivate"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[cfg(target_os = "linux")]
    fn focus_window_with_pid_wmctrl(pid: u32) -> bool {
        let Ok(output) = Command::new("wmctrl").args(["-lp"]).output() else {
            return false;
        };
        if !output.status.success() {
            return false;
        }
        let Ok(output_text) = String::from_utf8(output.stdout) else {
            return false;
        };
        let Some(window_id) = Self::find_wmctrl_window_id_for_pid(&output_text, pid) else {
            return false;
        };
        Self::spawn_focus_command("wmctrl", &["-ia", &window_id])
    }

    #[cfg(target_os = "linux")]
    fn find_wmctrl_window_id_for_pid(wmctrl_output: &str, pid: u32) -> Option<String> {
        for line in wmctrl_output.lines() {
            let mut parts = line.split_whitespace();
            let window_id = parts.next()?;
            let _desktop = parts.next();
            let output_pid = parts.next()?;
            if let Ok(output_pid) = output_pid.parse::<u32>() {
                if output_pid == pid {
                    return Some(window_id.to_string());
                }
            }
        }
        None
    }

    #[cfg(target_os = "linux")]
    fn find_window_by_pid_recursive(
        conn: &impl Connection,
        pid_atom: Atom,
        root: Window,
        pid: u32,
    ) -> Option<Window> {
        let tree_reply = match conn.query_tree(root) {
            Ok(cookie) => match cookie.reply() {
                Ok(reply) => reply,
                Err(_) => return None,
            },
            Err(_) => return None,
        };

        for child in &tree_reply.children {
            if Self::window_pid_matches(conn, pid_atom, *child) == Some(pid) {
                return Some(*child);
            }
        }

        for child in tree_reply.children {
            if let Some(found) = Self::find_window_by_pid_recursive(conn, pid_atom, child, pid) {
                return Some(found);
            }
        }

        None
    }

    #[cfg(target_os = "linux")]
    fn window_pid_matches(conn: &impl Connection, pid_atom: Atom, window: Window) -> Option<u32> {
        let property = conn
            .get_property(false, window, pid_atom, AtomEnum::CARDINAL, 0, 1)
            .ok()?
            .reply()
            .ok()?;

        property.value32().and_then(|mut values| values.next())
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

    /// Check if a specific app is running (with actual process state verification)
    /// For multi-instance apps, always returns false to allow launching additional instances
    pub fn is_running(&mut self, binary: &str) -> bool {
        // Multi-instance apps can always be launched again
        if Self::allows_multi_instance(binary) {
            return false;
        }
        if let Some(state) = self.children.get_mut(binary) {
            // Actually check if the process is still alive
            match state.child.try_wait() {
                Ok(Some(_status)) => {
                    // Process has exited - remove it
                    self.children.remove(binary);
                    self.update_running_status(binary, false);
                    false
                }
                Ok(None) => {
                    // Still running
                    true
                }
                Err(_) => {
                    // Error checking - assume dead
                    self.children.remove(binary);
                    self.update_running_status(binary, false);
                    false
                }
            }
        } else {
            false
        }
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
