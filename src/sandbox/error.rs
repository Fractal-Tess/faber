use thiserror::Error;

#[derive(Error, Debug)]
pub enum SandboxError {
    #[error("Container creation failed: {0}")]
    ContainerCreation(String),

    #[error("Mount operation failed: {0}")]
    MountFailed(String),

    #[error("Namespace setup failed: {0}")]
    NamespaceSetup(String),

    #[error("Resource limit enforcement failed: {0}")]
    ResourceLimitFailed(String),

    #[error("Container execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Sandbox cleanup failed: {0}")]
    CleanupFailed(String),
}
