use std::sync::Arc;

use faber_config::{FaberConfig, logging::LogRotation};
use tracing::{Level, subscriber::set_global_default};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn init_logging(config: Arc<FaberConfig>) -> Result<(), Box<dyn std::error::Error>> {
    let env_filter = match config.logging.level {
        Level::ERROR => "error",
        Level::WARN => "warn",
        Level::INFO => "info",
        Level::DEBUG => "debug",
        Level::TRACE => "trace",
    };

    std::fs::create_dir_all(&config.logging.dir)?;

    let file_appender = match config.logging.rotation {
        LogRotation::Hourly => {
            tracing_appender::rolling::hourly(&config.logging.dir, &config.logging.file_name_prefix)
        }
        LogRotation::Daily => {
            tracing_appender::rolling::daily(&config.logging.dir, &config.logging.file_name_prefix)
        }
    };

    let console_layer = fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false);

    let file_layer = fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_writer(file_appender);

    let subscriber = tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .with(EnvFilter::new(env_filter));

    let _ = set_global_default(subscriber);
    Ok(())
}
