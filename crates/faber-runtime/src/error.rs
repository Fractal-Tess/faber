use thiserror::Error;

#[derive(Error, Debug)]
pub enum FaberError {
    #[error("General error: {message}")]
    Generic { message: String },

    #[error("Failed to create pipe:\n Details: {details} \nError: {e}")]
    MkPipe { e: std::io::Error, details: String },

    #[error("Failed to close file descriptor:\n Error: {e}")]
    CloseFd { e: nix::Error },

    #[error("Failed to fork:\n Error: {e}")]
    Fork { e: nix::Error },

    #[error("Failed to unshare:\n Error: {e}")]
    Unshare { e: nix::Error },

    #[error("Failed to mount:\n Error: {e}")]
    Mount { e: nix::Error, details: String },

    #[error("Failed to unmount:\n Error: {e}")]
    Unmount { e: nix::Error, details: String },

    #[error("Failed to create container root directory:\n Error: {e}")]
    CreateContainerRootDir { e: std::io::Error },

    #[error("Failed to create directory:\n Error: {e}")]
    CreateDir { e: std::io::Error },

    #[error("Failed to remove container root directory:\n Error: {e}")]
    RemoveContainerRootDir { e: std::io::Error },
}
