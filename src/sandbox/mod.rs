//! Container-based sandboxing implementation
//!
//! This module provides secure container isolation for task execution,
//! similar to go-judge functionality.

pub mod cgroups;
pub mod container;
pub mod error;
pub mod mounts;
pub mod namespaces;
pub mod privileges;
pub mod seccomp;

pub use container::ContainerSandbox;
pub use mounts::{MountConfig, MountManager, MountPoint, MountType, SymLink};
pub use namespaces::{NamespaceConfig, NamespaceManager};
pub use seccomp::{SeccompFilter, SeccompLevel};

// Re-export ContainerSandbox as Sandbox for backward compatibility
pub use container::ContainerSandbox as Sandbox;
