//! slowDesktop — the SlowOS desktop shell
//!
//! A System 6-inspired desktop environment for the Slow Computer.
//! Launches and manages all SlowOS applications as child processes.
//!
//! This is the first thing that runs when the Slowbook boots.

mod desktop;
mod launcher;
mod process_manager;

use desktop::DesktopApp;
use eframe::NativeOptions;

fn main() {
    // Install panic hook that logs instead of crashing
    std::panic::set_hook(Box::new(|panic_info| {
        let msg = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        };
        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".to_string());
        eprintln!("[slowdesktop] PANIC at {}: {}", location, msg);
        // Write to log file for post-mortem
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/slowos-crash.log")
            .and_then(|mut f| {
                use std::io::Write;
                writeln!(
                    f,
                    "[{}] PANIC at {}: {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    location,
                    msg
                )
            });
    }));

    // Run the desktop shell — if eframe fails, log and restart
    loop {
        let result = std::panic::catch_unwind(|| {
            let options = NativeOptions {
                viewport: egui::ViewportBuilder::default()
                    .with_inner_size([960.0, 680.0])
                    .with_title("SlowOS")
                    .with_decorations(false)
                    .with_maximized(true),
                ..Default::default()
            };
            eframe::run_native(
                "SlowOS",
                options,
                Box::new(|cc| {
                    slowcore::SlowTheme::default().apply(&cc.egui_ctx);
                    Box::new(DesktopApp::new(cc))
                }),
            )
        });

        match result {
            Ok(Ok(())) => {
                // Clean exit
                break;
            }
            Ok(Err(e)) => {
                eprintln!("[slowdesktop] eframe error: {e}");
                // On embedded, try to restart after a brief pause
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
            Err(_) => {
                eprintln!("[slowdesktop] caught panic in main loop, restarting...");
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }

        // On desktop development, just exit after first failure
        if !is_embedded() {
            break;
        }
    }
}

/// Detect if we're running on the actual Slowbook hardware
fn is_embedded() -> bool {
    // Check for Raspberry Pi or our custom device tree
    std::path::Path::new("/proc/device-tree/model").exists()
        || std::env::var("SLOWOS_EMBEDDED").is_ok()
}
