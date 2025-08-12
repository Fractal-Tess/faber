//! Faber runtime core crate.
//!
//! This crate provides the primitives to construct a minimal, sandboxed execution
//! environment using Linux namespaces and `nix`, along with a high-level builder
//! and runtime for running tasks with isolated filesystem and tmpfs-backed workdir.
//!
//! # Example
//!
//! ```no_run
//! use faber::{Runtime, RuntimeBuilder, Task};
//!
//! let tasks = vec![Task { cmd: "echo".into(), args: Some(vec!["hello".into()]), env: None, cwd: None, stdin: None, files: None }];
//! let runtime = Runtime::builder()
//!     .with_workdir("/faber".into())
//!     .build()
//!     .expect("failed to build runtime");
//! let _results = runtime.run(tasks).expect("failed to run");
//! ```

mod builder;
mod cgroup;
mod environment;
mod error;
mod executor;
mod prelude;
mod runtime;
mod types;
mod utils;

pub use builder::RuntimeBuilder;
pub use runtime::Runtime;
pub use types::{CgroupConfig, Mount, RuntimeLimits, Task, TaskResult};
