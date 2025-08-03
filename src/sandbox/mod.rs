//! Container-based sandboxing implementation
//!
//! This module provides secure container isolation for task execution,
//! similar to go-judge functionality.

// pub mod container;
pub mod error;
// pub mod mounts;
// pub mod namespaces;
// pub mod resource_limits;
pub mod sandbox;

// pub use container::ContainerSandbox;
// pub use mounts::{Mount, MountConfig};
// pub use namespaces::NamespaceConfig;
// pub use resource_limits::ResourceLimits;
pub use sandbox::Sandbox;
