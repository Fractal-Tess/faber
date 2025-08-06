use serde::{Deserialize, Deserializer};
use std::str::FromStr;
use tracing::Level;

/// Queue configuration for the execution queue system
#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    /// The directory to store logs
    pub dir: String,
    /// The log level
    #[serde(deserialize_with = "deserialize_log_level")]
    pub level: Level,
    /// The log rotation
    #[serde(deserialize_with = "deserialize_log_rotation")]
    pub rotation: LogRotation,
    /// The prefix for the log file name
    pub file_name_prefix: String,
}

#[derive(Debug, Clone, Deserialize)]
pub enum LogRotation {
    Hourly,
    Daily,
}

fn deserialize_log_level<'de, D>(deserializer: D) -> Result<Level, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Level::from_str(&s).map_err(|_| {
        serde::de::Error::invalid_value(serde::de::Unexpected::Str(&s), &"valid log level")
    })
}

fn deserialize_log_rotation<'de, D>(deserializer: D) -> Result<LogRotation, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.as_str() {
        "hourly" => Ok(LogRotation::Hourly),
        "daily" => Ok(LogRotation::Daily),
        _ => Err(serde::de::Error::invalid_value(
            serde::de::Unexpected::Str(&s),
            &"valid log rotation",
        )),
    }
}
