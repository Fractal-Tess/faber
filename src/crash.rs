use std::thread;
use std::time::Duration;

use signal_hook::consts::signal::*;
use signal_hook::iterator::Signals;
use tracing::error;

pub fn install_crash_handlers() {
    eprintln!("=== Installing crash handlers ===");

    // Ignore SIGPIPE so failed pipe writes return EPIPE instead of terminating the process
    unsafe {
        eprintln!("Setting SIGPIPE handler to ignore");
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
        eprintln!("SIGPIPE handler set successfully");
    }

    // Log panics through tracing
    eprintln!("Setting panic hook");
    std::panic::set_hook(Box::new(|info| {
        eprintln!("=== PANIC DETECTED ===");
        eprintln!("panic: {}", info);
        eprintln!("=== PANIC END ===");
        error!("panic: {}", info);
    }));
    eprintln!("Panic hook set successfully");

    eprintln!("Installing signal handlers for SIGTERM, SIGINT, SIGQUIT, SIGHUP");
    let mut signals = match Signals::new([SIGTERM, SIGINT, SIGQUIT, SIGHUP]) {
        Ok(s) => {
            eprintln!("Signal handlers installed successfully");
            s
        }
        Err(e) => {
            eprintln!("Failed to install signal handlers: {}", e);
            error!("failed to install signal handlers: {e}");
            return;
        }
    };

    eprintln!("Spawning signal monitoring thread");
    thread::spawn(move || {
        eprintln!("Signal monitoring thread started");
        if let Some(sig) = signals.forever().next() {
            eprintln!("=== SIGNAL RECEIVED ===");
            eprintln!("received termination signal: {} — exiting", sig);
            error!("received termination signal: {} — exiting", sig);
            thread::sleep(Duration::from_millis(50));
            eprintln!("Exiting with code: {}", 128 + sig);
            std::process::exit(128 + sig);
        }
    });
    eprintln!("=== Crash handlers installation completed ===");
}
