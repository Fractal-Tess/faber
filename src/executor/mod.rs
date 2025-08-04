pub mod error;
pub mod task;
pub mod task_executor;

pub use task_executor::TaskExecutor;
// Backward compatibility alias
pub use task::{Task, TaskResult, TaskStatus};
pub use task_executor::TaskExecutor as Executor;
// Backward compatibility aliases
pub use task::{ExecutionTask, ExecutionTaskResult, ExecutionTaskStatus};
