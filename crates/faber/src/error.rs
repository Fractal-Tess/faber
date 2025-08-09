use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Faber generic error: {0}")]
    GenericError(String),

    #[error("Unshare failed")]
    UnshareFailed,

    #[error("Failed to create directory: {0}")]
    IoError(#[from] std::io::Error),

    #[error("FFI NulError: {0}")]
    FFINullError(#[from] std::ffi::NulError),

    #[error("Nix error: {0}")]
    NixError(#[from] nix::Error),
}
