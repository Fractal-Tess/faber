use tracing::{Level, subscriber::set_global_default};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn init_logging(level: &Level, log_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    let env_filter = match *level {
        Level::ERROR => "error",
        Level::WARN => "warn",
        Level::INFO => "info",
        Level::DEBUG => "debug",
        Level::TRACE => "trace",
    };

    std::fs::create_dir_all(log_dir)?;

    let file_appender = tracing_appender::rolling::hourly(log_dir, "faber.log");

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
