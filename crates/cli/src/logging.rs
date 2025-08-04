use tracing::{Level, subscriber::set_global_default};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn init_logging(level: Level, debug: bool, log_file: Option<&str>) {
    let env_filter = if debug {
        "debug"
    } else {
        match level {
            Level::ERROR => "error",
            Level::WARN => "warn",
            Level::INFO => "info",
            Level::DEBUG => "debug",
            Level::TRACE => "trace",
        }
    };

    // Ensure the logs directory exists
    let _ = std::fs::create_dir_all("logs");

    // Create file appender based on log_file parameter
    let file_appender = if let Some(log_path) = log_file {
        // Use the specified log file path
        if let Some(parent) = std::path::Path::new(log_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // For a single file, use a non-rolling appender
        tracing_appender::rolling::never("", log_path)
    } else {
        // Use daily rolling log
        tracing_appender::rolling::daily("logs", "faber.log")
    };

    // Create console layer
    let console_layer = fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false);

    // Create file layer
    let file_layer = fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_writer(file_appender);

    // Build the subscriber with both console and file layers
    let subscriber = tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .with(EnvFilter::new(env_filter));

    // Set the global default
    set_global_default(subscriber).expect("Failed to set global default subscriber");
}
