use thiserror::Error;

#[derive(Error, Debug)]
pub enum FaberError {
    #[error("General error: {message}")]
    General { message: String },
}
