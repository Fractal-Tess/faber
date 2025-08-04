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

    // Set the global default (ignore if already set)
    let _ = set_global_default(subscriber);
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn test_init_logging_with_debug() {
        let temp_dir = TempDir::new().unwrap();
        let log_file = temp_dir.path().join("test.log");

        // This should not panic
        init_logging(Level::INFO, true, Some(log_file.to_str().unwrap()));

        // Verify the log file was created
        assert!(log_file.exists());
    }

    #[test]
    fn test_init_logging_without_debug() {
        let temp_dir = TempDir::new().unwrap();
        let log_file = temp_dir.path().join("test.log");

        // This should not panic
        init_logging(Level::WARN, false, Some(log_file.to_str().unwrap()));

        // Verify the log file was created
        assert!(log_file.exists());
    }

    #[test]
    fn test_init_logging_with_different_levels() {
        let temp_dir = TempDir::new().unwrap();
        let log_file = temp_dir.path().join("test.log");

        // Test all log levels
        let levels = vec![
            Level::ERROR,
            Level::WARN,
            Level::INFO,
            Level::DEBUG,
            Level::TRACE,
        ];

        for level in levels {
            let test_file = temp_dir.path().join(format!("test_{:?}.log", level));
            init_logging(level, false, Some(test_file.to_str().unwrap()));
            assert!(test_file.exists());
        }
    }

    #[test]
    fn test_init_logging_with_directory_creation() {
        let temp_dir = TempDir::new().unwrap();
        let nested_dir = temp_dir.path().join("nested").join("deep");
        let log_file = nested_dir.join("test.log");

        // This should create the nested directory structure
        init_logging(Level::INFO, false, Some(log_file.to_str().unwrap()));

        // Verify the directory and file were created
        assert!(nested_dir.exists());
        assert!(log_file.exists());
    }

    #[test]
    fn test_init_logging_default_rolling() {
        // This should not panic and should create the logs directory
        init_logging(Level::INFO, false, None);

        // Verify the logs directory was created
        assert!(Path::new("logs").exists());
    }

    #[test]
    fn test_logging_level_mapping() {
        let temp_dir = TempDir::new().unwrap();
        let log_file = temp_dir.path().join("test.log");

        // Test that debug mode overrides the level
        init_logging(Level::ERROR, true, Some(log_file.to_str().unwrap()));
        assert!(log_file.exists());

        // Test that non-debug mode uses the specified level
        let log_file2 = temp_dir.path().join("test2.log");
        init_logging(Level::TRACE, false, Some(log_file2.to_str().unwrap()));
        assert!(log_file2.exists());
    }

    #[test]
    fn test_logging_with_relative_path() {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Test with relative path
        init_logging(Level::INFO, false, Some("relative_test.log"));

        // Verify the file was created
        assert!(Path::new("relative_test.log").exists());
    }
}
