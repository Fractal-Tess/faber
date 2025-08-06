use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum FaberConfigError {
    #[error("Config file  was not found at: {0}")]
    ConfigNotFound(PathBuf),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(
        "Failed to parse TOML configuration: {}. Please check your config file for syntax errors.",
        extract_toml_error_message(_0)
    )]
    Toml(#[from] toml::de::Error),
}

fn extract_toml_error_message(error: &toml::de::Error) -> String {
    error.message().to_owned()
}
