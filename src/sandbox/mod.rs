//! Container-based sandboxing implementation
//!
//! This module provides secure container isolation for task execution,
//! similar to go-judge functionality.

pub mod container;
pub mod mounts;
pub mod namespaces;
pub mod resource_limits;

pub use container::ContainerSandbox;
pub use mounts::{Mount, MountConfig};
pub use namespaces::NamespaceConfig;
pub use resource_limits::ResourceLimits;

use thiserror::Error;

/// Errors that can occur during sandbox operations
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
    #[error("System call failed: {0}")]
    SystemCall(#[from] nix::Error),
    #[error("IO operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Security violation: {0}")]
    SecurityViolation(String),
}

pub type Result<T> = std::result::Result<T, SandboxError>;
