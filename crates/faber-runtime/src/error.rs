use thiserror::Error;

#[derive(Error, Debug)]
pub enum FaberError {
    #[error("General error: {message}")]
    Generic { message: String },

    #[error("Failed to write file:\n Error: {e}\nDetails: {details}")]
    WriteFile { e: std::io::Error, details: String },

    #[error("Failed to create pipe:\n Details: {details} \nError: {e}")]
    MkPipe { e: std::io::Error, details: String },

    #[error("Failed to close file descriptor:\n Error: {e}")]
    CloseFd { e: nix::Error },

    #[error("Failed to wait for child process:\n Error: {e}")]
    WaitPid { e: nix::Error },

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

    #[error("Failed to enable cgroup controllers:\n Error: {e}\nDetails: {details}")]
    CgroupControllers { e: std::io::Error, details: String },

    #[error("Failed to create dev device: {detaills}")]
    MkDevDevice {
        detaills: String,
        e: nix::errno::Errno,
    },

    #[error("Failed to execute task:\n Error: {e}\nDetails: {details}")]
    ExecuteTask { e: std::io::Error, details: String },

    #[error("Failed to get stdout from task:\n Error: {e}\nDetails: {details}")]
    GetStdout {
        e: std::string::FromUtf8Error,
        details: String,
    },

    #[error("Failed to get stderr from task:\n Error: {e}\nDetails: {details}")]
    GetStderr {
        e: std::string::FromUtf8Error,
        details: String,
    },

    #[error("Failed to get stdin from task:\n Details: {details}")]
    GetStdin { details: String },

    #[error("Failed to write stdin to task:\n Error: {e}\nDetails: {details}")]
    WriteStdin { e: std::io::Error, details: String },

    #[error("Failed to get exit code from task:\n Error: {e}\nDetails: {details}")]
    GetExitCode { e: std::io::Error, details: String },

    #[error("Failed to set user ID:\n Error: {e}")]
    SetUserId { e: nix::Error },

    #[error("Failed to set group ID:\n Error: {e}")]
    SetGroupId { e: nix::Error },

    #[error("Failed to set hostname:\n Error: {e}\nDetails: {details}")]
    SetHostname { e: nix::Error, details: String },

    #[error("Failed to enable cgroup controllers:\n Error: {e}\nDetails: {details}")]
    CgroupControllerEnable { e: std::io::Error, details: String },

    #[error("Task exceeded timeout limit:\n Timeout: {timeout_duration:?}\nDetails: {details}")]
    TaskTimeout {
        timeout_duration: std::time::Duration,
        details: String,
    },
}
