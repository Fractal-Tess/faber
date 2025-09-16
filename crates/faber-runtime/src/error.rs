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

    #[error("Failed to mount:\n Error: {e} \nDetails: {details}")]
    Mount { e: nix::Error, details: String },

    #[error("Failed to unmount:\n Error: {e}\nDetails: {details}")]
    Umount { e: nix::Error, details: String },

    #[error("Failed to pivot root:\n Error: {e}\nDetails: {details}")]
    PivotRoot { e: nix::Error, details: String },

    #[error("Failed to set current directory:\n Error: {e}\nDetails: {details}")]
    Chdir { e: std::io::Error, details: String },

    #[error("Failed to create container root directory:\n Error: {e}\nDetails: {details}")]
    CreateContainerRootDir { e: std::io::Error, details: String },

    #[error("Failed to create directory:\n Error: {e}\nDetails: {details}")]
    CreateDir { e: std::io::Error, details: String },

    #[error("Failed to remove container root directory:\n Error: {e}")]
    RemoveContainerRootDir { e: std::io::Error, details: String },

    #[error("Failed to remove directory:\n Error: {e}")]
    RemoveDir { e: std::io::Error, details: String },

    #[error("Failed to parse result from child process:\n Error: {e}")]
    ParseResult {
        e: serde_json::Error,
        details: String,
    },

    #[error("Failed to create dev device: {detaills}")]
    MkDevDevice { detaills: String },
}
