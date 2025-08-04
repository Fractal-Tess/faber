use thiserror::Error;

#[derive(Error, Debug)]
pub enum FaberError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Sandbox error: {0}")]
    Sandbox(String),

    #[error("Execution error: {0}")]
    Execution(String),

    #[error("API error: {0}")]
    Api(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Resource limit exceeded: {0}")]
    ResourceLimit(String),

    #[error("Timeout: {0}")]
    Timeout(String),
}

pub type Result<T> = std::result::Result<T, FaberError>;
