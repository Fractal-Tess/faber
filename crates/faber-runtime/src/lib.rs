mod cancel;
mod cgroup;
mod container;
mod error;
mod prelude;
mod result;
mod runtime;
mod task;
mod utils;

pub use cancel::CancellationToken;
pub use cgroup::CgroupConfigBuilder;
pub use container::{ContainerConfigBuilder, DEFAULT_READONLY_PATHS};
pub use error::FaberError;

pub use result::{
    ExecutionStepResult, RuntimeResult, TaskGroupResult, TaskOutcome, TaskResult, TaskResultStats,
};
pub use runtime::{Runtime, RuntimeBuilder};
pub use task::{ExecutionStep, SandboxProfile, Task, TaskGroup};

/// Parse a cgroup memory size such as `4096`, `512K` or `256M`. `max` yields
/// `u64::MAX`.
pub fn parse_memory_limit(value: &str) -> std::result::Result<u64, FaberError> {
    cgroup::task::parse_memory_string(value)
}
