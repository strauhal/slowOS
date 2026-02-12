//! slowDesktop — the SlowOS desktop shell
//!
//! A System 6-inspired desktop environment for the Slow Computer.
//! Launches and manages all SlowOS applications as child processes.
//!
//! This is the first thing that runs when the Slowbook boots.

mod desktop;
mod process_manager;

use desktop::DesktopApp;
use eframe::NativeOptions;
use std::io::Write;

/// Maximum number of restart attempts before giving up
const MAX_RESTART_ATTEMPTS: u32 = 5;

/// Delay between restart attempts
const RESTART_DELAY_SECS: u64 = 2;

fn main() {
    // Install panic hook that logs instead of crashing
    setup_panic_handler();

    // Run the desktop shell with restart capability
    run_desktop_loop();
}

/// Set up a panic handler that logs crashes to a file
fn setup_panic_handler() {
    std::panic::set_hook(Box::new(|panic_info| {
        let msg = panic_info.payload()
            .downcast_ref::<&str>().map(|s| s.to_string())
            .or_else(|| panic_info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".to_string());

        let location = panic_info.location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        eprintln!("[slowdesktop] PANIC at {}: {}", location, msg);

        // Write to log file for post-mortem analysis
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/slowos-crash.log")
        {
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let _ = writeln!(file, "[{}] PANIC at {}: {}", timestamp, location, msg);

            // Also log backtrace if available
            let backtrace = std::backtrace::Backtrace::capture();
            if backtrace.status() == std::backtrace::BacktraceStatus::Captured {
                let _ = writeln!(file, "Backtrace:\n{}", backtrace);
            }
        }
    }));
}

/// Run the desktop shell with automatic restart on failure
fn run_desktop_loop() {
    let mut restart_count = 0u32;

    loop {
        let result = std::panic::catch_unwind(|| run_desktop());

        match result {
            Ok(Ok(())) => {
                // Clean exit requested
                eprintln!("[slowdesktop] clean shutdown");
                break;
            }
            Ok(Err(e)) => {
                eprintln!("[slowdesktop] eframe error: {}", e);
                restart_count += 1;
            }
            Err(_) => {
                eprintln!("[slowdesktop] caught panic, attempting recovery...");
                restart_count += 1;
            }
        }

        // Check if we should restart
        if !should_restart(restart_count) {
            eprintln!("[slowdesktop] too many failures, giving up");
            break;
        }

        eprintln!(
            "[slowdesktop] restarting in {} seconds (attempt {}/{})",
            RESTART_DELAY_SECS, restart_count, MAX_RESTART_ATTEMPTS
        );
        std::thread::sleep(std::time::Duration::from_secs(RESTART_DELAY_SECS));
    }
}

/// Determine if we should restart after a failure
fn should_restart(restart_count: u32) -> bool {
    // On embedded hardware, always try to restart (up to max attempts)
    if is_embedded() {
        return restart_count < MAX_RESTART_ATTEMPTS;
    }

    // On development machines, don't restart to allow debugging
    false
}

/// Run the desktop application
fn run_desktop() -> Result<(), eframe::Error> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([960.0, 680.0])
            .with_title("slowOS")
            .with_decorations(false)
            .with_maximized(true),
        ..Default::default()
    };

    eframe::run_native(
        "slowOS",
        options,
        Box::new(|cc| {
            // Apply the SlowOS theme
            slowcore::SlowTheme::default().apply(&cc.egui_ctx);
            Box::new(DesktopApp::new(cc))
        }),
    )
}

/// Detect if we're running on the actual Slowbook hardware
fn is_embedded() -> bool {
    // Check for Raspberry Pi or our custom device tree
    std::path::Path::new("/proc/device-tree/model").exists()
        || std::env::var("SLOWOS_EMBEDDED").is_ok()
}
